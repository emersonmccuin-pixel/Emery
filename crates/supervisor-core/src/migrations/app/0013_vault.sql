CREATE TABLE vault_entries (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    key TEXT NOT NULL,
    encrypted_value BLOB NOT NULL,
    description TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(scope, key)
);

CREATE INDEX idx_vault_entries_scope ON vault_entries(scope);

CREATE TABLE vault_audit_log (
    id TEXT PRIMARY KEY,
    entry_id TEXT NULL,
    action TEXT NOT NULL,
    actor TEXT NOT NULL,
    details_json TEXT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_vault_audit_log_entry ON vault_audit_log(entry_id);
