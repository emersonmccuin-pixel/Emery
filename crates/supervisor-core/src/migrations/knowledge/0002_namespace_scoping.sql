-- Add namespace column to work_items and documents.
-- Namespace is the primary scoping mechanism; project_id is kept for backwards compat.

ALTER TABLE work_items ADD COLUMN namespace TEXT NULL;
CREATE INDEX idx_work_items_namespace ON work_items(namespace);

ALTER TABLE documents ADD COLUMN namespace TEXT NULL;
CREATE INDEX idx_documents_namespace ON documents(namespace);
