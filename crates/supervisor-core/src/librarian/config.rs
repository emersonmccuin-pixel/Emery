//! Per-namespace librarian tuning knobs (EMERY-226.003).
//!
//! These are the levers a user has to make the librarian quieter, louder, or
//! pickier without touching code. The defaults below are deliberately
//! conservative — quiet over noisy, picky over greedy — and live in code so
//! that an empty `librarian_config` table behaves correctly.
//!
//! The capture loop reads `LibrarianConfig::for_namespace` at the start of
//! every run and obeys `triage_min_score` and `max_grains_per_run`. The
//! gardener reads `gardener_cap_percent` and `gardener_cooldown_h`.

use anyhow::Result;

use crate::models::{LibrarianConfigRow, UpsertLibrarianConfigRecord};
use crate::store::DatabaseSet;

/// Code defaults. A missing `librarian_config` row uses these.
pub const DEFAULT_TRIAGE_MIN_SCORE: i64 = 1;
pub const DEFAULT_MAX_GRAINS_PER_RUN: i64 = 5;
pub const DEFAULT_GARDENER_CAP_PERCENT: i64 = 20;
pub const DEFAULT_GARDENER_COOLDOWN_H: i64 = 24;

/// Resolved per-namespace librarian config. Always populated — either from
/// the row or from the code defaults.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LibrarianConfig {
    pub namespace: String,
    pub triage_min_score: i64,
    pub max_grains_per_run: i64,
    pub gardener_cap_percent: i64,
    pub gardener_cooldown_h: i64,
}

impl LibrarianConfig {
    /// Build the default config for a namespace. Used when there is no row.
    pub fn defaults_for(namespace: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            triage_min_score: DEFAULT_TRIAGE_MIN_SCORE,
            max_grains_per_run: DEFAULT_MAX_GRAINS_PER_RUN,
            gardener_cap_percent: DEFAULT_GARDENER_CAP_PERCENT,
            gardener_cooldown_h: DEFAULT_GARDENER_COOLDOWN_H,
        }
    }

    /// Look up the live config for a namespace, falling back to defaults.
    pub fn for_namespace(databases: &DatabaseSet, namespace: &str) -> Result<Self> {
        match databases.get_librarian_config(namespace)? {
            Some(row) => Ok(Self::from_row(row)),
            None => Ok(Self::defaults_for(namespace)),
        }
    }

    fn from_row(row: LibrarianConfigRow) -> Self {
        Self {
            namespace: row.namespace,
            triage_min_score: row.triage_min_score,
            max_grains_per_run: row.max_grains_per_run,
            gardener_cap_percent: row.gardener_cap_percent,
            gardener_cooldown_h: row.gardener_cooldown_h,
        }
    }
}

/// Persist a config row, replacing whatever was there. The caller is
/// expected to have validated the values.
pub fn save_config(
    databases: &DatabaseSet,
    config: &LibrarianConfig,
    now_unix: i64,
) -> Result<()> {
    databases.upsert_librarian_config(&UpsertLibrarianConfigRecord {
        namespace: config.namespace.clone(),
        triage_min_score: config.triage_min_score,
        max_grains_per_run: config.max_grains_per_run,
        gardener_cap_percent: config.gardener_cap_percent,
        gardener_cooldown_h: config.gardener_cooldown_h,
        updated_at: now_unix,
    })
}

/// Cheap range checks shared by the service-layer setter and any future
/// CLI/UI. Returns `Err` with a user-facing message on bad input.
pub fn validate_config(config: &LibrarianConfig) -> Result<(), String> {
    if !(0..=3).contains(&config.triage_min_score) {
        return Err(format!(
            "triage_min_score must be 0..=3 (got {})",
            config.triage_min_score
        ));
    }
    if !(1..=50).contains(&config.max_grains_per_run) {
        return Err(format!(
            "max_grains_per_run must be 1..=50 (got {})",
            config.max_grains_per_run
        ));
    }
    if !(1..=100).contains(&config.gardener_cap_percent) {
        return Err(format!(
            "gardener_cap_percent must be 1..=100 (got {})",
            config.gardener_cap_percent
        ));
    }
    if !(1..=720).contains(&config.gardener_cooldown_h) {
        return Err(format!(
            "gardener_cooldown_h must be 1..=720 (got {})",
            config.gardener_cooldown_h
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppPaths;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "emery-config-test-{}-{}",
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
    fn defaults_match_constants() {
        let c = LibrarianConfig::defaults_for("EMERY");
        assert_eq!(c.triage_min_score, DEFAULT_TRIAGE_MIN_SCORE);
        assert_eq!(c.max_grains_per_run, DEFAULT_MAX_GRAINS_PER_RUN);
        assert_eq!(c.gardener_cap_percent, DEFAULT_GARDENER_CAP_PERCENT);
        assert_eq!(c.gardener_cooldown_h, DEFAULT_GARDENER_COOLDOWN_H);
    }

    #[test]
    fn missing_row_returns_defaults() {
        let (_tmp, dbs) = make_db();
        let c = LibrarianConfig::for_namespace(&dbs, "EMERY").unwrap();
        assert_eq!(c, LibrarianConfig::defaults_for("EMERY"));
    }

    #[test]
    fn config_set_get_roundtrip() {
        let (_tmp, dbs) = make_db();
        let c = LibrarianConfig {
            namespace: "EMERY".to_string(),
            triage_min_score: 2,
            max_grains_per_run: 3,
            gardener_cap_percent: 10,
            gardener_cooldown_h: 12,
        };
        save_config(&dbs, &c, 1_700_000_000).unwrap();
        let back = LibrarianConfig::for_namespace(&dbs, "EMERY").unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn config_upsert_replaces_existing_row() {
        let (_tmp, dbs) = make_db();
        let initial = LibrarianConfig {
            namespace: "EMERY".to_string(),
            triage_min_score: 0,
            max_grains_per_run: 5,
            gardener_cap_percent: 20,
            gardener_cooldown_h: 24,
        };
        save_config(&dbs, &initial, 1_700_000_000).unwrap();
        let updated = LibrarianConfig {
            triage_min_score: 3,
            ..initial
        };
        save_config(&dbs, &updated, 1_700_000_001).unwrap();
        let back = LibrarianConfig::for_namespace(&dbs, "EMERY").unwrap();
        assert_eq!(back.triage_min_score, 3);
    }

    #[test]
    fn validate_rejects_out_of_range_values() {
        let mut c = LibrarianConfig::defaults_for("EMERY");
        c.triage_min_score = 5;
        assert!(validate_config(&c).is_err());

        let mut c = LibrarianConfig::defaults_for("EMERY");
        c.max_grains_per_run = 0;
        assert!(validate_config(&c).is_err());

        let mut c = LibrarianConfig::defaults_for("EMERY");
        c.gardener_cap_percent = 200;
        assert!(validate_config(&c).is_err());

        let mut c = LibrarianConfig::defaults_for("EMERY");
        c.gardener_cooldown_h = 0;
        assert!(validate_config(&c).is_err());
    }

    #[test]
    fn validate_accepts_defaults() {
        assert!(validate_config(&LibrarianConfig::defaults_for("EMERY")).is_ok());
    }
}
