CREATE UNIQUE INDEX IF NOT EXISTS idx_planning_assignments_unique_active
    ON planning_assignments(work_item_id, cadence_type, cadence_key)
    WHERE removed_at IS NULL;
