//! User feedback on librarian-written memories (EMERY-226.003).
//!
//! This module is the *control plane* for the librarian:
//!
//!   - `record_feedback` lets the user say "this memory was noise" or "this
//!     was exactly the kind of thing I want." A `noise` signal retires the
//!     memory immediately AND records a feedback row, but never deletes the
//!     row from `memories` — the audit value of "the librarian wrote this
//!     and the user said it was noise" is the entire point.
//!   - `compute_metrics` rolls up health metrics across `librarian_runs`,
//!     `librarian_candidates`, `gardener_proposals`, and `memory_feedback`.
//!     The flagged-noise rate is the headline number; if it climbs above
//!     ~10% in a given week, the prompts are wrong.
//!
//! No LLM calls happen here. Everything is local SQL + bookkeeping.

use anyhow::{Result, anyhow};
use rusqlite::params;
use serde::Serialize;
use uuid::Uuid;

use crate::models::{MemoryFeedbackRow, NewMemoryFeedbackRecord};
use crate::store::{DatabaseSet, open_connection};

/// Allowed values for `memory_feedback.signal`.
pub const SIGNAL_NOISE: &str = "noise";
pub const SIGNAL_VALUABLE: &str = "valuable";
pub const SIGNAL_WRONG_TYPE: &str = "wrong_type";
pub const SIGNAL_WRONG_CONTENT: &str = "wrong_content";

pub const ALL_SIGNALS: &[&str] = &[
    SIGNAL_NOISE,
    SIGNAL_VALUABLE,
    SIGNAL_WRONG_TYPE,
    SIGNAL_WRONG_CONTENT,
];

/// Validate that `signal` is one of the four allowed strings.
pub fn validate_signal(signal: &str) -> Result<(), String> {
    if ALL_SIGNALS.contains(&signal) {
        Ok(())
    } else {
        Err(format!(
            "memory_feedback.signal must be one of {:?} (got {:?})",
            ALL_SIGNALS, signal
        ))
    }
}

/// Record a feedback row against a specific memory. If `signal=noise`, the
/// memory is also retired immediately (sets `valid_to = now`).
///
/// The memory row is **never** deleted, by design — the audit trail is the
/// whole point of this work item.
pub fn record_feedback(
    databases: &DatabaseSet,
    memory_id: &str,
    signal: &str,
    note: Option<&str>,
    now_unix: i64,
) -> Result<MemoryFeedbackRow> {
    validate_signal(signal).map_err(|e| anyhow!(e))?;

    // Make sure the memory exists; refuse to flag a phantom.
    let memory = databases
        .get_memory(memory_id)?
        .ok_or_else(|| anyhow!("memory {memory_id} not found"))?;

    // For noise: retire the memory immediately. Idempotent on already-retired
    // rows because expire_memory is a straight UPDATE.
    if signal == SIGNAL_NOISE {
        databases.expire_memory(memory_id, now_unix)?;
    }

    let record = NewMemoryFeedbackRecord {
        id: format!("mfb_{}", Uuid::new_v4().simple()),
        memory_id: memory.id.clone(),
        run_id: None, // future: thread the source librarian_run id through
        signal: signal.to_string(),
        note: note.map(|s| s.to_string()),
        created_at: now_unix,
    };
    databases.insert_memory_feedback(&record)?;

    let rows = databases.list_memory_feedback_for_memory(memory_id)?;
    rows.into_iter()
        .find(|r| r.id == record.id)
        .ok_or_else(|| anyhow!("memory_feedback row {} vanished after insert", record.id))
}

/// Headline metric numbers for the librarian, optionally scoped to a single
/// namespace and a since-window. All numerator/denominator pairs are
/// computed against the same window.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LibrarianMetrics {
    pub namespace: Option<String>,
    pub since_unix: i64,
    pub now_unix: i64,

    /// COUNT(librarian_runs WHERE status='completed')
    pub completed_runs: i64,
    /// COUNT(memories) created in window
    pub memories_written: i64,
    /// COUNT(librarian_candidates) in window
    pub candidates_total: i64,
    /// COUNT(librarian_candidates WHERE critic_verdict='drop') in window
    pub candidates_dropped_by_critic: i64,
    /// COUNT(gardener_proposals) in window
    pub gardener_proposals_total: i64,
    /// COUNT(gardener_proposals WHERE user_decision='approve') in window
    pub gardener_proposals_approved: i64,
    /// COUNT(memory_feedback WHERE signal='noise') in window
    pub noise_flags: i64,

    /// memories_written / completed_runs (None if no runs)
    pub capture_rate: Option<f64>,
    /// candidates_dropped_by_critic / candidates_total (None if no candidates)
    pub critic_drop_rate: Option<f64>,
    /// gardener_proposals_approved / decided proposals (None if no decisions)
    pub gardener_approval_rate: Option<f64>,
    /// noise_flags / memories_written (None if no memories)
    pub noise_flag_rate: Option<f64>,

    /// Per-prompt-version noise breakdown — built from
    /// `librarian_runs.prompt_versions` (a JSON blob) joined to candidates'
    /// written memories' feedback. Empty when nothing in window.
    pub prompt_version_health: Vec<PromptVersionHealth>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptVersionHealth {
    pub prompt_versions: String,
    pub written: i64,
    pub flagged_noise: i64,
    pub noise_rate: Option<f64>,
}

