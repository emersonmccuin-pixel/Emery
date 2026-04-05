-- Account-level defaults
ALTER TABLE accounts ADD COLUMN default_safety_mode TEXT NULL;
ALTER TABLE accounts ADD COLUMN default_launch_args_json TEXT NULL;

-- Project-level overrides (keyed by agent_kind in JSON)
ALTER TABLE projects ADD COLUMN agent_safety_overrides_json TEXT NULL;
