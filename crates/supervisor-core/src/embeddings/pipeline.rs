//! Canonical input strings and SHA-256 hashing for the embedding pipeline.

use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Canonical input builders
// ---------------------------------------------------------------------------

/// Build the canonical input string for a work item.
///
/// Format: `"{title}\n\n{description}\n\n{acceptance_criteria}"`
/// Acceptance criteria is omitted (with its separator) when absent.
pub fn canonical_work_item_input(
    title: &str,
    description: &str,
    acceptance_criteria: Option<&str>,
) -> String {
    match acceptance_criteria {
        Some(ac) if !ac.trim().is_empty() => {
            format!("{title}\n\n{description}\n\n{ac}")
        }
        _ => format!("{title}\n\n{description}"),
    }
}

/// Build the canonical input string for a document.
///
/// Format: `"{title}\n\n{content_markdown}"`
pub fn canonical_document_input(title: &str, content_markdown: &str) -> String {
    format!("{title}\n\n{content_markdown}")
}

// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------

/// SHA-256 hash of the canonical input string, returned as a lowercase hex string.
pub fn compute_input_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_work_item_with_ac() {
        let s = canonical_work_item_input("Title", "Desc", Some("AC"));
        assert_eq!(s, "Title\n\nDesc\n\nAC");
    }

    #[test]
    fn canonical_work_item_without_ac() {
        let s = canonical_work_item_input("Title", "Desc", None);
        assert_eq!(s, "Title\n\nDesc");
    }

    #[test]
    fn canonical_work_item_blank_ac_omitted() {
        let s = canonical_work_item_input("Title", "Desc", Some("   "));
        assert_eq!(s, "Title\n\nDesc");
    }

    #[test]
    fn canonical_document() {
        let s = canonical_document_input("Doc", "Content");
        assert_eq!(s, "Doc\n\nContent");
    }

    #[test]
    fn hash_is_stable_and_hex() {
        let h = compute_input_hash("hello");
        // Known SHA-256 of "hello":
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn same_input_same_hash() {
        let a = compute_input_hash("test input");
        let b = compute_input_hash("test input");
        assert_eq!(a, b);
    }

    #[test]
    fn different_input_different_hash() {
        let a = compute_input_hash("input a");
        let b = compute_input_hash("input b");
        assert_ne!(a, b);
    }
}
