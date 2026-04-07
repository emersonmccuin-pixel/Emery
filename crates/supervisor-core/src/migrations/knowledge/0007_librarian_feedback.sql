-- EMERY-226.003 — librarian feedback + per-namespace tuning knobs.
--
-- librarian_config: per-namespace knobs that the capture loop and gardener
-- read at the start of each run. Defaults are conservative and live in code
-- (`crate::librarian::config`); a missing row implies defaults.
--
-- memory_feedback: append-only feedback rows recorded by the user against
-- specific memories. `signal=noise` is the headline path: it sets the
-- memory's valid_to immediately AND records this row, so we have an audit
-- trail of "the librarian wrote this and the user said it was noise."

CREATE TABLE librarian_config (
  namespace            TEXT PRIMARY KEY,
  triage_min_score     INTEGER NOT NULL DEFAULT 1,    -- skip pipeline below this
  max_grains_per_run   INTEGER NOT NULL DEFAULT 5,    -- hard cap on extractor output
  gardener_cap_percent INTEGER NOT NULL DEFAULT 20,
  gardener_cooldown_h  INTEGER NOT NULL DEFAULT 24,
  updated_at           INTEGER NOT NULL
);

CREATE TABLE memory_feedback (
  id           TEXT PRIMARY KEY,
  memory_id    TEXT NOT NULL,                  -- forensic only; no FK so retired memories don't break audit reads
  run_id       TEXT,                           -- optional pointer back to the librarian run that produced it
  signal       TEXT NOT NULL,                  -- noise|valuable|wrong_type|wrong_content
  note         TEXT,
  created_at   INTEGER NOT NULL
);

CREATE INDEX idx_memory_feedback_memory ON memory_feedback(memory_id);
CREATE INDEX idx_memory_feedback_run    ON memory_feedback(run_id);
CREATE INDEX idx_memory_feedback_signal ON memory_feedback(signal);
