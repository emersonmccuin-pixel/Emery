//! End-to-end capture pipeline orchestrator (EMERY-226.001).
//!
//! Threads triage → extract → validator → critic → reconciliation → audit
//! through a single function. The orchestrator is intentionally decoupled
//! from the rest of the codebase via two trait abstractions:
//!
//!   - [`crate::librarian::client::ChatClient`] for LLM calls.
//!   - [`Reconciler`] for the final memory write.
//!
//! That decoupling lets unit tests drive the entire pipeline without a
//! database, an Anthropic key, or a Voyage key.
//!
//! ## Audit log invariant
//!
//! Every code path that touches the pipeline must end with a `librarian_runs`
//! row in a terminal status (not "running"). This is enforced by structuring
//! the body around `?` early-returns that all funnel through `finish_run`
//! via a single match at the bottom of `run_capture`.

use anyhow::Result;

use crate::librarian::audit;
use crate::librarian::client::ChatClient;
use crate::librarian::critic::run_critic;
use crate::librarian::extract::run_extract;
use crate::librarian::triage::run_triage;
use crate::store::DatabaseSet;

/// Final memory writer abstraction. Production wires this to
/// `SupervisorService::memory_add`; tests use a fake.
pub trait Reconciler: Send + Sync {
    /// Add a memory grain to the store and return `(memory_id, action_label)`.
    /// Action label is one of "ADD" / "UPDATE" / "SUPERSEDE" / "NOOP".
    fn add(
        &self,
        namespace: &str,
        content: &str,
        source_ref: Option<&str>,
    ) -> Result<(String, String), String>;
}

/// Outcome of a single capture run, useful to callers (e.g., session
/// completion handlers) and to assertions in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOutcome {
    pub run_id: String,
    pub triage_score: i64,
    pub status: String,
    /// Number of grains the extractor produced (post-validator).
    pub extracted: usize,
    /// Number of grains the critic kept.
    pub kept: usize,
    /// Number of grains successfully written to the memory store.
    pub written: usize,
}

