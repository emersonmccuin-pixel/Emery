//! Triage stage of the librarian pipeline (EMERY-226.001).
//!
//! Calls Haiku with TRIAGE_PROMPT_V1, expects `{"score": 0|1|2|3, "reason": "..."}`
//! and returns a typed [`TriageResult`]. Score 0 short-circuits the rest of the
//! pipeline at the caller — this module just produces the score.

use serde::Deserialize;

use crate::embeddings::anthropic::HAIKU_MODEL;
use crate::librarian::client::ChatClient;
use crate::librarian::prompts::{TRIAGE_PROMPT_V1, render_with_transcript};

const TRIAGE_MAX_TOKENS: u32 = 200;

#[derive(Debug, Clone)]
pub struct TriageResult {
    pub score: i64,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
struct TriageJson {
    score: i64,
    #[serde(default)]
    reason: String,
}

/// Run the triage stage on a transcript.
///
/// Returns Ok(TriageResult) on success. On any LLM/parse failure, returns Err
/// with a human-readable reason. The caller is responsible for writing the
/// failure to the audit log; this function does not touch the database.
pub fn run_triage(client: &dyn ChatClient, transcript: &str) -> Result<TriageResult, String> {
    let prompt = render_with_transcript(TRIAGE_PROMPT_V1, transcript);
    let raw = client.complete(HAIKU_MODEL, TRIAGE_MAX_TOKENS, &prompt)?;
    parse_triage_response(&raw)
}

/// Pull the first JSON object out of `raw` and parse it as a triage response.
/// Tolerates leading/trailing prose ("Here you go: { ... }").
pub fn parse_triage_response(raw: &str) -> Result<TriageResult, String> {
    let json_slice = extract_first_json_object(raw)
        .ok_or_else(|| format!("triage response had no JSON object: {raw}"))?;
    let parsed: TriageJson = serde_json::from_str(json_slice)
        .map_err(|e| format!("triage JSON parse error: {e}; raw={raw}"))?;
    if !(0..=3).contains(&parsed.score) {
        return Err(format!("triage score out of range 0..3: {}", parsed.score));
    }
    Ok(TriageResult {
        score: parsed.score,
        reason: parsed.reason,
    })
}

/// Find the first balanced `{...}` substring (greedy: matches braces).
/// Returns None if no JSON object is present.
pub(crate) fn extract_first_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            match b {
                b'\\' => escape = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Find the first balanced `[...]` JSON array substring.
/// Used by the extractor to tolerate prose-wrapped responses.
#[allow(dead_code)] // used by extract.rs in the next subtask
pub(crate) fn extract_first_json_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            match b {
                b'\\' => escape = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::client::FakeChatClient;

    #[test]
    fn parse_clean_json() {
        let r = parse_triage_response(r#"{"score":0,"reason":"routine"}"#).unwrap();
        assert_eq!(r.score, 0);
        assert_eq!(r.reason, "routine");
    }

    #[test]
    fn parse_with_prose_wrapper() {
        let raw = r#"Sure, here's the score:
{"score": 2, "reason": "real architectural decision"}
hope that helps!"#;
        let r = parse_triage_response(raw).unwrap();
        assert_eq!(r.score, 2);
        assert_eq!(r.reason, "real architectural decision");
    }

    #[test]
    fn parse_rejects_out_of_range() {
        let err = parse_triage_response(r#"{"score":4,"reason":"x"}"#).unwrap_err();
        assert!(err.contains("out of range"));
    }

    #[test]
    fn parse_rejects_no_json() {
        let err = parse_triage_response("nothing here").unwrap_err();
        assert!(err.contains("no JSON"));
    }

    #[test]
    fn run_triage_returns_zero_for_empty_transcript() {
        let fake = FakeChatClient::new(vec![Ok(
            r#"{"score":0,"reason":"empty session"}"#.to_string(),
        )]);
        let r = run_triage(&fake, "").unwrap();
        assert_eq!(r.score, 0);
    }

    #[test]
    fn run_triage_returns_nonzero_for_decision_session() {
        let fake = FakeChatClient::new(vec![Ok(
            r#"{"score":2,"reason":"explicit choice between Voyage and Cohere"}"#.to_string(),
        )]);
        let r = run_triage(&fake, "we picked voyage").unwrap();
        assert_eq!(r.score, 2);
        assert!(r.reason.contains("Voyage"));
    }

    #[test]
    fn run_triage_propagates_llm_error() {
        let fake = FakeChatClient::new(vec![Err("rate_limited".to_string())]);
        let err = run_triage(&fake, "anything").unwrap_err();
        assert_eq!(err, "rate_limited");
    }

    #[test]
    fn extract_first_json_object_handles_nested() {
        let s = r#"prefix {"a": {"b": 1}} suffix"#;
        assert_eq!(extract_first_json_object(s), Some(r#"{"a": {"b": 1}}"#));
    }

    #[test]
    fn extract_first_json_object_ignores_braces_in_strings() {
        let s = r#"{"a": "has } brace"}"#;
        assert_eq!(extract_first_json_object(s), Some(s));
    }

    #[test]
    fn extract_first_json_array_handles_nested() {
        let s = r#"prefix [1, [2, 3], 4] suffix"#;
        assert_eq!(extract_first_json_array(s), Some(r#"[1, [2, 3], 4]"#));
    }
}
