CREATE TABLE mcp_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    server_type TEXT NOT NULL DEFAULT 'stdio',
    command TEXT NULL,
    args_json TEXT NULL,
    env_json TEXT NULL,
    url TEXT NULL,
    is_builtin INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