/// Run the full capture pipeline for one session transcript.
///
/// This function never panics on LLM/parse failures: every error path is
/// recorded as a terminal `librarian_runs.status` row. The caller may
/// inspect [`CaptureOutcome::status`] to know what happened.
pub fn run_capture(
    databases: &DatabaseSet,
    client: &dyn ChatClient,
    reconciler: &dyn Reconciler,
    session_id: &str,
    namespace: &str,
    transcript: &str,
) -> Result<CaptureOutcome> {
    let run_id = audit::start_run(databases, session_id, namespace)?;

    // ── Stage 1: triage ──────────────────────────────────────────────────
    let triage = match run_triage(client, transcript) {
        Ok(t) => t,
        Err(e) => {
            audit::finish_run(databases, &run_id, "failed_triage", Some(&e))?;
            return Ok(CaptureOutcome {
                run_id,
                triage_score: -1,
                status: "failed_triage".to_string(),
                extracted: 0,
                kept: 0,
                written: 0,
            });
        }
    };
    audit::record_triage(databases, &run_id, triage.score, &triage.reason)?;

    if triage.score == 0 {
        audit::finish_run(databases, &run_id, "skipped_triage", None)?;
        return Ok(CaptureOutcome {
            run_id,
            triage_score: 0,
            status: "skipped_triage".to_string(),
            extracted: 0,
            kept: 0,
            written: 0,
        });
    }

    // ── Stage 2: extract (+ deterministic evidence-anchor validator) ─────
    let validated = match run_extract(client, transcript) {
        Ok(v) => v,
        Err(e) => {
            audit::finish_run(databases, &run_id, "failed_extract", Some(&e))?;
            return Ok(CaptureOutcome {
                run_id,
                triage_score: triage.score,
                status: "failed_extract".to_string(),
                extracted: 0,
                kept: 0,
                written: 0,
            });
        }
    };

    // Persist one row per validated candidate so the audit log captures the
    // pre-critic state. The orchestrator preserves order through critic and
    // reconciliation, so positional matching is safe.
    let mut row_ids: Vec<String> = Vec::with_capacity(validated.len());
    for vc in &validated {
        let id = audit::record_extracted_candidate(databases, &run_id, vc)?;
        row_ids.push(id);
    }

    // ── Stage 3: critic ──────────────────────────────────────────────────
    let kept = match run_critic(client, transcript, validated.clone()) {
        Ok(k) => k,
        Err(e) => {
            audit::finish_run(databases, &run_id, "failed_critic", Some(&e))?;
            return Ok(CaptureOutcome {
                run_id,
                triage_score: triage.score,
                status: "failed_critic".to_string(),
                extracted: validated.len(),
                kept: 0,
                written: 0,
            });
        }
    };

    // Mark each extracted candidate keep/drop based on whether it survived
    // the critic. Match by position-equivalent content+evidence: the kept
    // list is a subset of `validated` in original order.
    let mut kept_iter = kept.iter().peekable();
    for (i, vc) in validated.iter().enumerate() {
        let is_kept = matches!(
            kept_iter.peek(),
            Some(k)
                if k.candidate.candidate.content == vc.candidate.content
                    && k.candidate.candidate.evidence_quote == vc.candidate.evidence_quote
        );
        if is_kept {
            let k = kept_iter.next().unwrap();
            audit::record_critic_verdict(
                databases,
                &row_ids[i],
                true,
                Some(&k.critic_reason),
            )?;
        } else {
            audit::record_critic_verdict(databases, &row_ids[i], false, None)?;
        }
    }

    // ── Stage 4: reconciliation ──────────────────────────────────────────
    let mut written = 0usize;
    let mut had_failure = false;
    let mut last_failure: Option<String> = None;

    // Walk the kept survivors and find their original row_id by position.
    let mut kept_index = 0usize;
    for (i, vc) in validated.iter().enumerate() {
        if kept_index >= kept.len() {
            break;
        }
        let k = &kept[kept_index];
        if k.candidate.candidate.content != vc.candidate.content
            || k.candidate.candidate.evidence_quote != vc.candidate.evidence_quote
        {
            continue;
        }
        kept_index += 1;
        let source_ref = format!("session:{session_id}");
        match reconciler.add(namespace, &vc.candidate.content, Some(&source_ref)) {
            Ok((memory_id, action)) => {
                audit::record_reconciliation(databases, &row_ids[i], &action, Some(&memory_id))?;
                written += 1;
            }
            Err(e) => {
                had_failure = true;
                last_failure = Some(e.clone());
                audit::record_reconciliation(databases, &row_ids[i], "ERROR", None)?;
            }
        }
    }

    let status = if had_failure { "failed_reconcile" } else { "completed" };
    audit::finish_run(databases, &run_id, status, last_failure.as_deref())?;

    Ok(CaptureOutcome {
        run_id,
        triage_score: triage.score,
        status: status.to_string(),
        extracted: validated.len(),
        kept: kept.len(),
        written,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppPaths;
    use crate::librarian::client::FakeChatClient;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// In-process fake reconciler. Records every call so tests can assert
    /// what reached the memory store.
    struct FakeReconciler {
        calls: Mutex<Vec<(String, String)>>, // (namespace, content)
        next_id: Mutex<usize>,
        fail_after: Option<usize>,
    }

    impl FakeReconciler {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                next_id: Mutex::new(0),
                fail_after: None,
            }
        }
    }

    impl Reconciler for FakeReconciler {
        fn add(
            &self,
            namespace: &str,
            content: &str,
            _source_ref: Option<&str>,
        ) -> Result<(String, String), String> {
            let mut n = self.next_id.lock().unwrap();
            *n += 1;
            if let Some(limit) = self.fail_after {
                if *n > limit {
                    return Err("reconciler exploded".to_string());
                }
            }
            self.calls
                .lock()
                .unwrap()
                .push((namespace.to_string(), content.to_string()));
            Ok((format!("mem_fake_{}", *n), "ADD".to_string()))
        }
    }

    fn unique_temp_root() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "emery-librarian-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_db() -> (PathBuf, DatabaseSet) {
        let root = unique_temp_root();
        let paths = AppPaths::from_root(root.clone()).unwrap();
        let dbs = DatabaseSet::initialize(&paths).unwrap();
        (root, dbs)
    }

    #[test]
    fn triage_zero_short_circuits_with_audit_row() {
        let (_tmp, dbs) = make_db();
        let fake_chat =
            FakeChatClient::new(vec![Ok(r#"{"score":0,"reason":"routine"}"#.to_string())]);
        let recon = FakeReconciler::new();

        let outcome = run_capture(
            &dbs,
            &fake_chat,
            &recon,
            "sess_1",
            "EMERY",
            "did some work",
        )
        .unwrap();

        assert_eq!(outcome.status, "skipped_triage");
        assert_eq!(outcome.triage_score, 0);
        assert_eq!(outcome.written, 0);
        assert_eq!(audit::count_runs_for_session(&dbs, "sess_1").unwrap(), 1);
        assert!(recon.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn full_pipeline_writes_one_memory() {
        let (_tmp, dbs) = make_db();
        // 1) triage = 2
        // 2) extract returns one candidate with verbatim quote
        // 3) critic keeps it
        let triage = r#"{"score":2,"reason":"real decision"}"#.to_string();
        let extract = r#"[{"grain_type":"decision","content":"Use WAL","evidence_quote":"we'll use WAL"}]"#
            .to_string();
        let critic = r#"{"verdicts":[{"grain_index":0,"verdict":"keep","reason":"clear choice"}]}"#
            .to_string();
        let fake_chat = FakeChatClient::new(vec![Ok(triage), Ok(extract), Ok(critic)]);
        let recon = FakeReconciler::new();

        let transcript = "the team agreed: we'll use WAL mode for sure";
        let outcome = run_capture(&dbs, &fake_chat, &recon, "sess_2", "EMERY", transcript).unwrap();

        assert_eq!(outcome.status, "completed");
        assert_eq!(outcome.extracted, 1);
        assert_eq!(outcome.kept, 1);
        assert_eq!(outcome.written, 1);
        let calls = recon.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "EMERY");
        assert_eq!(calls[0].1, "Use WAL");
    }

    #[test]
    fn extractor_hallucinated_evidence_dropped_before_critic() {
        let (_tmp, dbs) = make_db();
        let triage = r#"{"score":2,"reason":"x"}"#.to_string();
        // Extractor produces one good and one fake-evidence candidate.
        let extract = r#"[
            {"grain_type":"decision","content":"keep me","evidence_quote":"verbatim phrase"},
            {"grain_type":"insight","content":"drop me","evidence_quote":"never said this"}
        ]"#
        .to_string();
        // Critic keeps the only candidate that reached it.
        let critic =
            r#"{"verdicts":[{"grain_index":0,"verdict":"keep","reason":"ok"}]}"#.to_string();
        let fake_chat = FakeChatClient::new(vec![Ok(triage), Ok(extract), Ok(critic)]);
        let recon = FakeReconciler::new();

        let outcome = run_capture(
            &dbs,
            &fake_chat,
            &recon,
            "sess_3",
            "EMERY",
            "they used a verbatim phrase here",
        )
        .unwrap();

        assert_eq!(outcome.extracted, 1, "validator should drop the fake one");
        assert_eq!(outcome.kept, 1);
        assert_eq!(outcome.written, 1);
        let calls = recon.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "keep me");
    }

    #[test]
    fn critic_drops_all_writes_zero_memories() {
        let (_tmp, dbs) = make_db();
        let triage = r#"{"score":2,"reason":"x"}"#.to_string();
        let extract =
            r#"[{"grain_type":"decision","content":"weak","evidence_quote":"weak phrase"}]"#
                .to_string();
        let critic =
            r#"{"verdicts":[{"grain_index":0,"verdict":"drop","reason":"vague"}]}"#.to_string();
        let fake_chat = FakeChatClient::new(vec![Ok(triage), Ok(extract), Ok(critic)]);
        let recon = FakeReconciler::new();

        let outcome = run_capture(&dbs, &fake_chat, &recon, "sess_4", "EMERY", "weak phrase").unwrap();

        assert_eq!(outcome.status, "completed");
        assert_eq!(outcome.kept, 0);
        assert_eq!(outcome.written, 0);
        assert!(recon.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn triage_failure_writes_failed_audit_row() {
        let (_tmp, dbs) = make_db();
        let fake_chat = FakeChatClient::new(vec![Err("rate_limited".to_string())]);
        let recon = FakeReconciler::new();

        let outcome = run_capture(&dbs, &fake_chat, &recon, "sess_5", "EMERY", "x").unwrap();

        assert_eq!(outcome.status, "failed_triage");
        assert_eq!(audit::count_runs_for_session(&dbs, "sess_5").unwrap(), 1);
    }

    #[test]
    fn extract_failure_writes_failed_audit_row() {
        let (_tmp, dbs) = make_db();
        let triage = r#"{"score":2,"reason":"x"}"#.to_string();
        let fake_chat =
            FakeChatClient::new(vec![Ok(triage), Err("model_overloaded".to_string())]);
        let recon = FakeReconciler::new();

        let outcome = run_capture(&dbs, &fake_chat, &recon, "sess_6", "EMERY", "x").unwrap();

        assert_eq!(outcome.status, "failed_extract");
        assert_eq!(audit::count_runs_for_session(&dbs, "sess_6").unwrap(), 1);
    }

    #[test]
    fn critic_failure_writes_failed_audit_row() {
        let (_tmp, dbs) = make_db();
        let triage = r#"{"score":2,"reason":"x"}"#.to_string();
        let extract =
            r#"[{"grain_type":"decision","content":"a","evidence_quote":"phrase"}]"#.to_string();
        let fake_chat = FakeChatClient::new(vec![
            Ok(triage),
            Ok(extract),
            Err("rate_limited".to_string()),
        ]);
        let recon = FakeReconciler::new();

        let outcome = run_capture(
            &dbs,
            &fake_chat,
            &recon,
            "sess_7",
            "EMERY",
            "the user said phrase",
        )
        .unwrap();

        assert_eq!(outcome.status, "failed_critic");
        assert_eq!(outcome.extracted, 1);
        assert_eq!(outcome.written, 0);
    }
}
