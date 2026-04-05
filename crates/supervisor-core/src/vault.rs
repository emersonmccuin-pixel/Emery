/// Vault core: DPAPI-encrypted credential storage.
///
/// Values are encrypted using Windows DPAPI (CryptProtectData / CryptUnprotectData),
/// which binds ciphertext to the current user's credentials on this machine.
/// The plaintext value is NEVER logged or included in diagnostics.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use uuid::Uuid;

use crate::models::{
    CreateVaultEntryRequest, VaultAuditEntry, VaultEntry,
};
use crate::store::DatabaseSet;

// ---------------------------------------------------------------------------
// DPAPI encryption (Windows-only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod dpapi {
    use anyhow::{Result, anyhow};
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_PROMPTSTRUCT,
        CRYPT_INTEGER_BLOB,
    };

    pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>> {
        unsafe {
            let mut input = CRYPT_INTEGER_BLOB {
                cbData: plaintext.len() as u32,
                pbData: plaintext.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            CryptProtectData(
                &mut input,
                None,
                None,
                None,
                None::<*const CRYPTPROTECT_PROMPTSTRUCT>,
                0,
                &mut output,
            )
            .map_err(|e| anyhow!("CryptProtectData failed: {e}"))?;

            // Copy the output into a Vec and free the Windows-allocated memory.
            let len = output.cbData as usize;
            let mut result = vec![0u8; len];
            std::ptr::copy_nonoverlapping(output.pbData, result.as_mut_ptr(), len);
            LocalFree(Some(HLOCAL(output.pbData as *mut _)));
            Ok(result)
        }
    }

    pub fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>> {
        unsafe {
            let mut input = CRYPT_INTEGER_BLOB {
                cbData: ciphertext.len() as u32,
                pbData: ciphertext.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            CryptUnprotectData(
                &mut input,
                None,
                None,
                None,
                None::<*const CRYPTPROTECT_PROMPTSTRUCT>,
                0,
                &mut output,
            )
            .map_err(|e| anyhow!("CryptUnprotectData failed: {e}"))?;

            let len = output.cbData as usize;
            let mut result = vec![0u8; len];
            std::ptr::copy_nonoverlapping(output.pbData, result.as_mut_ptr(), len);
            LocalFree(Some(HLOCAL(output.pbData as *mut _)));
            Ok(result)
        }
    }
}

// ---------------------------------------------------------------------------
// Fallback for non-Windows builds (compile-time stub only; not used at runtime)
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod dpapi {
    use anyhow::{Result, anyhow};

    pub fn encrypt(_plaintext: &[u8]) -> Result<Vec<u8>> {
        Err(anyhow!("DPAPI encryption is only available on Windows"))
    }

    pub fn decrypt(_ciphertext: &[u8]) -> Result<Vec<u8>> {
        Err(anyhow!("DPAPI decryption is only available on Windows"))
    }
}

/// Encrypt plaintext bytes using DPAPI, returning opaque ciphertext.
pub fn encrypt_value(plaintext: &[u8]) -> Result<Vec<u8>> {
    dpapi::encrypt(plaintext)
}

/// Decrypt DPAPI ciphertext, returning plaintext bytes.
pub fn decrypt_value(ciphertext: &[u8]) -> Result<Vec<u8>> {
    dpapi::decrypt(ciphertext)
}

// ---------------------------------------------------------------------------
// Lock state (in-memory, not persisted)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct LockState {
    unlocked: bool,
    unlocked_at: Option<i64>,
    /// Unix seconds when the lock auto-expires.  `None` = no expiry.
    unlock_expires_at: Option<i64>,
}

