use super::{SessionOutputRedactionRule, SDK_LOCKED_CLAUDE_AUTH_ENV_KEYS};
use crate::db::{ProjectRecord, StorageInfo, WorktreeRecord};
use crate::session_api::SupervisorRuntimeInfo;
use crate::vault::{ResolvedVaultBinding, VaultAccessBindingRequest, VaultBindingDelivery};
use portable_pty::CommandBuilder;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

pub(super) struct SessionLaunchArtifactsGuard {
    storage: StorageInfo,
    session_record_id: i64,
    active: bool,
}

pub(super) struct ParsedLaunchProfileEnv {
    pub(super) literal_env: Vec<(String, String)>,
    pub(super) vault_bindings: Vec<VaultAccessBindingRequest>,
}

pub(super) struct ResolvedLaunchProfileEnv {
    pub(super) literal_env: Vec<(String, String)>,
    pub(super) vault_env_bindings: Vec<ResolvedVaultBinding>,
    pub(super) vault_file_bindings: Vec<MaterializedVaultFileBinding>,
}

pub(super) struct MaterializedVaultFileBinding {
    pub(super) binding: ResolvedVaultBinding,
    pub(super) path: PathBuf,
}

impl SessionLaunchArtifactsGuard {
    pub(super) fn new(storage: StorageInfo, session_record_id: i64) -> Self {
        Self {
            storage,
            session_record_id,
            active: true,
        }
    }

    pub(super) fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for SessionLaunchArtifactsGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        remove_project_commander_mcp_config(&self.storage, self.session_record_id);
        cleanup_session_runtime_secret_artifacts(&self.storage, self.session_record_id);
    }
}

impl ResolvedLaunchProfileEnv {
    pub(super) fn new(
        literal_env: Vec<(String, String)>,
        vault_env_bindings: Vec<ResolvedVaultBinding>,
        vault_file_bindings: Vec<MaterializedVaultFileBinding>,
    ) -> Self {
        Self {
            literal_env,
            vault_env_bindings,
            vault_file_bindings,
        }
    }

