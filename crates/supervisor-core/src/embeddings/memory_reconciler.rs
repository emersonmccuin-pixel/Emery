//! Memory reconciliation: decide ADD / UPDATE / SUPERSEDE / NOOP for incoming memory.
//!
//! Flow:
//! 1. Embed the incoming content via Voyage.
//! 2. Similarity-query top-K valid memories in the namespace.
//! 3. If max cosine similarity < SIMILARITY_THRESHOLD → straight ADD.
//! 4. Else → call Claude Haiku with a structured prompt and parse its decision.

use crate::embeddings::anthropic::AnthropicClient;
use crate::embeddings::ranker::{blob_to_vec, cosine_similarity};

/// Cosine similarity threshold above which we invoke LLM reconciliation.
pub const SIMILARITY_THRESHOLD: f32 = 0.75;

/// Top-K candidates passed to the reconciliation prompt.
pub const TOP_K: usize = 5;

// ---------------------------------------------------------------------------
// Decision type
// ---------------------------------------------------------------------------

/// The reconciliation action decided for an incoming memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileAction {
    /// Insert as a new, independent memory.
    Add,
    /// Update the content of the closest existing memory in-place.
    Update,
    /// Retire the existing memory (set valid_to=now), insert new with supersedes_id.
    Supersede,
    /// Drop the incoming; the existing memory already covers it.
    Noop,
}

impl std::fmt::Display for ReconcileAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add => write!(f, "ADD"),
            Self::Update => write!(f, "UPDATE"),
            Self::Supersede => write!(f, "SUPERSEDE"),
            Self::Noop => write!(f, "NOOP"),
        }
    }
}

