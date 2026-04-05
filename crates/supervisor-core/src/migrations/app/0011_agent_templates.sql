ALTER TABLE projects ADD COLUMN project_type TEXT NULL;

CREATE TABLE agent_templates (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    template_key TEXT NOT NULL,
    label TEXT NOT NULL,
    origin_mode TEXT NOT NULL DEFAULT 'code',
    default_model TEXT NULL,
    instructions_md TEXT NULL,
    stop_rules_json TEXT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id),
    UNIQUE(project_id, template_key)
);
CREATE INDEX idx_agent_templates_project ON agent_templates(project_id);
CREATE INDEX idx_agent_templates_project_active ON agent_templates(project_id, archived_at) WHERE archived_at IS NULL;
