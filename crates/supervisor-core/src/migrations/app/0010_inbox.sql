CREATE TABLE inbox_entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    session_id TEXT NULL,
    work_item_id TEXT NULL,
    worktree_id TEXT NULL,
    entry_type TEXT NOT NULL DEFAULT 'session_complete',
    title TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'success',
    branch_name TEXT NULL,
    diff_stat_json TEXT NULL,
    metadata_json TEXT NULL,
    read_at INTEGER NULL,
    resolved_at INTEGER NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id),
    FOREIGN KEY(session_id) REFERENCES sessions(id),
    FOREIGN KEY(worktree_id) REFERENCES worktrees(id)
);
CREATE INDEX idx_inbox_entries_project_status ON inbox_entries(project_id, status);
CREATE INDEX idx_inbox_entries_created ON inbox_entries(created_at DESC);
CREATE INDEX idx_inbox_entries_unread ON inbox_entries(read_at) WHERE read_at IS NULL;
CREATE INDEX idx_inbox_entries_session ON inbox_entries(session_id);
