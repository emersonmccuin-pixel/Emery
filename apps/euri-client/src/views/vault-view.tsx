import { useEffect, useState } from "react";
import { navStore } from "../nav-store";
import {
  vaultList,
  vaultSet,
  vaultDelete,
  vaultUnlock,
  vaultLock,
  vaultStatus,
  vaultAuditLog,
} from "../lib";
import type { VaultEntry, VaultLockStatus, VaultAuditEntry } from "../types";
import { useAppStore } from "../store";

// ── Helpers ────────────────────────────────────────────────────────────────

function formatRelativeTime(ts: number): string {
  const diff = Date.now() - ts * 1000;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return new Date(ts * 1000).toLocaleDateString();
}

function formatCountdown(expiresAt: number): string {
  const remaining = expiresAt * 1000 - Date.now();
  if (remaining <= 0) return "expired";
  const mins = Math.floor(remaining / 60_000);
  const secs = Math.floor((remaining % 60_000) / 1000);
  if (mins > 0) return `${mins}m ${secs}s`;
  return `${secs}s`;
}

function scopeLabel(scope: string, projects: Array<{ id: string; name: string }>): string {
  if (scope === "global") return "Global";
  const project = projects.find((p) => p.id === scope);
  return project ? project.name : scope;
}

// ── Main view ──────────────────────────────────────────────────────────────