    pub(super) fn materialize(
        literal_env: Vec<(String, String)>,
        vault_bindings: Vec<ResolvedVaultBinding>,
        storage: &StorageInfo,
        session_record_id: i64,
    ) -> Result<Self, String> {
        let mut vault_env_bindings = Vec::new();
        let mut vault_file_bindings = Vec::new();
        let result = (|| {
            for (index, binding) in vault_bindings.into_iter().enumerate() {
                match binding.delivery {
                    VaultBindingDelivery::Env => vault_env_bindings.push(binding),
                    VaultBindingDelivery::File => {
                        let path = session_runtime_secret_file_path(
                            storage,
                            session_record_id,
                            index,
                            &binding.env_var,
                        );
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent).map_err(|error| {
                                format!(
                                    "failed to create session runtime secret directory {}: {error}",
                                    parent.display()
                                )
                            })?;
                        }
                        fs::write(&path, binding.value.as_bytes()).map_err(|error| {
                            format!(
                                "failed to materialize vault secret file {}: {error}",
                                path.display()
                            )
                        })?;
                        vault_file_bindings.push(MaterializedVaultFileBinding { binding, path });
                    }
                }
            }
            Ok::<(), String>(())
        })();

        if let Err(error) = result {
            cleanup_session_runtime_secret_artifacts(storage, session_record_id);
            return Err(error);
        }

        Ok(Self::new(
            literal_env,
            vault_env_bindings,
            vault_file_bindings,
        ))
    }

    pub(super) fn vault_env_var_names(&self) -> Vec<String> {
        let mut names = self
            .vault_env_bindings
            .iter()
            .map(|binding| binding.env_var.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn env_vault_entry_names(&self) -> Vec<String> {
        let mut names = self
            .vault_env_bindings
            .iter()
            .map(|binding| binding.entry_name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn file_env_var_names(&self) -> Vec<String> {
        let mut names = self
            .vault_file_bindings
            .iter()
            .map(|binding| binding.binding.env_var.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn file_vault_entry_names(&self) -> Vec<String> {
        let mut names = self
            .vault_file_bindings
            .iter()
            .map(|binding| binding.binding.entry_name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn env_binding_count(&self) -> usize {
        self.vault_env_bindings.len()
    }

    pub(super) fn file_binding_count(&self) -> usize {
        self.vault_file_bindings.len()
    }

    pub(super) fn env_bindings_for_audit(&self) -> Vec<&ResolvedVaultBinding> {
        self.vault_env_bindings.iter().collect()
    }

    pub(super) fn file_bindings_for_audit(&self) -> Vec<&ResolvedVaultBinding> {
        self.vault_file_bindings
            .iter()
            .map(|binding| &binding.binding)
            .collect()
    }

    pub(super) fn into_redaction_rules(self) -> Vec<SessionOutputRedactionRule> {
        self.vault_env_bindings
            .into_iter()
            .map(|binding| SessionOutputRedactionRule {
                label: binding.entry_name,
                value: binding.value,
            })
            .chain(
                self.vault_file_bindings
                    .into_iter()
                    .map(|binding| SessionOutputRedactionRule {
                        label: binding.binding.entry_name,
                        value: binding.binding.value,
                    }),
            )
            .collect()
    }
}

pub(super) fn apply_launch_profile_env(
    command: &mut CommandBuilder,
    launch_env: &ResolvedLaunchProfileEnv,
    lock_sdk_auth_env: bool,
) {
    for (key, value) in &launch_env.literal_env {
        if lock_sdk_auth_env && is_locked_sdk_auth_env_key(key) {
            continue;
        }

        command.env(key, value);
    }

    for binding in &launch_env.vault_env_bindings {
        if lock_sdk_auth_env && is_locked_sdk_auth_env_key(&binding.env_var) {
            continue;
        }

        command.env(&binding.env_var, binding.value.as_str());
    }

    for binding in &launch_env.vault_file_bindings {
        if lock_sdk_auth_env && is_locked_sdk_auth_env_key(&binding.binding.env_var) {
            continue;
        }

        command.env(&binding.binding.env_var, binding.path.display().to_string());
    }
}

pub(super) fn merge_launch_vault_bindings(
    existing: Vec<VaultAccessBindingRequest>,
    additional: &[VaultAccessBindingRequest],
) -> Result<Vec<VaultAccessBindingRequest>, String> {
    let mut merged = Vec::new();

    for binding in existing {
        upsert_launch_vault_binding(&mut merged, normalize_launch_vault_binding(&binding)?);
    }
    for binding in additional {
        upsert_launch_vault_binding(&mut merged, normalize_launch_vault_binding(binding)?);
    }

    Ok(merged)
}

pub(super) fn parse_launch_profile_env(raw: &str) -> Result<ParsedLaunchProfileEnv, String> {
    let value =
        serde_json::from_str::<Value>(raw).map_err(|error| format!("invalid env JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "environment JSON must be an object".to_string())?;

    let mut literal_env = Vec::new();
    let mut vault_bindings = Vec::new();

    for (env_var, value) in object {
        if let Some(binding) = parse_vault_env_binding(env_var, value)? {
            vault_bindings.push(binding);
            continue;
        }

        literal_env.push((
            env_var.clone(),
            value
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| value.to_string()),
        ));
    }

    Ok(ParsedLaunchProfileEnv {
        literal_env,
        vault_bindings,
    })
}

fn is_locked_sdk_auth_env_key(key: &str) -> bool {
    SDK_LOCKED_CLAUDE_AUTH_ENV_KEYS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(key))
}

fn upsert_launch_vault_binding(
    bindings: &mut Vec<VaultAccessBindingRequest>,
    binding: VaultAccessBindingRequest,
) {
    if let Some(existing) = bindings
        .iter_mut()
        .find(|existing| existing.env_var.eq_ignore_ascii_case(&binding.env_var))
    {
        *existing = binding;
    } else {
        bindings.push(binding);
    }
}

fn normalize_launch_vault_binding(
    binding: &VaultAccessBindingRequest,
) -> Result<VaultAccessBindingRequest, String> {
    let env_var = binding.env_var.trim();
    if env_var.is_empty() {
        return Err("launch vault env var is required".to_string());
    }

    let entry_name = binding.entry_name.trim();
    if entry_name.is_empty() {
        return Err("launch vault entry name is required".to_string());
    }

    let mut seen_scope_tags = BTreeSet::new();
    let mut required_scope_tags = Vec::new();
    for scope_tag in &binding.required_scope_tags {
        let normalized = scope_tag.trim();
        if normalized.is_empty() {
            return Err("launch vault scope tag is required".to_string());
        }
        if seen_scope_tags.insert(normalized.to_string()) {
            required_scope_tags.push(normalized.to_string());
        }
    }

    Ok(VaultAccessBindingRequest {
        env_var: env_var.to_string(),
        entry_name: entry_name.to_string(),
        required_scope_tags,
        delivery: binding.delivery.clone(),
    })
}

fn parse_vault_env_binding(
    env_var: &str,
    value: &Value,
) -> Result<Option<VaultAccessBindingRequest>, String> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };

    let source = object
        .get("source")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let entry_name = object
        .get("vault")
        .or_else(|| object.get("entry"))
        .or_else(|| object.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let Some(entry_name) = entry_name else {
        return Ok(None);
    };

    if let Some(source) = source {
        if !source.eq_ignore_ascii_case("vault") {
            return Err(format!(
                "environment JSON binding for {env_var} has unsupported source {source}"
            ));
        }
    }

    let scope_tags_value = object.get("scopeTags").or_else(|| object.get("scope_tags"));
    let required_scope_tags = match scope_tags_value {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        format!(
                            "environment JSON binding for {env_var} contains a non-string scope tag"
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(other) => {
            return Err(format!(
                "environment JSON binding for {env_var} has invalid scopeTags value: {other}"
            ));
        }
        None => Vec::new(),
    };

    let delivery = match object
        .get("delivery")
        .or_else(|| object.get("deliveryMode"))
    {
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "env" => VaultBindingDelivery::Env,
            "file" | "file_path" | "filepath" => VaultBindingDelivery::File,
            other => {
                return Err(format!(
                    "environment JSON binding for {env_var} has unsupported delivery {other}"
                ));
            }
        },
        Some(other) => {
            return Err(format!(
                "environment JSON binding for {env_var} has invalid delivery value: {other}"
            ));
        }
        None => VaultBindingDelivery::Env,
    };

    Ok(Some(VaultAccessBindingRequest {
        env_var: env_var.to_string(),
        entry_name: entry_name.to_string(),
        required_scope_tags,
        delivery,
    }))
}

pub(super) fn build_project_commander_mcp_config_json(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    supervisor_runtime: &SupervisorRuntimeInfo,
    session_record_id: i64,
) -> Result<String, String> {
    let mut headers = serde_json::Map::new();
    headers.insert(
        "x-project-commander-token".to_string(),
        serde_json::Value::String(supervisor_runtime.token.clone()),
    );
    headers.insert(
        "x-project-commander-project-id".to_string(),
        serde_json::Value::String(project.id.to_string()),
    );
    headers.insert(
        "x-project-commander-session-id".to_string(),
        serde_json::Value::String(session_record_id.to_string()),
    );
    headers.insert(
        "x-project-commander-source".to_string(),
        serde_json::Value::String("agent_mcp_http".to_string()),
    );

    if let Some(worktree) = worktree {
        headers.insert(
            "x-project-commander-worktree-id".to_string(),
            serde_json::Value::String(worktree.id.to_string()),
        );
    }

    let config = serde_json::json!({
        "mcpServers": {
            "project-commander": {
                "type": "http",
                "url": format!("http://127.0.0.1:{}/mcp", supervisor_runtime.port),
                "headers": headers
            }
        }
    });
    serde_json::to_string(&config)
        .map_err(|error| format!("failed to serialize Project Commander MCP config: {error}"))
}

pub(super) fn persist_project_commander_mcp_config(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    storage: &StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    session_record_id: i64,
) -> Result<PathBuf, String> {
    let config_json = build_project_commander_mcp_config_json(
        project,
        worktree,
        supervisor_runtime,
        session_record_id,
    )?;
    let config_dir = project_commander_mcp_config_dir(storage);
    fs::create_dir_all(&config_dir).map_err(|error| {
        format!(
            "failed to create Project Commander MCP config directory {}: {error}",
            config_dir.display()
        )
    })?;

    let config_path = project_commander_mcp_config_path(storage, session_record_id);
    fs::write(&config_path, config_json).map_err(|error| {
        format!(
            "failed to write Project Commander MCP config file {}: {error}",
            config_path.display()
        )
    })?;

    Ok(config_path)
}

pub(super) fn remove_project_commander_mcp_config(storage: &StorageInfo, session_record_id: i64) {
    let config_path = project_commander_mcp_config_path(storage, session_record_id);

    if !config_path.exists() {
        return;
    }

    if let Err(error) = fs::remove_file(&config_path) {
        log::warn!(
            "failed to remove Project Commander MCP config file {}: {error}",
            config_path.display()
        );
    }
}

pub(super) fn project_commander_mcp_config_path(
    storage: &StorageInfo,
    session_record_id: i64,
) -> PathBuf {
    project_commander_mcp_config_dir(storage).join(format!(
        "project-commander-session-{session_record_id}.mcp.json"
    ))
}

pub(super) fn session_runtime_secret_dir(storage: &StorageInfo, session_record_id: i64) -> PathBuf {
    session_runtime_secret_root_dir(storage).join(format!("session-{session_record_id}"))
}

pub(super) fn session_runtime_secret_file_path(
    storage: &StorageInfo,
    session_record_id: i64,
    ordinal: usize,
    env_var: &str,
) -> PathBuf {
    let sanitized_env_var = env_var
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_ascii_lowercase();
    session_runtime_secret_dir(storage, session_record_id)
        .join(format!("{ordinal:02}-{sanitized_env_var}.secret"))
}

pub(super) fn cleanup_session_runtime_secret_artifacts(
    storage: &StorageInfo,
    session_record_id: i64,
) {
    let secret_dir = session_runtime_secret_dir(storage, session_record_id);

    if !secret_dir.exists() {
        return;
    }

    if let Err(error) = fs::remove_dir_all(&secret_dir) {
        log::warn!(
            "failed to remove session runtime secret directory {}: {error}",
            secret_dir.display()
        );
    }
}

fn project_commander_mcp_config_dir(storage: &StorageInfo) -> PathBuf {
    PathBuf::from(&storage.app_data_dir).join("mcp-config")
}

fn session_runtime_secret_root_dir(storage: &StorageInfo) -> PathBuf {
    PathBuf::from(&storage.app_data_dir)
        .join("runtime")
        .join("session-secrets")
}