impl Default for LockState {
    fn default() -> Self {
        Self {
            unlocked: false,
            unlocked_at: None,
            unlock_expires_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// VaultService
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VaultService {
    databases: DatabaseSet,
    lock: Arc<Mutex<LockState>>,
}

impl VaultService {
    pub fn new(databases: DatabaseSet) -> Self {
        Self {
            databases,
            lock: Arc::new(Mutex::new(LockState::default())),
        }
    }

    // --- Lock management ---

    /// Unlock the vault.  `duration_minutes` controls auto-lock; `None` = 60 minutes.
    pub fn unlock(&self, duration_minutes: Option<i64>) {
        let now = unix_seconds();
        let minutes = duration_minutes.unwrap_or(60).max(1);
        let mut state = self.lock.lock().expect("vault lock poisoned");
        state.unlocked = true;
        state.unlocked_at = Some(now);
        state.unlock_expires_at = Some(now + minutes * 60);
    }

    /// Lock the vault immediately.
    pub fn lock(&self) {
        let mut state = self.lock.lock().expect("vault lock poisoned");
        state.unlocked = false;
        state.unlock_expires_at = None;
    }

    /// Returns `true` if the vault is currently unlocked and the session has not expired.
    pub fn is_unlocked(&self) -> bool {
        let mut state = self.lock.lock().expect("vault lock poisoned");
        if !state.unlocked {
            return false;
        }
        if let Some(expires_at) = state.unlock_expires_at {
            if unix_seconds() >= expires_at {
                // Auto-lock
                state.unlocked = false;
                state.unlock_expires_at = None;
                return false;
            }
        }
        true
    }

    pub fn lock_state(&self) -> crate::models::VaultLockState {
        let state = self.lock.lock().expect("vault lock poisoned");
        crate::models::VaultLockState {
            unlocked: state.unlocked,
            unlocked_at: state.unlocked_at,
            unlock_expires_at: state.unlock_expires_at,
        }
    }

    // --- CRUD ---

    /// Create or update a vault entry.  The plaintext value is encrypted before storage.
    /// Returns the entry metadata (never the plaintext value).
    pub fn set_entry(
        &self,
        request: CreateVaultEntryRequest,
        actor: &str,
    ) -> Result<VaultEntry> {
        let scope = request.scope.trim().to_string();
        let key = request.key.trim().to_string();
        if scope.is_empty() {
            return Err(anyhow!("vault entry scope must not be empty"));
        }
        if key.is_empty() {
            return Err(anyhow!("vault entry key must not be empty"));
        }

        let encrypted = encrypt_value(request.value.as_bytes())?;
        let now = unix_seconds();

        // Upsert: check for existing entry with same scope+key
        let existing = self.databases.get_vault_entry_by_scope_key(&scope, &key)?;

        let entry_id;
        if let Some(existing) = existing {
            entry_id = existing.id.clone();
            let update = crate::models::VaultEntryUpdateRecord {
                id: existing.id,
                encrypted_value: Some(encrypted),
                description: request.description.clone(),
                updated_at: now,
            };
            self.databases.update_vault_entry(&update)?;
        } else {
            entry_id = Uuid::new_v4().to_string();
            self.databases.insert_vault_entry(&crate::models::NewVaultEntryRecord {
                id: entry_id.clone(),
                scope: scope.clone(),
                key: key.clone(),
                encrypted_value: encrypted,
                description: request.description.clone(),
                created_at: now,
                updated_at: now,
            })?;
        }

        self.databases.insert_vault_audit(&crate::models::NewVaultAuditRecord {
            id: Uuid::new_v4().to_string(),
            entry_id: Some(entry_id.clone()),
            action: "write".to_string(),
            actor: actor.to_string(),
            details_json: None,
            created_at: now,
        })?;

        let entry = self
            .databases
            .get_vault_entry(&entry_id)?
            .ok_or_else(|| anyhow!("vault entry disappeared after upsert"))?;

        Ok(VaultEntry {
            id: entry.id,
            scope: entry.scope,
            key: entry.key,
            description: entry.description,
            created_at: entry.created_at,
            updated_at: entry.updated_at,
        })
    }

    /// Retrieve and decrypt a vault entry value.  Requires vault to be unlocked.
    pub fn get_entry_value(&self, scope: &str, key: &str, actor: &str) -> Result<Option<String>> {
        if !self.is_unlocked() {
            return Err(anyhow!("vault is locked"));
        }

        let row = self.databases.get_vault_entry_by_scope_key(scope, key)?;
        let Some(row) = row else {
            return Ok(None);
        };

        let plaintext_bytes = decrypt_value(&row.encrypted_value)?;
        let plaintext = String::from_utf8(plaintext_bytes)
            .map_err(|_| anyhow!("vault entry value is not valid UTF-8"))?;

        let now = unix_seconds();
        self.databases.insert_vault_audit(&crate::models::NewVaultAuditRecord {
            id: Uuid::new_v4().to_string(),
            entry_id: Some(row.id),
            action: "read".to_string(),
            actor: actor.to_string(),
            details_json: None,
            created_at: now,
        })?;

        Ok(Some(plaintext))
    }

    /// Delete a vault entry by ID.
    pub fn delete_entry(&self, id: &str, actor: &str) -> Result<()> {
        let now = unix_seconds();
        self.databases.insert_vault_audit(&crate::models::NewVaultAuditRecord {
            id: Uuid::new_v4().to_string(),
            entry_id: Some(id.to_string()),
            action: "delete".to_string(),
            actor: actor.to_string(),
            details_json: None,
            created_at: now,
        })?;
        self.databases.delete_vault_entry(id)
    }

    /// List vault entries (metadata only, no plaintext values).
    pub fn list_entries(&self, scope: Option<&str>) -> Result<Vec<VaultEntry>> {
        self.databases.list_vault_entries(scope)
    }

    /// Resolve environment variables for a session launch.
    /// Merges `global` scope with `{project_id}` scope; project entries override global.
    pub fn resolve_env_for_session(&self, project_id: &str) -> Result<HashMap<String, String>> {
        if !self.is_unlocked() {
            return Err(anyhow!("vault is locked"));
        }

        let global_rows = self.databases.list_vault_entry_rows(Some("global"))?;
        let project_rows = self.databases.list_vault_entry_rows(Some(project_id))?;

        let mut env: HashMap<String, String> = HashMap::new();

        for row in global_rows {
            match decrypt_value(&row.encrypted_value) {
                Ok(bytes) => {
                    if let Ok(s) = String::from_utf8(bytes) {
                        env.insert(row.key, s);
                    }
                }
                Err(_) => {
                    // Skip entries that fail to decrypt rather than aborting the session
                }
            }
        }

        for row in project_rows {
            match decrypt_value(&row.encrypted_value) {
                Ok(bytes) => {
                    if let Ok(s) = String::from_utf8(bytes) {
                        env.insert(row.key, s);
                    }
                }
                Err(_) => {}
            }
        }

        Ok(env)
    }

    /// List vault audit log entries.
    pub fn list_audit(&self, entry_id: Option<&str>, limit: usize) -> Result<Vec<VaultAuditEntry>> {
        self.databases.list_vault_audit(entry_id, limit)
    }
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs() as i64
}