/// Compute the librarian metrics over `now - since_secs .. now`, optionally
/// scoped to a single namespace.
pub fn compute_metrics(
    databases: &DatabaseSet,
    namespace: Option<&str>,
    now_unix: i64,
    since_secs: i64,
) -> Result<LibrarianMetrics> {
    let since_unix = now_unix.saturating_sub(since_secs);
    let conn = open_connection(&databases.paths().knowledge_db)?;

    let ns_filter_runs = namespace.map(|ns| (ns.to_string(),));
    let ns_filter_mem = namespace.map(|ns| (ns.to_string(),));

    // completed_runs
    let completed_runs: i64 = match &ns_filter_runs {
        Some((ns,)) => conn.query_row(
            "SELECT COUNT(*) FROM librarian_runs
              WHERE status = 'completed' AND started_at >= ?1 AND namespace = ?2",
            params![since_unix, ns],
            |row| row.get(0),
        )?,
        None => conn.query_row(
            "SELECT COUNT(*) FROM librarian_runs
              WHERE status = 'completed' AND started_at >= ?1",
            params![since_unix],
            |row| row.get(0),
        )?,
    };

    // memories_written: created_at IN window, optionally namespace-scoped
    let memories_written: i64 = match &ns_filter_mem {
        Some((ns,)) => conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE created_at >= ?1 AND namespace = ?2",
            params![since_unix, ns],
            |row| row.get(0),
        )?,
        None => conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE created_at >= ?1",
            params![since_unix],
            |row| row.get(0),
        )?,
    };

    // candidates_total + candidates_dropped_by_critic — join through runs for
    // namespace filtering.
    let (candidates_total, candidates_dropped_by_critic): (i64, i64) = match namespace {
        Some(ns) => {
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM librarian_candidates c
                   JOIN librarian_runs r ON r.id = c.run_id
                  WHERE c.created_at >= ?1 AND r.namespace = ?2",
                params![since_unix, ns],
                |row| row.get(0),
            )?;
            let dropped: i64 = conn.query_row(
                "SELECT COUNT(*) FROM librarian_candidates c
                   JOIN librarian_runs r ON r.id = c.run_id
                  WHERE c.created_at >= ?1 AND r.namespace = ?2
                    AND c.critic_verdict = 'drop'",
                params![since_unix, ns],
                |row| row.get(0),
            )?;
            (total, dropped)
        }
        None => {
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM librarian_candidates WHERE created_at >= ?1",
                params![since_unix],
                |row| row.get(0),
            )?;
            let dropped: i64 = conn.query_row(
                "SELECT COUNT(*) FROM librarian_candidates
                  WHERE created_at >= ?1 AND critic_verdict = 'drop'",
                params![since_unix],
                |row| row.get(0),
            )?;
            (total, dropped)
        }
    };

    // gardener_proposals — join through gardener_runs for namespace filter
    let (gardener_proposals_total, gardener_proposals_approved, gardener_proposals_decided): (
        i64,
        i64,
        i64,
    ) = match namespace {
        Some(ns) => {
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM gardener_proposals p
                   JOIN gardener_runs r ON r.id = p.run_id
                  WHERE p.created_at >= ?1 AND r.namespace = ?2",
                params![since_unix, ns],
                |row| row.get(0),
            )?;
            let approved: i64 = conn.query_row(
                "SELECT COUNT(*) FROM gardener_proposals p
                   JOIN gardener_runs r ON r.id = p.run_id
                  WHERE p.created_at >= ?1 AND r.namespace = ?2
                    AND p.user_decision = 'approve'",
                params![since_unix, ns],
                |row| row.get(0),
            )?;
            let decided: i64 = conn.query_row(
                "SELECT COUNT(*) FROM gardener_proposals p
                   JOIN gardener_runs r ON r.id = p.run_id
                  WHERE p.created_at >= ?1 AND r.namespace = ?2
                    AND p.user_decision IS NOT NULL",
                params![since_unix, ns],
                |row| row.get(0),
            )?;
            (total, approved, decided)
        }
        None => {
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM gardener_proposals WHERE created_at >= ?1",
                params![since_unix],
                |row| row.get(0),
            )?;
            let approved: i64 = conn.query_row(
                "SELECT COUNT(*) FROM gardener_proposals
                  WHERE created_at >= ?1 AND user_decision = 'approve'",
                params![since_unix],
                |row| row.get(0),
            )?;
            let decided: i64 = conn.query_row(
                "SELECT COUNT(*) FROM gardener_proposals
                  WHERE created_at >= ?1 AND user_decision IS NOT NULL",
                params![since_unix],
                |row| row.get(0),
            )?;
            (total, approved, decided)
        }
    };

    // noise_flags — join feedback to memories for namespace filter
    let noise_flags: i64 = match namespace {
        Some(ns) => conn.query_row(
            "SELECT COUNT(*) FROM memory_feedback f
               JOIN memories m ON m.id = f.memory_id
              WHERE f.created_at >= ?1 AND m.namespace = ?2
                AND f.signal = 'noise'",
            params![since_unix, ns],
            |row| row.get(0),
        )?,
        None => conn.query_row(
            "SELECT COUNT(*) FROM memory_feedback
              WHERE created_at >= ?1 AND signal = 'noise'",
            params![since_unix],
            |row| row.get(0),
        )?,
    };

    // Per-prompt-version noise breakdown.
    // Group librarian_runs.prompt_versions; for each group, count memories
    // written in window and noise flags against those memories.
    let mut prompt_version_health = Vec::new();
    {
        let sql = match namespace {
            Some(_) => {
                "SELECT r.prompt_versions,
                        COUNT(DISTINCT c.written_memory_id) AS written,
                        SUM(CASE WHEN f.signal = 'noise' THEN 1 ELSE 0 END) AS flagged
                   FROM librarian_runs r
                   JOIN librarian_candidates c ON c.run_id = r.id
                                              AND c.written_memory_id IS NOT NULL
                   LEFT JOIN memory_feedback f ON f.memory_id = c.written_memory_id
                  WHERE r.started_at >= ?1 AND r.namespace = ?2
                  GROUP BY r.prompt_versions"
            }
            None => {
                "SELECT r.prompt_versions,
                        COUNT(DISTINCT c.written_memory_id) AS written,
                        SUM(CASE WHEN f.signal = 'noise' THEN 1 ELSE 0 END) AS flagged
                   FROM librarian_runs r
                   JOIN librarian_candidates c ON c.run_id = r.id
                                              AND c.written_memory_id IS NOT NULL
                   LEFT JOIN memory_feedback f ON f.memory_id = c.written_memory_id
                  WHERE r.started_at >= ?1
                  GROUP BY r.prompt_versions"
            }
        };
        let mut stmt = conn.prepare(sql)?;
        let rows: Vec<(String, i64, Option<i64>)> = match namespace {
            Some(ns) => stmt
                .query_map(params![since_unix, ns], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?,
            None => stmt
                .query_map(params![since_unix], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        };
        for (prompt_versions, written, flagged) in rows {
            let flagged = flagged.unwrap_or(0);
            let noise_rate = if written > 0 {
                Some(flagged as f64 / written as f64)
            } else {
                None
            };
            prompt_version_health.push(PromptVersionHealth {
                prompt_versions,
                written,
                flagged_noise: flagged,
                noise_rate,
            });
        }
    }

    let capture_rate = if completed_runs > 0 {
        Some(memories_written as f64 / completed_runs as f64)
    } else {
        None
    };
    let critic_drop_rate = if candidates_total > 0 {
        Some(candidates_dropped_by_critic as f64 / candidates_total as f64)
    } else {
        None
    };
    let gardener_approval_rate = if gardener_proposals_decided > 0 {
        Some(gardener_proposals_approved as f64 / gardener_proposals_decided as f64)
    } else {
        None
    };
    let noise_flag_rate = if memories_written > 0 {
        Some(noise_flags as f64 / memories_written as f64)
    } else {
        None
    };

    Ok(LibrarianMetrics {
        namespace: namespace.map(|s| s.to_string()),
        since_unix,
        now_unix,
        completed_runs,
        memories_written,
        candidates_total,
        candidates_dropped_by_critic,
        gardener_proposals_total,
        gardener_proposals_approved,
        noise_flags,
        capture_rate,
        critic_drop_rate,
        gardener_approval_rate,
        noise_flag_rate,
        prompt_version_health,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppPaths;
    use crate::models::{
        NewLibrarianCandidateRecord, NewLibrarianRunRecord, NewMemoryRecord,
    };
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "emery-feedback-test-{}-{}",
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

    fn insert_memory(dbs: &DatabaseSet, namespace: &str, content: &str, created_at: i64) -> String {
        let id = format!("mem_{}", Uuid::new_v4().simple());
        dbs.insert_memory(&NewMemoryRecord {
            id: id.clone(),
            namespace: namespace.to_string(),
            content: content.to_string(),
            source_ref: None,
            embedding: None,
            embedding_model: None,
            input_hash: None,
            valid_from: created_at,
            valid_to: None,
            supersedes_id: None,
            created_at,
            updated_at: created_at,
        })
        .unwrap();
        id
    }

    #[test]
    fn validate_signal_accepts_known_values() {
        for s in ALL_SIGNALS {
            assert!(validate_signal(s).is_ok());
        }
        assert!(validate_signal("garbage").is_err());
    }

    #[test]
    fn flag_noise_retires_memory_immediately() {
        let (_tmp, dbs) = make_db();
        let id = insert_memory(&dbs, "EMERY", "use WAL", 1_700_000_000);

        let row = record_feedback(&dbs, &id, SIGNAL_NOISE, Some("not actually a decision"), 1_700_000_001).unwrap();
        assert_eq!(row.signal, SIGNAL_NOISE);
        assert_eq!(row.note.as_deref(), Some("not actually a decision"));

        let mem = dbs.get_memory(&id).unwrap().unwrap();
        assert_eq!(mem.valid_to, Some(1_700_000_001));
    }

    #[test]
    fn flag_does_not_delete_memory_row() {
        let (_tmp, dbs) = make_db();
        let id = insert_memory(&dbs, "EMERY", "use WAL", 1_700_000_000);
        record_feedback(&dbs, &id, SIGNAL_NOISE, None, 1_700_000_001).unwrap();
        // Row still present.
        assert!(dbs.get_memory(&id).unwrap().is_some());
        // Feedback row exists.
        let rows = dbs.list_memory_feedback_for_memory(&id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].signal, SIGNAL_NOISE);
    }

    #[test]
    fn valuable_signal_does_not_retire() {
        let (_tmp, dbs) = make_db();
        let id = insert_memory(&dbs, "EMERY", "use WAL", 1_700_000_000);
        record_feedback(&dbs, &id, SIGNAL_VALUABLE, None, 1_700_000_001).unwrap();
        let mem = dbs.get_memory(&id).unwrap().unwrap();
        assert!(mem.valid_to.is_none());
    }

    #[test]
    fn flag_unknown_memory_errors() {
        let (_tmp, dbs) = make_db();
        let err = record_feedback(&dbs, "mem_does_not_exist", SIGNAL_NOISE, None, 1).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn flag_unknown_signal_errors() {
        let (_tmp, dbs) = make_db();
        let id = insert_memory(&dbs, "EMERY", "x", 1);
        let err = record_feedback(&dbs, &id, "garbage", None, 2).unwrap_err();
        assert!(err.to_string().contains("memory_feedback.signal"));
    }

    #[test]
    fn metrics_returns_zero_for_empty_store() {
        let (_tmp, dbs) = make_db();
        let m = compute_metrics(&dbs, Some("EMERY"), 1_700_000_000, 7 * 24 * 60 * 60).unwrap();
        assert_eq!(m.completed_runs, 0);
        assert_eq!(m.memories_written, 0);
        assert_eq!(m.candidates_total, 0);
        assert_eq!(m.noise_flags, 0);
        assert!(m.capture_rate.is_none());
        assert!(m.critic_drop_rate.is_none());
        assert!(m.gardener_approval_rate.is_none());
        assert!(m.noise_flag_rate.is_none());
        assert!(m.prompt_version_health.is_empty());
    }

    /// Seed two librarian runs with different prompt versions, each writing
    /// one memory. Flag one as noise. Assert per-prompt buckets surface.
    #[test]
    fn metrics_breaks_down_by_prompt_version() {
        let (_tmp, dbs) = make_db();
        let now = 1_700_000_000;

        let mem_v1 = insert_memory(&dbs, "EMERY", "v1 memory", now);
        let mem_v2 = insert_memory(&dbs, "EMERY", "v2 memory", now);

        // Run with prompt_versions = "v1" → wrote mem_v1
        dbs.insert_librarian_run(&NewLibrarianRunRecord {
            id: "lrun_v1".to_string(),
            session_id: "sess".to_string(),
            namespace: "EMERY".to_string(),
            triage_score: Some(2),
            triage_reason: Some("ok".to_string()),
            prompt_versions: "v1".to_string(),
            status: "completed".to_string(),
            started_at: now,
            finished_at: Some(now),
            failure_reason: None,
        })
        .unwrap();
        dbs.insert_librarian_candidate(&NewLibrarianCandidateRecord {
            id: "lcan_v1".to_string(),
            run_id: "lrun_v1".to_string(),
            grain_type: "decision".to_string(),
            content: "x".to_string(),
            evidence_quote: "x".to_string(),
            evidence_offset: Some(0),
            critic_verdict: Some("keep".to_string()),
            critic_reason: Some("ok".to_string()),
            reconcile_action: Some("ADD".to_string()),
            written_memory_id: Some(mem_v1.clone()),
            created_at: now,
        })
        .unwrap();

        // Run with prompt_versions = "v2" → wrote mem_v2, flagged noise
        dbs.insert_librarian_run(&NewLibrarianRunRecord {
            id: "lrun_v2".to_string(),
            session_id: "sess".to_string(),
            namespace: "EMERY".to_string(),
            triage_score: Some(2),
            triage_reason: Some("ok".to_string()),
            prompt_versions: "v2".to_string(),
            status: "completed".to_string(),
            started_at: now,
            finished_at: Some(now),
            failure_reason: None,
        })
        .unwrap();
        dbs.insert_librarian_candidate(&NewLibrarianCandidateRecord {
            id: "lcan_v2".to_string(),
            run_id: "lrun_v2".to_string(),
            grain_type: "decision".to_string(),
            content: "y".to_string(),
            evidence_quote: "y".to_string(),
            evidence_offset: Some(0),
            critic_verdict: Some("keep".to_string()),
            critic_reason: Some("ok".to_string()),
            reconcile_action: Some("ADD".to_string()),
            written_memory_id: Some(mem_v2.clone()),
            created_at: now,
        })
        .unwrap();
        record_feedback(&dbs, &mem_v2, SIGNAL_NOISE, None, now + 5).unwrap();

        let m = compute_metrics(&dbs, Some("EMERY"), now + 10, 24 * 60 * 60).unwrap();
        assert_eq!(m.completed_runs, 2);
        assert_eq!(m.memories_written, 2);
        assert_eq!(m.noise_flags, 1);
        assert_eq!(m.noise_flag_rate, Some(0.5));

        let buckets: std::collections::HashMap<_, _> = m
            .prompt_version_health
            .iter()
            .map(|p| (p.prompt_versions.as_str(), p))
            .collect();
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets["v1"].written, 1);
        assert_eq!(buckets["v1"].flagged_noise, 0);
        assert_eq!(buckets["v2"].written, 1);
        assert_eq!(buckets["v2"].flagged_noise, 1);
    }

    #[test]
    fn metrics_critic_drop_rate_counts_drops() {
        let (_tmp, dbs) = make_db();
        let now = 1_700_000_000;

        dbs.insert_librarian_run(&NewLibrarianRunRecord {
            id: "lrun_drop".to_string(),
            session_id: "sess".to_string(),
            namespace: "EMERY".to_string(),
            triage_score: Some(2),
            triage_reason: Some("ok".to_string()),
            prompt_versions: "v1".to_string(),
            status: "completed".to_string(),
            started_at: now,
            finished_at: Some(now),
            failure_reason: None,
        })
        .unwrap();
        for (i, verdict) in [("keep", "k"), ("drop", "d1"), ("drop", "d2")]
            .into_iter()
            .enumerate()
        {
            dbs.insert_librarian_candidate(&NewLibrarianCandidateRecord {
                id: format!("lcan_{i}"),
                run_id: "lrun_drop".to_string(),
                grain_type: "decision".to_string(),
                content: verdict.1.to_string(),
                evidence_quote: verdict.1.to_string(),
                evidence_offset: Some(0),
                critic_verdict: Some(verdict.0.to_string()),
                critic_reason: Some("r".to_string()),
                reconcile_action: None,
                written_memory_id: None,
                created_at: now,
            })
            .unwrap();
        }

        let m = compute_metrics(&dbs, Some("EMERY"), now + 10, 24 * 60 * 60).unwrap();
        assert_eq!(m.candidates_total, 3);
        assert_eq!(m.candidates_dropped_by_critic, 2);
        assert!((m.critic_drop_rate.unwrap() - 2.0 / 3.0).abs() < 1e-9);
    }
}
