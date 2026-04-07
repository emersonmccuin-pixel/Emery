//! Extraction stage of the librarian pipeline (EMERY-226.001).
//!
//! Calls Sonnet with EXTRACT_PROMPT_V1, parses the JSON array response, and
//! runs the deterministic evidence-anchor validator. Candidates whose
//! `evidence_quote` is not a literal substring of the transcript are dropped
//! before they ever reach the critic — this is the first line of defense
//! against hallucinated evidence.

use serde::Deserialize;

use crate::embeddings::anthropic::SONNET_MODEL;
use crate::librarian::client::ChatClient;
use crate::librarian::prompts::{EXTRACT_PROMPT_V1, render_with_transcript};
use crate::librarian::triage::extract_first_json_array;

const EXTRACT_MAX_TOKENS: u32 = 2000;

/// One candidate grain produced by the extractor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub grain_type: String,
    pub content: String,
    pub evidence_quote: String,
}

/// A candidate that has passed the deterministic evidence-anchor validator.
/// Carries the byte offset of the verbatim quote within the transcript so the
/// audit log can record exactly where the evidence came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCandidate {
    pub candidate: Candidate,
    pub evidence_offset: i64,
}

#[derive(Debug, Deserialize)]
struct ExtractedJson {
    grain_type: String,
    content: String,
    evidence_quote: String,
}

/// The four legal grain types. Anything else is dropped.
const VALID_GRAIN_TYPES: &[&str] = &["decision", "insight", "open_question", "contradiction"];

/// Run the extraction stage on a transcript.
///
/// Returns a list of *validated* candidates — every entry is guaranteed to
/// have an `evidence_quote` that appears verbatim in the transcript. The
/// caller is responsible for passing survivors to the critic stage and
/// writing audit rows; this function does not touch the database.
pub fn run_extract(
    client: &dyn ChatClient,
    transcript: &str,
) -> Result<Vec<ValidatedCandidate>, String> {
    let prompt = render_with_transcript(EXTRACT_PROMPT_V1, transcript);
    let raw = client.complete(SONNET_MODEL, EXTRACT_MAX_TOKENS, &prompt)?;
    let candidates = parse_extract_response(&raw)?;
    Ok(validate_evidence_anchors(transcript, candidates))
}

/// Pull the first JSON array out of `raw` and parse it as a list of
/// candidate grains. Tolerates leading/trailing prose. An empty array is
/// a valid (and frequently correct) response.
pub fn parse_extract_response(raw: &str) -> Result<Vec<Candidate>, String> {
    let json_slice = extract_first_json_array(raw)
        .ok_or_else(|| format!("extract response had no JSON array: {raw}"))?;
    let parsed: Vec<ExtractedJson> = serde_json::from_str(json_slice)
        .map_err(|e| format!("extract JSON parse error: {e}; raw={raw}"))?;
    let mut out = Vec::with_capacity(parsed.len());
    for c in parsed {
        if !VALID_GRAIN_TYPES.contains(&c.grain_type.as_str()) {
            // Drop unknown grain types silently — the prompt forbids them, and
            // the critic stage would drop them anyway. We surface this only
            // through the audit row count (extracted vs validated).
            continue;
        }
        out.push(Candidate {
            grain_type: c.grain_type,
            content: c.content,
            evidence_quote: c.evidence_quote,
        });
    }
    Ok(out)
}

/// Drop any candidate whose `evidence_quote` is not a literal substring of
/// the transcript. This is the deterministic first line of defense against
/// hallucinated evidence — the critic is the second line.
pub fn validate_evidence_anchors(
    transcript: &str,
    candidates: Vec<Candidate>,
) -> Vec<ValidatedCandidate> {
    candidates
        .into_iter()
        .filter_map(|c| {
            // Reject empty quotes outright — they trivially "match" everywhere.
            if c.evidence_quote.is_empty() {
                return None;
            }
            transcript
                .find(&c.evidence_quote)
                .map(|offset| ValidatedCandidate {
                    candidate: c,
                    evidence_offset: offset as i64,
                })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::client::FakeChatClient;

    #[test]
    fn parse_empty_array_is_ok() {
        let r = parse_extract_response("[]").unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn parse_single_decision() {
        let raw = r#"[
            {"grain_type":"decision","content":"Use SQLite WAL","evidence_quote":"we'll use WAL"}
        ]"#;
        let r = parse_extract_response(raw).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].grain_type, "decision");
        assert_eq!(r[0].content, "Use SQLite WAL");
    }

    #[test]
    fn parse_with_prose_wrapper() {
        let raw = r#"Here's what I found:
        [
          {"grain_type":"insight","content":"Voyage rate-limits per key","evidence_quote":"rate limit per key"}
        ]
        That's all."#;
        let r = parse_extract_response(raw).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].grain_type, "insight");
    }

    #[test]
    fn parse_drops_unknown_grain_type() {
        let raw = r#"[
            {"grain_type":"summary","content":"x","evidence_quote":"y"},
            {"grain_type":"decision","content":"a","evidence_quote":"b"}
        ]"#;
        let r = parse_extract_response(raw).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].grain_type, "decision");
    }

    #[test]
    fn parse_rejects_no_array() {
        let err = parse_extract_response("nothing here").unwrap_err();
        assert!(err.contains("no JSON array"));
    }

    #[test]
    fn validate_keeps_verbatim_evidence() {
        let transcript = "the user said: we'll use WAL mode for sure";
        let candidates = vec![Candidate {
            grain_type: "decision".to_string(),
            content: "Use WAL".to_string(),
            evidence_quote: "we'll use WAL".to_string(),
        }];
        let v = validate_evidence_anchors(transcript, candidates);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].evidence_offset, 15);
    }

    #[test]
    fn validate_drops_hallucinated_evidence() {
        let transcript = "the user said hello";
        let candidates = vec![Candidate {
            grain_type: "decision".to_string(),
            content: "x".to_string(),
            evidence_quote: "this never appeared".to_string(),
        }];
        let v = validate_evidence_anchors(transcript, candidates);
        assert!(v.is_empty());
    }

    #[test]
    fn validate_drops_empty_quote() {
        let transcript = "anything";
        let candidates = vec![Candidate {
            grain_type: "decision".to_string(),
            content: "x".to_string(),
            evidence_quote: String::new(),
        }];
        let v = validate_evidence_anchors(transcript, candidates);
        assert!(v.is_empty());
    }

    #[test]
    fn run_extract_returns_empty_for_routine_session() {
        let fake = FakeChatClient::new(vec![Ok("[]".to_string())]);
        let v = run_extract(&fake, "routine work").unwrap();
        assert!(v.is_empty());
    }

    #[test]
    fn run_extract_validates_against_transcript() {
        // The extractor returns one good and one hallucinated candidate.
        // The validator should drop the hallucinated one.
        let raw = r#"[
            {"grain_type":"decision","content":"keep","evidence_quote":"verbatim phrase"},
            {"grain_type":"insight","content":"drop","evidence_quote":"never said this"}
        ]"#;
        let fake = FakeChatClient::new(vec![Ok(raw.to_string())]);
        let v = run_extract(&fake, "the user gave a verbatim phrase here").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].candidate.content, "keep");
    }

    #[test]
    fn run_extract_propagates_llm_error() {
        let fake = FakeChatClient::new(vec![Err("rate_limited".to_string())]);
        let err = run_extract(&fake, "anything").unwrap_err();
        assert_eq!(err, "rate_limited");
    }
}
