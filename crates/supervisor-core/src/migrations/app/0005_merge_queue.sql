CREATE TABLE merge_queue (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    worktree_id TEXT NOT NULL,
    branch_name TEXT NOT NULL,
    base_ref TEXT NOT NULL,
    position INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    diff_stat_json TEXT NULL,
    conflict_files_json TEXT NULL,
    has_uncommitted_changes INTEGER NOT NULL DEFAULT 0,
    queued_at INTEGER NOT NULL,
    merged_at INTEGER NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id),
    FOREIGN KEY(session_id) REFERENCES sessions(id),
    FOREIGN KEY(worktree_id) REFERENCES worktrees(id)
);

CREATE INDEX idx_merge_queue_project_status ON merge_queue(project_id, status);
CREATE INDEX idx_merge_queue_position ON merge_queue(project_id, position);
