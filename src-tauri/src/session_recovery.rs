use crate::db::{AppState, SessionRecord, WorktreeRecord};
use crate::supervisor_api::CrashRecoveryManifest;
use std::collections::HashMap;

pub fn build_crash_recovery_manifest(
    state: &AppState,
    reconciled: &[SessionRecord],
) -> Result<CrashRecoveryManifest, String> {
    let deduped_sessions = dedupe_recovery_sessions_by_target(reconciled);

    let interrupted_sessions: Vec<SessionRecord> = deduped_sessions
        .iter()
        .filter(|s| s.state == "interrupted")
        .cloned()
        .collect();

    let orphaned_sessions: Vec<SessionRecord> = deduped_sessions
        .iter()
        .filter(|s| s.state == "orphaned")
        .cloned()
        .collect();

    let mut affected_worktrees: Vec<WorktreeRecord> = Vec::new();
    let mut seen_worktree_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for session in &deduped_sessions {
        if let Some(worktree_id) = session.worktree_id {
            if seen_worktree_ids.insert(worktree_id) {
                match state.get_worktree(worktree_id) {
                    Ok(worktree) => affected_worktrees.push(worktree),
                    Err(error) => {
                        log::warn!(
                            "failed to load worktree {worktree_id} for crash manifest: {error}"
                        );
                    }
                }
            }
        }
    }

    let affected_work_items = state.list_in_progress_work_items().map_err(|error| {
        format!("failed to load in-progress work items for crash manifest: {error}")
    })?;

    Ok(CrashRecoveryManifest {
        was_crash: true,
        interrupted_sessions,
        orphaned_sessions,
        affected_worktrees,
        affected_work_items,
    })
}

pub fn dedupe_recovery_sessions_by_target(reconciled: &[SessionRecord]) -> Vec<SessionRecord> {
    let mut sessions_by_target: HashMap<(i64, Option<i64>), SessionRecord> = HashMap::new();

    for session in reconciled
        .iter()
        .filter(|session| session.state == "interrupted" || session.state == "orphaned")
    {
        let key = (session.project_id, session.worktree_id);
        match sessions_by_target.get(&key) {
            Some(existing) if !is_newer_recovery_session(session, existing) => {}
            _ => {
                sessions_by_target.insert(key, session.clone());
            }
        }
    }

    let mut deduped = sessions_by_target.into_values().collect::<Vec<_>>();
    deduped.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    deduped
}

fn is_newer_recovery_session(candidate: &SessionRecord, existing: &SessionRecord) -> bool {
    candidate.updated_at > existing.updated_at
        || (candidate.updated_at == existing.updated_at && candidate.id > existing.id)
}
