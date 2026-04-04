CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL,
    default_account_id TEXT NULL,
    settings_json TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER NULL
);

CREATE INDEX idx_projects_sort_order ON projects(sort_order);
CREATE INDEX idx_projects_slug ON projects(slug);

CREATE TABLE project_roots (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    label TEXT NOT NULL,
    path TEXT NOT NULL,
    git_root_path TEXT NULL,
    remote_url TEXT NULL,
    root_kind TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER NULL,
    UNIQUE(project_id, path),
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

CREATE INDEX idx_project_roots_project_sort ON project_roots(project_id, sort_order);
CREATE INDEX idx_project_roots_git_root ON project_roots(git_root_path);

CREATE TABLE env_presets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    env_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    agent_kind TEXT NOT NULL,
    label TEXT NOT NULL,
    binary_path TEXT NULL,
    config_root TEXT NULL,
    env_preset_ref TEXT NULL,
    is_default INTEGER NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(env_preset_ref) REFERENCES env_presets(id)
);

CREATE INDEX idx_accounts_agent_kind ON accounts(agent_kind);
CREATE INDEX idx_accounts_default ON accounts(is_default);

CREATE TABLE worktrees (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    project_root_id TEXT NOT NULL,
    branch_name TEXT NOT NULL,
    head_commit TEXT NULL,
    base_ref TEXT NULL,
    path TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    created_by_session_id TEXT NULL,
    last_used_at INTEGER NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    closed_at INTEGER NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id),
    FOREIGN KEY(project_root_id) REFERENCES project_roots(id)
);

CREATE INDEX idx_worktrees_project ON worktrees(project_id);
CREATE INDEX idx_worktrees_root_status ON worktrees(project_root_id, status);
CREATE INDEX idx_worktrees_last_used ON worktrees(last_used_at);

CREATE TABLE session_specs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    project_root_id TEXT NULL,
    worktree_id TEXT NULL,
    work_item_id TEXT NULL,
    account_id TEXT NOT NULL,
    agent_kind TEXT NOT NULL,
    cwd TEXT NOT NULL,
    command TEXT NOT NULL,
    args_json TEXT NOT NULL,
    env_preset_ref TEXT NULL,
    origin_mode TEXT NOT NULL,
    title_policy TEXT NOT NULL,
    restore_policy TEXT NOT NULL,
    initial_terminal_cols INTEGER NOT NULL,
    initial_terminal_rows INTEGER NOT NULL,
    context_bundle_artifact_id TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id),
    FOREIGN KEY(project_root_id) REFERENCES project_roots(id),
    FOREIGN KEY(worktree_id) REFERENCES worktrees(id),
    FOREIGN KEY(account_id) REFERENCES accounts(id),
    FOREIGN KEY(env_preset_ref) REFERENCES env_presets(id)
);

CREATE INDEX idx_session_specs_project ON session_specs(project_id);
CREATE INDEX idx_session_specs_work_item ON session_specs(work_item_id);
CREATE INDEX idx_session_specs_worktree ON session_specs(worktree_id);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    session_spec_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    project_root_id TEXT NULL,
    worktree_id TEXT NULL,
    work_item_id TEXT NULL,
    account_id TEXT NOT NULL,
    agent_kind TEXT NOT NULL,
    origin_mode TEXT NOT NULL,
    current_mode TEXT NOT NULL,
    title TEXT NULL,
    title_source TEXT NOT NULL,
    user_prompt_count INTEGER NOT NULL,
    next_title_refresh_at_prompt_count INTEGER NULL,
    runtime_state TEXT NOT NULL,
    status TEXT NOT NULL,
    activity_state TEXT NOT NULL,
    needs_input_reason TEXT NULL,
    pty_owner_key TEXT NULL,
    cwd TEXT NOT NULL,
    transcript_primary_artifact_id TEXT NULL,
    raw_log_artifact_id TEXT NULL,
    started_at INTEGER NULL,
    ended_at INTEGER NULL,
    last_output_at INTEGER NULL,
    last_attached_at INTEGER NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER NULL,
    FOREIGN KEY(session_spec_id) REFERENCES session_specs(id),
    FOREIGN KEY(project_id) REFERENCES projects(id),
    FOREIGN KEY(project_root_id) REFERENCES project_roots(id),
    FOREIGN KEY(worktree_id) REFERENCES worktrees(id),
    FOREIGN KEY(account_id) REFERENCES accounts(id)
);

CREATE INDEX idx_sessions_project_created ON sessions(project_id, created_at DESC);
CREATE INDEX idx_sessions_origin_mode ON sessions(origin_mode);
CREATE INDEX idx_sessions_current_mode ON sessions(current_mode);
CREATE INDEX idx_sessions_runtime_state ON sessions(runtime_state);
CREATE INDEX idx_sessions_status ON sessions(status);
CREATE INDEX idx_sessions_work_item ON sessions(work_item_id);
CREATE INDEX idx_sessions_last_attached ON sessions(last_attached_at);

CREATE TABLE session_artifacts (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    artifact_class TEXT NOT NULL,
    artifact_type TEXT NOT NULL,
    path TEXT NOT NULL,
    is_durable INTEGER NOT NULL,
    is_primary INTEGER NOT NULL,
    source TEXT NULL,
    generator_ref TEXT NULL,
    supersedes_artifact_id TEXT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(session_id) REFERENCES sessions(id),
    FOREIGN KEY(supersedes_artifact_id) REFERENCES session_artifacts(id)
);

CREATE INDEX idx_session_artifacts_session_class ON session_artifacts(session_id, artifact_class);
CREATE INDEX idx_session_artifacts_primary ON session_artifacts(session_id, artifact_type, is_primary);

CREATE TABLE planning_assignments (
    id TEXT PRIMARY KEY,
    work_item_id TEXT NOT NULL,
    cadence_type TEXT NOT NULL,
    cadence_key TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    removed_at INTEGER NULL
);

CREATE INDEX idx_planning_assignments_work_item ON planning_assignments(work_item_id);
CREATE INDEX idx_planning_assignments_cadence ON planning_assignments(cadence_type, cadence_key);

CREATE TABLE workspace_state (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    saved_at INTEGER NOT NULL
);

CREATE TABLE workflow_reconciliation_proposals (
    id TEXT PRIMARY KEY,
    source_session_id TEXT NOT NULL,
    target_entity_type TEXT NOT NULL,
    target_entity_id TEXT NULL,
    proposal_type TEXT NOT NULL,
    proposed_change_payload TEXT NOT NULL,
    reason TEXT NOT NULL,
    confidence REAL NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    resolved_at INTEGER NULL,
    FOREIGN KEY(source_session_id) REFERENCES sessions(id)
);

CREATE INDEX idx_reconciliation_source_session ON workflow_reconciliation_proposals(source_session_id);
CREATE INDEX idx_reconciliation_status ON workflow_reconciliation_proposals(status);
