//! Read-only librarian digest builder (EMERY-226.002).
//!
//! The digest is the user-facing surface for "what has the librarian been
//! doing?" — it groups recent capture activity by namespace, day, and
//! grain type, and optionally surfaces dropped candidates so the user can
//! tune the prompts. It performs no LLM calls.
//!
//! The MCP tool `emery_librarian_digest` is the only required surface for
//! the librarian; all higher-level UI can be built on top of this.

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use crate::store::{DatabaseSet, open_connection};

/// Per-grain-type counts inside a single namespace.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GrainCounts {
    pub decision: i64,
    pub insight: i64,
    pub open_question: i64,
    pub contradiction: i64,
    pub other: i64,
}

impl GrainCounts {
    pub fn total(&self) -> i64 {
        self.decision + self.insight + self.open_question + self.contradiction + self.other
    }

    fn bump(&mut self, grain_type: &str) {
        match grain_type {
            "decision" => self.decision += 1,
            "insight" => self.insight += 1,
            "open_question" => self.open_question += 1,
            "contradiction" => self.contradiction += 1,
            _ => self.other += 1,
        }
    }
}

/// One memory that survived the pipeline, listed in the digest.
#[derive(Debug, Clone, Serialize)]
pub struct DigestMemoryRow {
    pub memory_id: String,
    pub namespace: String,
    pub grain_type: String,
    pub content: String,
    pub created_at: i64,
}

/// One critic-dropped candidate, included only when the caller requests it.
#[derive(Debug, Clone, Serialize)]
pub struct DroppedCandidateRow {
    pub candidate_id: String,
    pub namespace: String,
    pub grain_type: String,
    pub content: String,
    pub critic_reason: Option<String>,
    pub created_at: i64,
}

/// Top-level digest payload.
#[derive(Debug, Clone, Serialize)]
pub struct LibrarianDigest {
    pub namespace: Option<String>,
    pub since_unix: i64,
    pub now_unix: i64,
    pub kept_counts: GrainCounts,
    pub dropped_count: i64,
    pub kept: Vec<DigestMemoryRow>,
    pub dropped: Vec<DroppedCandidateRow>,
}

