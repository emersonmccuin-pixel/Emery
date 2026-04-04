ALTER TABLE session_specs ADD COLUMN current_mode TEXT NOT NULL DEFAULT 'ad_hoc';
ALTER TABLE session_specs ADD COLUMN title TEXT NULL;
