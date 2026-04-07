//! Audit-log helpers for the librarian pipeline (EMERY-226.001).
//!
//! Thin wrappers over the `DatabaseSet::*_librarian_*` methods. The point of
//! this module is to centralize ID generation, timestamping, and the
//! status-string vocabulary so the orchestrator can focus on flow control.
//!
//! Status vocabulary for `librarian_runs.status`:
//!
//!   - "running"          — row inserted, pipeline in flight
//!   - "skipped_triage"   — triage scored 0; pipeline short-circuited
//!   - "completed"        — pipeline finished, zero or more memories written
//!   - "failed_triage"    — triage LLM/parse error
//!   - "failed_extract"   — extract LLM/parse error
//!   - "failed_critic"    — critic LLM/parse error
//!   - "failed_reconcile" — reconciler error on at least one survivor

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use uuid::Uuid;

use crate::models::{NewLibrarianCandidateRecord, NewLibrarianRunRecord};
use crate::store::DatabaseSet;

use crate::librarian::extract::ValidatedCandidate;
use crate::librarian::prompts::current_versions_json;

pub fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs() as i64
}

pub fn new_run_id() -> String {
    format!("lrun_{}", Uuid::new_v4().simple())
}

pub fn new_candidate_id() -> String {
    format!("lcan_{}", Uuid::new_v4().simple())
}

/// Insert the initial `librarian_runs` row in the "running" state. Called
/// once per session capture, before any LLM call.
pub fn start_run(
    databases: &DatabaseSet,
    session_id: &str,
    namespace: &str,
) -> Result<String> {
    let run_id = new_run_id();
    let record = NewLibrarianRunRecord {
        id: run_id.clone(),
        session_id: session_id.to_string(),
        namespace: namespace.to_string(),
        triage_score: None,
        triage_reason: None,
        prompt_versions: current_versions_json(),
        status: "running".to_string(),
        started_at: now_seconds(),
        finished_at: None,
        failure_reason: None,
    };
    databases.insert_librarian_run(&record)?;
    Ok(run_id)
}

/// Record the triage verdict on a run row.
pub fn record_triage(
    databases: &DatabaseSet,
    run_id: &str,
    score: i64,
    reason: &str,
) -> Result<()> {
    databases.update_librarian_run_triage(run_id, score, reason)
}

/// Insert one `librarian_candidates` row for a validated extractor output.
/// Returns the candidate id so the orchestrator can update it after the
/// critic and reconciler stages.
pub fn record_extracted_candidate(
    databases: &DatabaseSet,
    run_id: &str,
    candidate: &ValidatedCandidate,
) -> Result<String> {
    let id = new_candidate_id();
    let record = NewLibrarianCandidateRecord {
        id: id.clone(),
        run_id: run_id.to_string(),
        grain_type: candidate.candidate.grain_type.clone(),
        content: candidate.candidate.content.clone(),
        evidence_quote: candidate.candidate.evidence_quote.clone(),
        evidence_offset: Some(candidate.evidence_offset),
        critic_verdict: None,
        critic_reason: None,
        reconcile_action: None,
        written_memory_id: None,
        created_at: now_seconds(),
    };
    databases.insert_librarian_candidate(&record)?;
    Ok(id)
}

/// Mark a candidate as kept (or implicitly dropped) by the critic.
pub fn record_critic_verdict(
    databases: &DatabaseSet,
    candidate_id: &str,
    kept: bool,
    reason: Option<&str>,
) -> Result<()> {
    let verdict = if kept { "keep" } else { "drop" };
    databases.update_librarian_candidate_critic(candidate_id, verdict, reason)
}

/// Mark a candidate's reconciliation action and (if it produced one) the
/// resulting memory id.
pub fn record_reconciliation(
    databases: &DatabaseSet,
    candidate_id: &str,
    action: &str,
    written_memory_id: Option<&str>,
) -> Result<()> {
    databases.update_librarian_candidate_reconcile(candidate_id, action, written_memory_id)
}

/// Finalize a run row with the given status. `failure_reason` is None on
/// the success paths.
pub fn finish_run(
    databases: &DatabaseSet,
    run_id: &str,
    status: &str,
    failure_reason: Option<&str>,
) -> Result<()> {
    databases.finalize_librarian_run(run_id, status, now_seconds(), failure_reason)
}

/// Convenience: helper for use in tests and assertions to count rows.
#[cfg(test)]
pub fn count_runs_for_session(databases: &DatabaseSet, session_id: &str) -> Result<i64> {
    databases.count_librarian_runs_for_session(session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_id_has_expected_prefix() {
        assert!(new_run_id().starts_with("lrun_"));
    }

    #[test]
    fn candidate_id_has_expected_prefix() {
        assert!(new_candidate_id().starts_with("lcan_"));
    }

    #[test]
    fn now_seconds_is_after_2024() {
        assert!(now_seconds() > 1_700_000_000);
    }
}