/// Build a digest of recent librarian activity.
///
/// The window is `now - since_secs .. now`, applied to `librarian_candidates.created_at`.
pub fn build_digest(
    databases: &DatabaseSet,
    namespace: Option<&str>,
    now_unix: i64,
    since_secs: i64,
    include_dropped: bool,
) -> Result<LibrarianDigest> {
    let since_unix = now_unix.saturating_sub(since_secs);
    let connection = open_connection(&databases.paths().knowledge_db)?;

    // Pull all candidates in the window for runs in the requested namespace.
    // We join through librarian_runs to get the namespace filter.
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    let mut kept_counts = GrainCounts::default();
    let mut dropped_count: i64 = 0;

    let (sql, ns_filter): (&str, Option<String>) = match namespace {
        Some(ns) => (
            "SELECT c.id, r.namespace, c.grain_type, c.content,
                    c.critic_verdict, c.critic_reason, c.written_memory_id, c.created_at
               FROM librarian_candidates c
               JOIN librarian_runs r ON r.id = c.run_id
              WHERE c.created_at >= ?1
                AND r.namespace = ?2
              ORDER BY c.created_at DESC",
            Some(ns.to_string()),
        ),
        None => (
            "SELECT c.id, r.namespace, c.grain_type, c.content,
                    c.critic_verdict, c.critic_reason, c.written_memory_id, c.created_at
               FROM librarian_candidates c
               JOIN librarian_runs r ON r.id = c.run_id
              WHERE c.created_at >= ?1
              ORDER BY c.created_at DESC",
            None,
        ),
    };

    let mut stmt = connection.prepare(sql)?;
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
    )> = match ns_filter {
        Some(ns) => stmt
            .query_map(params![since_unix, ns], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        None => stmt
            .query_map(params![since_unix], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    };

    for (
        candidate_id,
        ns,
        grain_type,
        content,
        critic_verdict,
        critic_reason,
        written_memory_id,
        created_at,
    ) in rows
    {
        // A candidate is "kept" if it produced a written memory.
        let was_written = written_memory_id.is_some();
        if was_written {
            kept_counts.bump(&grain_type);
            kept.push(DigestMemoryRow {
                memory_id: written_memory_id.unwrap(),
                namespace: ns,
                grain_type,
                content,
                created_at,
            });
        } else if critic_verdict.as_deref() == Some("drop") || critic_verdict.is_none() {
            dropped_count += 1;
            if include_dropped {
                dropped.push(DroppedCandidateRow {
                    candidate_id,
                    namespace: ns,
                    grain_type,
                    content,
                    critic_reason,
                    created_at,
                });
            }
        }
    }

    Ok(LibrarianDigest {
        namespace: namespace.map(|s| s.to_string()),
        since_unix,
        now_unix,
        kept_counts,
        dropped_count,
        kept,
        dropped,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppPaths;
    use crate::models::{NewLibrarianCandidateRecord, NewLibrarianRunRecord};
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "emery-digest-test-{}-{}",
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

    use uuid::Uuid;

    /// Insert a synthetic run + candidates with explicit timestamps so the
    /// digest window assertions are deterministic regardless of wall clock.
    fn seed_run(
        dbs: &DatabaseSet,
        namespace: &str,
        kept: &[(&str, &str)],
        dropped: &[(&str, &str)],
        created_at: i64,
    ) -> String {
        let run_id = format!("lrun_test_{}", Uuid::new_v4().simple());
        dbs.insert_librarian_run(&NewLibrarianRunRecord {
            id: run_id.clone(),
            session_id: "sess".to_string(),
            namespace: namespace.to_string(),
            triage_score: Some(2),
            triage_reason: Some("seed".to_string()),
            prompt_versions: r#"{"triage":"v1"}"#.to_string(),
            status: "completed".to_string(),
            started_at: created_at,
            finished_at: Some(created_at),
            failure_reason: None,
        })
        .unwrap();

        for (gt, content) in kept {
            let cid = format!("lcan_test_{}", Uuid::new_v4().simple());
            dbs.insert_librarian_candidate(&NewLibrarianCandidateRecord {
                id: cid.clone(),
                run_id: run_id.clone(),
                grain_type: gt.to_string(),
                content: content.to_string(),
                evidence_quote: "q".to_string(),
                evidence_offset: Some(0),
                critic_verdict: None,
                critic_reason: None,
                reconcile_action: None,
                written_memory_id: None,
                created_at,
            })
            .unwrap();
            dbs.update_librarian_candidate_critic(&cid, "keep", Some("ok"))
                .unwrap();
            dbs.update_librarian_candidate_reconcile(&cid, "ADD", Some("mem_fake"))
                .unwrap();
        }
        for (gt, content) in dropped {
            let cid = format!("lcan_test_{}", Uuid::new_v4().simple());
            dbs.insert_librarian_candidate(&NewLibrarianCandidateRecord {
                id: cid.clone(),
                run_id: run_id.clone(),
                grain_type: gt.to_string(),
                content: content.to_string(),
                evidence_quote: "q".to_string(),
                evidence_offset: Some(0),
                critic_verdict: None,
                critic_reason: None,
                reconcile_action: None,
                written_memory_id: None,
                created_at,
            })
            .unwrap();
            dbs.update_librarian_candidate_critic(&cid, "drop", Some("vague"))
                .unwrap();
        }
        run_id
    }

    #[test]
    fn digest_counts_grain_types() {
        let (_tmp, dbs) = make_db();
        let now = 1_800_000_000;
        seed_run(
            &dbs,
            "EMERY",
            &[
                ("decision", "use WAL"),
                ("decision", "use Voyage"),
                ("insight", "rate limit"),
            ],
            &[("decision", "vague")],
            now,
        );

        let d = build_digest(&dbs, Some("EMERY"), now + 10, 24 * 60 * 60, false).unwrap();
        assert_eq!(d.kept_counts.decision, 2);
        assert_eq!(d.kept_counts.insight, 1);
        assert_eq!(d.kept_counts.total(), 3);
        assert_eq!(d.dropped_count, 1);
        assert!(d.dropped.is_empty(), "include_dropped=false should hide bodies");
    }

    #[test]
    fn digest_includes_dropped_only_when_requested() {
        let (_tmp, dbs) = make_db();
        let now = 1_800_000_000;
        seed_run(&dbs, "EMERY", &[], &[("decision", "weak grain")], now);

        let off = build_digest(&dbs, Some("EMERY"), now + 10, 24 * 60 * 60, false).unwrap();
        assert_eq!(off.dropped_count, 1);
        assert!(off.dropped.is_empty());

        let on = build_digest(&dbs, Some("EMERY"), now + 10, 24 * 60 * 60, true).unwrap();
        assert_eq!(on.dropped_count, 1);
        assert_eq!(on.dropped.len(), 1);
        assert_eq!(on.dropped[0].critic_reason.as_deref(), Some("vague"));
    }

    #[test]
    fn digest_window_excludes_old_runs() {
        let (_tmp, dbs) = make_db();
        let now = 1_800_000_000;
        let week = 7 * 24 * 60 * 60;
        seed_run(&dbs, "EMERY", &[("decision", "old")], &[], now - 2 * week);
        seed_run(&dbs, "EMERY", &[("decision", "new")], &[], now);

        let d = build_digest(&dbs, Some("EMERY"), now + 10, week, false).unwrap();
        assert_eq!(d.kept_counts.decision, 1);
        assert_eq!(d.kept[0].content, "new");
    }

    #[test]
    fn digest_namespace_filter_isolates_namespaces() {
        let (_tmp, dbs) = make_db();
        let now = 1_800_000_000;
        seed_run(&dbs, "EMERY", &[("decision", "emery thing")], &[], now);
        seed_run(&dbs, "OTHER", &[("decision", "other thing")], &[], now);

        let emery = build_digest(&dbs, Some("EMERY"), now + 10, 24 * 60 * 60, false).unwrap();
        assert_eq!(emery.kept_counts.decision, 1);
        assert_eq!(emery.kept[0].content, "emery thing");

        let all = build_digest(&dbs, None, now + 10, 24 * 60 * 60, false).unwrap();
        assert_eq!(all.kept_counts.decision, 2);
    }
}
