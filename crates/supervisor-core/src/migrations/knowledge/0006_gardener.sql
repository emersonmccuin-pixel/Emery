-- Gardener tables (EMERY-226.002).
-- The gardener is a propose-only curator: it suggests retirements, but never
-- modifies a memory's valid_to itself. Only emery_gardener_decide(approve)
-- retires a memory. The audit value of these tables is permanent — proposals
-- and decisions stay forever so we can answer "why is this gone?" months later.

CREATE TABLE gardener_runs (
  id              TEXT    PRIMARY KEY,
  namespace       TEXT    NOT NULL,
  prompt_version  TEXT    NOT NULL,
  status          TEXT    NOT NULL,    -- proposed|approved|rejected|partially_approved|failed
  proposed_count  INTEGER NOT NULL,
  approved_count  INTEGER,
  started_at      INTEGER NOT NULL,
  finished_at     INTEGER,
  failure_reason  TEXT
);

CREATE TABLE gardener_proposals (
  id              TEXT    PRIMARY KEY,
  run_id          TEXT    NOT NULL REFERENCES gardener_runs(id),
  memory_id       TEXT    NOT NULL,    -- no FK: memories may be retired/superseded later
  reason          TEXT    NOT NULL,
  user_decision   TEXT,                -- approve|reject|NULL
  decided_at      INTEGER,
  created_at      INTEGER NOT NULL
);

CREATE INDEX idx_gardener_runs_namespace  ON gardener_runs(namespace);
CREATE INDEX idx_gardener_runs_started_at ON gardener_runs(started_at);
CREATE INDEX idx_gardener_proposals_run   ON gardener_proposals(run_id);
CREATE INDEX idx_gardener_proposals_pending
  ON gardener_proposals(run_id)
  WHERE user_decision IS NULL;
