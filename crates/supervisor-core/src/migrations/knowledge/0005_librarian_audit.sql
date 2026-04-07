-- Librarian audit tables (EMERY-226.001).
-- Every capture pipeline run gets a row in librarian_runs;
-- every candidate grain considered (kept or dropped) gets a row in librarian_candidates.
-- The audit log is a first-class falsifiability instrument: it must always be possible
-- to answer "why did this memory get in / why didn't this one" — forever.

CREATE TABLE librarian_runs (
  id              TEXT    PRIMARY KEY,
  session_id      TEXT    NOT NULL,
  namespace       TEXT    NOT NULL,
  triage_score    INTEGER,           -- 0..3, NULL if triage failed
  triage_reason   TEXT,
  prompt_versions TEXT    NOT NULL,  -- JSON: {"triage":"v1","extract":"v1","critic":"v1"}
  status          TEXT    NOT NULL,  -- queued|running|done|failed|skipped
  started_at      INTEGER NOT NULL,
  finished_at     INTEGER,
  failure_reason  TEXT
);

CREATE TABLE librarian_candidates (
  id                TEXT    PRIMARY KEY,
  run_id            TEXT    NOT NULL REFERENCES librarian_runs(id),
  grain_type        TEXT    NOT NULL,  -- decision|insight|open_question|contradiction
  content           TEXT    NOT NULL,
  evidence_quote    TEXT    NOT NULL,  -- verbatim from transcript
  evidence_offset   INTEGER,
  critic_verdict    TEXT,              -- keep|drop|NULL (not yet judged)
  critic_reason     TEXT,
  reconcile_action  TEXT,              -- ADD|UPDATE|SUPERSEDE|NOOP|NULL
  written_memory_id TEXT REFERENCES memories(id),
  created_at        INTEGER NOT NULL
);

CREATE INDEX idx_librarian_runs_session     ON librarian_runs(session_id);
CREATE INDEX idx_librarian_candidates_run   ON librarian_candidates(run_id);
