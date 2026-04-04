ALTER TABLE workflow_reconciliation_proposals
    ADD COLUMN work_item_id TEXT NULL;

ALTER TABLE workflow_reconciliation_proposals
    ADD COLUMN updated_at INTEGER NULL;

UPDATE workflow_reconciliation_proposals
SET
    work_item_id = NULL,
    updated_at = created_at
WHERE updated_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_reconciliation_work_item
    ON workflow_reconciliation_proposals(work_item_id);

CREATE INDEX IF NOT EXISTS idx_reconciliation_target
    ON workflow_reconciliation_proposals(target_entity_type, target_entity_id);
