-- Add embedding columns to work_items and documents.
-- All columns are nullable so existing rows keep NULL values; backfill on startup.

ALTER TABLE work_items ADD COLUMN embedding      BLOB NULL;
ALTER TABLE work_items ADD COLUMN embedding_model TEXT NULL;
ALTER TABLE work_items ADD COLUMN input_hash     TEXT NULL;
ALTER TABLE work_items ADD COLUMN embedded_at    INTEGER NULL;

ALTER TABLE documents  ADD COLUMN embedding      BLOB NULL;
ALTER TABLE documents  ADD COLUMN embedding_model TEXT NULL;
ALTER TABLE documents  ADD COLUMN input_hash     TEXT NULL;
ALTER TABLE documents  ADD COLUMN embedded_at    INTEGER NULL;