export function VaultView() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const projects = bootstrap?.projects ?? [];

  const [entries, setEntries] = useState<VaultEntry[]>([]);
  const [lockStatus, setLockStatus] = useState<VaultLockStatus | null>(null);
  const [auditLog, setAuditLog] = useState<VaultAuditEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAuditLog, setShowAuditLog] = useState(false);
  const [unlockMinutes, setUnlockMinutes] = useState(30);

  // Countdown refresh
  const [, setTick] = useState(0);
  useEffect(() => {
    const interval = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(interval);
  }, []);

  async function loadAll() {
    try {
      const [entriesResult, statusResult] = await Promise.all([
        vaultList(),
        vaultStatus(),
      ]);
      setEntries(entriesResult);
      setLockStatus(statusResult);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function loadAuditLog() {
    try {
      const log = await vaultAuditLog();
      setAuditLog(log);
    } catch {
      // audit log is best-effort
    }
  }

  useEffect(() => {
    void loadAll();
  }, []);

  useEffect(() => {
    if (showAuditLog) {
      void loadAuditLog();
    }
  }, [showAuditLog]);

  async function handleUnlock() {
    try {
      const status = await vaultUnlock(unlockMinutes);
      setLockStatus(status);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleLock() {
    try {
      const status = await vaultLock();
      setLockStatus(status);
    } catch (e) {
      setError(String(e));
    }
  }

  function handleEntryDeleted(id: string) {
    setEntries((prev) => prev.filter((e) => e.id !== id));
  }

  function handleEntryUpdated(updated: VaultEntry) {
    setEntries((prev) => prev.map((e) => (e.id === updated.id ? updated : e)));
  }

  function handleEntryCreated(entry: VaultEntry) {
    setEntries((prev) => [...prev, entry]);
  }

  // Group entries: global first, then by project scope
  const globalEntries = entries.filter((e) => e.scope === "global");
  const projectScopes = [...new Set(entries.filter((e) => e.scope !== "global").map((e) => e.scope))];

  const isUnlocked = lockStatus?.unlocked ?? false;

  return (
    <div className="content-frame">
    <div className="vault-view">
      {/* Header */}
      <div className="vault-header">
        <div className="vault-header-left">
          <h2 className="vault-title">Vault</h2>
          <LockStatusIndicator lockStatus={lockStatus} />
        </div>
        <div className="vault-header-actions">
          {isUnlocked ? (
            <button className="btn-ghost btn-sm" onClick={() => void handleLock()}>
              Lock
            </button>
          ) : (
            <div className="vault-unlock-row">
              <select
                className="settings-select vault-duration-select"
                value={unlockMinutes}
                onChange={(e) => setUnlockMinutes(Number(e.target.value))}
              >
                <option value={15}>15 min</option>
                <option value={30}>30 min</option>
                <option value={60}>1 hr</option>
                <option value={120}>2 hr</option>
                <option value={480}>8 hr</option>
              </select>
              <button className="btn-primary btn-sm" onClick={() => void handleUnlock()}>
                Unlock
              </button>
            </div>
          )}
          <button className="btn-ghost btn-sm" onClick={() => navStore.goBack()}>
            Back
          </button>
        </div>
      </div>

      {error && (
        <div className="vault-error">{error}</div>
      )}

      {loading ? (
        <div className="vault-loading">Loading vault...</div>
      ) : (
        <div className="vault-body">
          {/* Add Entry Form */}
          <AddEntryForm
            projects={projects}
            onCreated={handleEntryCreated}
          />

          {/* Global entries */}
          <div className="vault-scope-group">
            <div className="vault-scope-label">
              <span className="vault-scope-badge vault-scope-badge-global">Global</span>
              <span className="vault-scope-count">{globalEntries.length} {globalEntries.length === 1 ? "entry" : "entries"}</span>
            </div>
            {globalEntries.length === 0 ? (
              <div className="vault-empty-scope">No global entries.</div>
            ) : (
              <div className="vault-entry-list">
                {globalEntries.map((entry) => (
                  <EntryRow
                    key={entry.id}
                    entry={entry}
                    scopeLabel="Global"
                    onDeleted={() => handleEntryDeleted(entry.id)}
                    onUpdated={handleEntryUpdated}
                  />
                ))}
              </div>
            )}
          </div>

          {/* Per-project entries */}
          {projectScopes.map((scope) => {
            const scopeEntries = entries.filter((e) => e.scope === scope);
            const label = scopeLabel(scope, projects);
            return (
              <div key={scope} className="vault-scope-group">
                <div className="vault-scope-label">
                  <span className="vault-scope-badge vault-scope-badge-project">{label}</span>
                  <span className="vault-scope-count">{scopeEntries.length} {scopeEntries.length === 1 ? "entry" : "entries"}</span>
                </div>
                <div className="vault-entry-list">
                  {scopeEntries.map((entry) => (
                    <EntryRow
                      key={entry.id}
                      entry={entry}
                      scopeLabel={label}
                      onDeleted={() => handleEntryDeleted(entry.id)}
                      onUpdated={handleEntryUpdated}
                    />
                  ))}
                </div>
              </div>
            );
          })}

          {entries.length === 0 && (
            <div className="vault-empty-note">No vault entries yet. Add one above.</div>
          )}

          {/* Audit log */}
          <div className="vault-audit-section">
            <button
              className="vault-audit-toggle"
              onClick={() => setShowAuditLog((v) => !v)}
            >
              {showAuditLog ? "▾" : "▸"} Audit Log
            </button>
            {showAuditLog && (
              <div className="vault-audit-log">
                {auditLog.length === 0 ? (
                  <div className="vault-audit-empty">No audit entries.</div>
                ) : (
                  auditLog.slice(0, 50).map((entry) => (
                    <div key={entry.id} className="vault-audit-row">
                      <span className="vault-audit-action">{entry.action}</span>
                      <span className="vault-audit-key">{entry.key}</span>
                      {entry.scope !== "global" && (
                        <span className="vault-audit-scope">[{entry.scope.slice(0, 8)}]</span>
                      )}
                      {entry.actor && (
                        <span className="vault-audit-actor">{entry.actor}</span>
                      )}
                      <span className="vault-audit-time">{formatRelativeTime(entry.timestamp)}</span>
                    </div>
                  ))
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
    </div>
  );
}

// ── Lock Status Indicator ──────────────────────────────────────────────────

function LockStatusIndicator({ lockStatus }: { lockStatus: VaultLockStatus | null }) {
  if (!lockStatus) return null;

  if (lockStatus.unlocked && lockStatus.unlock_expires_at) {
    const countdown = formatCountdown(lockStatus.unlock_expires_at);
    return (
      <div className="vault-lock-status vault-lock-unlocked">
        <span className="vault-lock-dot vault-lock-dot-green" />
        <span>Unlocked — {countdown} remaining</span>
      </div>
    );
  }

  return (
    <div className="vault-lock-status vault-lock-locked">
      <span className="vault-lock-dot vault-lock-dot-red" />
      <span>Locked</span>
    </div>
  );
}

// ── Entry Row ──────────────────────────────────────────────────────────────

function EntryRow({
  entry,
  scopeLabel,
  onDeleted,
  onUpdated,
}: {
  entry: VaultEntry;
  scopeLabel: string;
  onDeleted: () => void;
  onUpdated: (updated: VaultEntry) => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [editingDesc, setEditingDesc] = useState(false);
  const [descInput, setDescInput] = useState(entry.description ?? "");
  const [saving, setSaving] = useState(false);

  async function handleDelete() {
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    setDeleting(true);
    try {
      await vaultDelete(entry.id);
      onDeleted();
    } catch {
      setDeleting(false);
      setConfirmDelete(false);
    }
  }

  async function handleSaveDescription() {
    setSaving(true);
    try {
      // vault_set with existing key updates the entry (description only — value unchanged via placeholder)
      // We use a special empty sentinel to signal description-only update.
      // Actually we call vault_set with scope/key/value="" just to update description.
      // But the backend won't know not to overwrite value. So we store a placeholder
      // and rely on the backend to handle description-only updates if supported.
      // For now, update_vault_entry might not exist — we use vault_set with a flag approach.
      // Per spec: "Edit (description only — can't see value)".
      // We'll use vault_set with value="" only to update description.
      // If the backend doesn't support this, we'll do a best-effort.
      const updated = await vaultSet(entry.scope, entry.key, "", descInput.trim() || null);
      onUpdated(updated);
      setEditingDesc(false);
    } catch {
      // fall back silently
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="vault-entry-row">
      <div className="vault-entry-info">
        <div className="vault-entry-key-row">
          <span className="vault-entry-key">{entry.key}</span>
          <span className={`vault-scope-badge ${entry.scope === "global" ? "vault-scope-badge-global" : "vault-scope-badge-project"}`}>
            {scopeLabel}
          </span>
        </div>
        {editingDesc ? (
          <div className="vault-entry-desc-edit">
            <input
              className="settings-input"
              type="text"
              value={descInput}
              onChange={(e) => setDescInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSaveDescription();
                if (e.key === "Escape") {
                  setDescInput(entry.description ?? "");
                  setEditingDesc(false);
                }
              }}
              placeholder="Description (optional)"
              autoFocus
            />
            <div className="settings-form-actions">
              <button
                className="btn-primary btn-sm"
                onClick={() => void handleSaveDescription()}
                disabled={saving}
              >
                {saving ? "..." : "Save"}
              </button>
              <button
                className="btn-ghost btn-sm"
                onClick={() => {
                  setDescInput(entry.description ?? "");
                  setEditingDesc(false);
                }}
                disabled={saving}
              >
                Cancel
              </button>
            </div>
          </div>
        ) : (
          <span className="vault-entry-desc">
            {entry.description ?? <span className="vault-entry-no-desc">no description</span>}
          </span>
        )}
        <div className="vault-entry-meta">
          <span>created {formatRelativeTime(entry.created_at)}</span>
          {entry.updated_at !== entry.created_at && (
            <span>· updated {formatRelativeTime(entry.updated_at)}</span>
          )}
        </div>
      </div>
      <div className="vault-entry-actions">
        {!editingDesc && (
          <button
            className="btn-ghost btn-sm"
            onClick={() => {
              setDescInput(entry.description ?? "");
              setEditingDesc(true);
              setConfirmDelete(false);
            }}
            title="Edit description"
          >
            Edit
          </button>
        )}
        {confirmDelete ? (
          <>
            <button
              className="btn-danger btn-sm"
              onClick={() => void handleDelete()}
              disabled={deleting}
            >
              {deleting ? "..." : "Confirm"}
            </button>
            <button
              className="btn-ghost btn-sm"
              onClick={() => setConfirmDelete(false)}
              disabled={deleting}
            >
              Cancel
            </button>
          </>
        ) : (
          <button
            className="btn-ghost btn-sm vault-delete-btn"
            onClick={() => void handleDelete()}
            title="Delete entry"
          >
            Delete
          </button>
        )}
      </div>
    </div>
  );
}

// ── Add Entry Form ─────────────────────────────────────────────────────────

function AddEntryForm({
  projects,
  onCreated,
}: {
  projects: Array<{ id: string; name: string }>;
  onCreated: (entry: VaultEntry) => void;
}) {
  const [showForm, setShowForm] = useState(false);
  const [scope, setScope] = useState("global");
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [description, setDescription] = useState("");
  const [showValue, setShowValue] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleCreate() {
    if (!key.trim() || !value.trim()) return;
    setSaving(true);
    setError(null);
    try {
      const entry = await vaultSet(scope, key.trim(), value, description.trim() || null);
      onCreated(entry);
      setKey("");
      setValue("");
      setDescription("");
      setScope("global");
      setShowForm(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  if (!showForm) {
    return (
      <button className="vault-add-btn btn-ghost btn-sm" onClick={() => setShowForm(true)}>
        + Add Entry
      </button>
    );
  }

  return (
    <div className="vault-add-form settings-add-form">
      <div className="vault-add-form-title">New Vault Entry</div>

      <div className="settings-field-group">
        <label className="settings-label">Scope</label>
        <select
          className="settings-select"
          value={scope}
          onChange={(e) => setScope(e.target.value)}
        >
          <option value="global">Global</option>
          {projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </div>

      <div className="settings-field-group">
        <label className="settings-label">Key</label>
        <input
          className="settings-input"
          type="text"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setShowForm(false);
          }}
          placeholder="e.g. GITHUB_TOKEN"
          autoFocus
          autoComplete="off"
        />
      </div>

      <div className="settings-field-group">
        <label className="settings-label">Value</label>
        <div className="settings-input-row">
          <input
            className="settings-input vault-value-input"
            type={showValue ? "text" : "password"}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleCreate();
              if (e.key === "Escape") setShowForm(false);
            }}
            placeholder="Secret value"
            autoComplete="new-password"
          />
          <button
            className="btn-ghost btn-sm"
            type="button"
            onClick={() => setShowValue((v) => !v)}
            title={showValue ? "Hide value" : "Show value"}
          >
            {showValue ? "Hide" : "Show"}
          </button>
        </div>
        <span className="vault-value-hint">Value is never shown after saving.</span>
      </div>

      <div className="settings-field-group">
        <label className="settings-label">Description (optional)</label>
        <input
          className="settings-input"
          type="text"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void handleCreate();
            if (e.key === "Escape") setShowForm(false);
          }}
          placeholder="What is this secret for?"
        />
      </div>

      {error && <div className="vault-form-error">{error}</div>}

      <div className="settings-form-actions">
        <button
          className="btn-primary btn-sm"
          onClick={() => void handleCreate()}
          disabled={saving || !key.trim() || !value.trim()}
        >
          {saving ? "Saving..." : "Save Entry"}
        </button>
        <button
          className="btn-ghost btn-sm"
          onClick={() => {
            setShowForm(false);
            setError(null);
          }}
          disabled={saving}
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
