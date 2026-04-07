//! Pure-Rust cosine similarity, recency decay, and status-weight scoring.
//!
//! All tunable constants are defined in this module.
//!
//! `final_score = cosine_similarity * recency_decay(updated_at) * status_weight(status)`

use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Tunable constants
// ---------------------------------------------------------------------------

/// Half-life for recency decay (30 days in seconds).
pub const HALF_LIFE_SECS: f64 = 30.0 * 24.0 * 3600.0;

/// Minimum recency decay factor (floor so old items still appear if highly relevant).
pub const RECENCY_DECAY_MIN: f32 = 0.1;

/// Maximum recency decay factor.
pub const RECENCY_DECAY_MAX: f32 = 1.0;

/// Status weights: tunes final score based on work item / document status.
pub const STATUS_WEIGHT_IN_PROGRESS: f32 = 1.2;
pub const STATUS_WEIGHT_BACKLOG: f32 = 1.0;
pub const STATUS_WEIGHT_DONE: f32 = 0.7;
pub const STATUS_WEIGHT_CANCELLED: f32 = 0.4;

// ---------------------------------------------------------------------------
// BLOB encoding
// ---------------------------------------------------------------------------

/// Serialize a `Vec<f32>` as raw little-endian bytes.
pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for &x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Deserialize raw little-endian bytes back to `Vec<f32>`.
/// Returns an empty vec if the byte length is not a multiple of 4.
pub fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    if b.len() % 4 != 0 {
        return vec![];
    }
    b.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

// ---------------------------------------------------------------------------
// Cosine similarity
// ---------------------------------------------------------------------------

/// Cosine similarity between two equal-length f32 slices.
///
/// Returns 0.0 if either vector has zero norm or lengths differ.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

// ---------------------------------------------------------------------------
// Recency decay
// ---------------------------------------------------------------------------

/// Exponential recency decay clamped to [`RECENCY_DECAY_MIN`, `RECENCY_DECAY_MAX`].
///
/// `decay = 2^(-age_secs / HALF_LIFE_SECS)` clamped to [0.1, 1.0].
pub fn recency_decay(updated_at: i64) -> f32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(updated_at);
    let age_secs = (now - updated_at).max(0) as f64;
    let decay = 2f64.powf(-age_secs / HALF_LIFE_SECS) as f32;
    decay.clamp(RECENCY_DECAY_MIN, RECENCY_DECAY_MAX)
}

// ---------------------------------------------------------------------------
// Status weight
// ---------------------------------------------------------------------------

/// Returns the status weight for a given status string.
/// Unknown statuses default to `STATUS_WEIGHT_BACKLOG`.
pub fn status_weight(status: &str) -> f32 {
    match status {
        "in_progress" => STATUS_WEIGHT_IN_PROGRESS,
        "backlog" => STATUS_WEIGHT_BACKLOG,
        "done" => STATUS_WEIGHT_DONE,
        "cancelled" => STATUS_WEIGHT_CANCELLED,
        _ => STATUS_WEIGHT_BACKLOG,
    }
}

// ---------------------------------------------------------------------------
// Combined score
// ---------------------------------------------------------------------------

/// `final_score = cosine * recency_decay(updated_at) * status_weight(status)`
pub fn final_score(cosine: f32, updated_at: i64, status: &str) -> f32 {
    cosine * recency_decay(updated_at) * status_weight(status)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_round_trip() {
        let v: Vec<f32> = vec![1.0, -2.5, 0.0, 0.333];
        let blob = vec_to_blob(&v);
        let back = blob_to_vec(&blob);
        assert_eq!(v.len(), back.len());
        for (a, b) in v.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn blob_to_vec_odd_length_returns_empty() {
        let blob = vec![1u8, 2, 3]; // 3 bytes — not divisible by 4
        assert!(blob_to_vec(&blob).is_empty());
    }

    #[test]
    fn cosine_similarity_identical_vectors() {
        let v = vec![1.0f32, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_zero_vector_returns_zero() {
        let a = vec![0.0f32, 0.0];
        let b = vec![1.0f32, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_mismatched_lengths_returns_zero() {
        let a = vec![1.0f32, 2.0];
        let b = vec![1.0f32];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn recency_decay_recent_item_near_one() {
        // An item updated just now should have decay close to 1.0.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let decay = recency_decay(now);
        assert!(decay > 0.95, "recent decay = {}", decay);
    }

    #[test]
    fn recency_decay_very_old_item_hits_floor() {
        // An item from 10 years ago should hit RECENCY_DECAY_MIN.
        let very_old = 0i64; // Unix epoch: 1970
        let decay = recency_decay(very_old);
        assert!((decay - RECENCY_DECAY_MIN).abs() < 1e-4);
    }

    #[test]
    fn recency_decay_30_day_old_is_half() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let thirty_days_ago = now - (30 * 24 * 3600);
        let decay = recency_decay(thirty_days_ago);
        // 2^(-1) = 0.5; clamped to [0.1, 1.0] so should be ~0.5
        assert!((decay - 0.5).abs() < 0.02, "30-day decay = {}", decay);
    }

    #[test]
    fn status_weights() {
        assert!(status_weight("in_progress") > status_weight("backlog"));
        assert!(status_weight("backlog") > status_weight("done"));
        assert!(status_weight("done") > status_weight("cancelled"));
        assert_eq!(status_weight("unknown"), STATUS_WEIGHT_BACKLOG);
    }
}