/// Parse the LLM's one-word response into a `ReconcileAction`.
/// Accepts the word anywhere in the response (case-insensitive), favouring
/// first match so preamble like "I'd say SUPERSEDE." works.
pub fn parse_action(response: &str) -> Option<ReconcileAction> {
    let upper = response.to_uppercase();
    // Order matters: check longer/more-specific words first.
    if upper.contains("SUPERSEDE") {
        Some(ReconcileAction::Supersede)
    } else if upper.contains("UPDATE") {
        Some(ReconcileAction::Update)
    } else if upper.contains("NOOP") {
        Some(ReconcileAction::Noop)
    } else if upper.contains("ADD") {
        Some(ReconcileAction::Add)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Candidate passed to the prompt
// ---------------------------------------------------------------------------

/// Minimal representation of an existing memory for the reconciliation prompt.
#[derive(Debug, Clone)]
pub struct MemoryCandidate {
    pub id: String,
    pub content: String,
    pub cosine: f32,
}

// ---------------------------------------------------------------------------
// Score-based candidate selection
// ---------------------------------------------------------------------------

/// Given an embedding BLOB column and a query vector, compute similarity.
/// Returns None if the blob is empty or can't decode.
pub fn score_candidate(embedding_blob: &[u8], query_vec: &[f32]) -> Option<f32> {
    let v = blob_to_vec(embedding_blob);
    if v.is_empty() {
        return None;
    }
    Some(cosine_similarity(query_vec, &v))
}

// ---------------------------------------------------------------------------
// Prompt builder
// ---------------------------------------------------------------------------

/// Build the Haiku prompt for the reconciliation decision.
pub fn build_reconcile_prompt(incoming: &str, candidates: &[MemoryCandidate]) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are a memory reconciliation assistant. \
        Given an INCOMING memory and a list of EXISTING memories, decide the best action:\n\
        - ADD: the incoming is a new, independent fact not covered by any existing memory\n\
        - UPDATE: the incoming refines or corrects an existing memory without contradicting it\n\
        - SUPERSEDE: the incoming directly contradicts or replaces an existing memory\n\
        - NOOP: the incoming is semantically equivalent to an existing memory — nothing new\n\n\
        IMPORTANT: Reply with ONLY one word: ADD, UPDATE, SUPERSEDE, or NOOP. No explanation.\n\n");

    prompt.push_str(&format!("INCOMING:\n{incoming}\n\nEXISTING:\n"));
    for (i, c) in candidates.iter().enumerate() {
        prompt.push_str(&format!("{}. [id:{}] {}\n", i + 1, c.id, c.content));
    }
    prompt
}

// ---------------------------------------------------------------------------
// Main reconciliation entry point (pure logic — no I/O)
// ---------------------------------------------------------------------------

/// Given the query vector and the scored candidates, decide without LLM if possible.
/// Returns `Some(ReconcileAction::Add)` when max similarity is below threshold.
/// Returns `None` when an LLM call is needed.
pub fn fast_path(max_similarity: f32) -> Option<ReconcileAction> {
    if max_similarity < SIMILARITY_THRESHOLD {
        Some(ReconcileAction::Add)
    } else {
        None
    }
}

/// Call Haiku and parse its decision.  Falls back to ADD on any error.
pub fn llm_reconcile(
    client: &AnthropicClient,
    incoming: &str,
    candidates: &[MemoryCandidate],
) -> ReconcileAction {
    let prompt = build_reconcile_prompt(incoming, candidates);
    match client.complete(&prompt) {
        Ok(response) => parse_action(&response).unwrap_or(ReconcileAction::Add),
        Err(e) => {
            eprintln!("[memory reconciler] Haiku call failed ({e}); defaulting to ADD");
            ReconcileAction::Add
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_action tests ───────────────────────────────────────────────────

    #[test]
    fn parse_add() {
        assert_eq!(parse_action("ADD"), Some(ReconcileAction::Add));
        assert_eq!(parse_action("add"), Some(ReconcileAction::Add));
        assert_eq!(parse_action("The answer is ADD."), Some(ReconcileAction::Add));
    }

    #[test]
    fn parse_update() {
        assert_eq!(parse_action("UPDATE"), Some(ReconcileAction::Update));
        assert_eq!(parse_action("update"), Some(ReconcileAction::Update));
    }

    #[test]
    fn parse_supersede() {
        assert_eq!(parse_action("SUPERSEDE"), Some(ReconcileAction::Supersede));
        assert_eq!(parse_action("supersede"), Some(ReconcileAction::Supersede));
        assert_eq!(parse_action("I'd say SUPERSEDE."), Some(ReconcileAction::Supersede));
    }

    #[test]
    fn parse_noop() {
        assert_eq!(parse_action("NOOP"), Some(ReconcileAction::Noop));
        assert_eq!(parse_action("noop"), Some(ReconcileAction::Noop));
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert_eq!(parse_action("MERGE"), None);
        assert_eq!(parse_action(""), None);
        assert_eq!(parse_action("   "), None);
    }

    #[test]
    fn supersede_beats_add_in_same_response() {
        // If the LLM outputs "SUPERSEDE (not ADD)", SUPERSEDE wins (checked first).
        assert_eq!(
            parse_action("SUPERSEDE (not ADD)"),
            Some(ReconcileAction::Supersede)
        );
    }

    // ── fast_path tests ──────────────────────────────────────────────────────

    #[test]
    fn fast_path_below_threshold_returns_add() {
        assert_eq!(fast_path(0.5), Some(ReconcileAction::Add));
        assert_eq!(fast_path(0.0), Some(ReconcileAction::Add));
        assert_eq!(fast_path(SIMILARITY_THRESHOLD - 0.01), Some(ReconcileAction::Add));
    }

    #[test]
    fn fast_path_at_or_above_threshold_returns_none() {
        assert_eq!(fast_path(SIMILARITY_THRESHOLD), None);
        assert_eq!(fast_path(0.99), None);
        assert_eq!(fast_path(1.0), None);
    }

    // ── prompt builder tests ─────────────────────────────────────────────────

    #[test]
    fn build_reconcile_prompt_includes_incoming_and_candidates() {
        let candidates = vec![
            MemoryCandidate {
                id: "mem_abc".into(),
                content: "voyage chosen for embeddings".into(),
                cosine: 0.9,
            },
        ];
        let prompt = build_reconcile_prompt("we use voyage ai", &candidates);
        assert!(prompt.contains("we use voyage ai"));
        assert!(prompt.contains("voyage chosen for embeddings"));
        assert!(prompt.contains("mem_abc"));
    }

    #[test]
    fn build_reconcile_prompt_has_instruction_words() {
        let prompt = build_reconcile_prompt("content", &[]);
        assert!(prompt.contains("ADD"));
        assert!(prompt.contains("SUPERSEDE"));
        assert!(prompt.contains("NOOP"));
    }

    // ── display ──────────────────────────────────────────────────────────────

    #[test]
    fn action_display() {
        assert_eq!(ReconcileAction::Add.to_string(), "ADD");
        assert_eq!(ReconcileAction::Update.to_string(), "UPDATE");
        assert_eq!(ReconcileAction::Supersede.to_string(), "SUPERSEDE");
        assert_eq!(ReconcileAction::Noop.to_string(), "NOOP");
    }
}
