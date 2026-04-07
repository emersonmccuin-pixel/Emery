-- Episodic memory layer (EMERY-217.003).
-- Each row is an atomic, time-aware fact with optional embedding and supersession chain.
-- valid_to IS NULL means the memory is currently valid.

CREATE TABLE memories (
  id               TEXT    PRIMARY KEY,
  namespace        TEXT    NOT NULL,
  content          TEXT    NOT NULL,
  source_ref       TEXT,           -- e.g. "session:abc123" or "wi:EMERY-217.002"
  embedding        BLOB,
  embedding_model  TEXT,
  input_hash       TEXT,
  valid_from       INTEGER NOT NULL,
  valid_to         INTEGER,        -- NULL = currently valid
  supersedes_id    TEXT    REFERENCES memories(id),
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL
);

CREATE INDEX idx_memories_namespace ON memories(namespace);
CREATE INDEX idx_memories_valid     ON memories(valid_to);
