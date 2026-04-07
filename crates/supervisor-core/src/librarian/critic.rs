//! Critic stage of the librarian pipeline (EMERY-226.001).
//!
//! Calls Haiku with CRITIC_PROMPT_V1, parses verdicts, and applies them to
//! the validated candidate list. The critic is **deletion-only**: it can
//! mark a grain `keep` or `drop`, nothing else. Any verdict the critic does
//! not return is treated as an implicit `drop` (default-deny).
//!
//! This is the second line of defense against hallucinated evidence; the
//! first line is the deterministic substring check in `extract.rs`.

use serde::Deserialize;

use crate::embeddings::anthropic::HAIKU_MODEL;
use crate::librarian::client::ChatClient;
use crate::librarian::extract::ValidatedCandidate;
use crate::librarian::prompts::{CRITIC_PROMPT_V1, render_critic};
use crate::librarian::triage::extract_first_json_object;

const CRITIC_MAX_TOKENS: u32 = 1500;

/// One verdict from the critic. `verdict` is "keep" or "drop"; anything else
/// is treated as "drop" by `apply_verdicts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub grain_index: usize,
    pub verdict: String,
    pub reason: String,
}

/// A candidate that survived the critic, paired with the critic's reason.
#[derive(Debug, Clone)]
pub struct KeptCandidate {
    pub candidate: ValidatedCandidate,
    pub critic_reason: String,
}

#[derive(Debug, Deserialize)]
struct CriticJson {
    verdicts: Vec<VerdictJson>,
}

#[derive(Debug, Deserialize)]
struct VerdictJson {
    grain_index: usize,
    verdict: String,
    #[serde(default)]
    reason: String,
}

/// Run the critic stage on a list of validated candidates.
///
/// If `candidates` is empty, this short-circuits without an LLM call. Returns
/// the subset of candidates the critic chose to keep.
pub fn run_critic(
    client: &dyn ChatClient,
    transcript: &str,
    candidates: Vec<ValidatedCandidate>,
) -> Result<Vec<KeptCandidate>, String> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let candidates_json = candidates_to_json(&candidates);
    let prompt = render_critic(CRITIC_PROMPT_V1, &candidates_json, transcript);
    let raw = client.complete(HAIKU_MODEL, CRITIC_MAX_TOKENS, &prompt)?;
    let verdicts = parse_critic_response(&raw)?;
    Ok(apply_verdicts(candidates, &verdicts))
}

/// Serialize the candidate list into the JSON shape the critic prompt expects.
/// We hand-roll this rather than pulling in serde derives on `Candidate` to
/// keep the type free of serde concerns elsewhere in the codebase.
pub fn candidates_to_json(candidates: &[ValidatedCandidate]) -> String {
    let mut out = String::from("[");
    for (i, vc) in candidates.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let c = &vc.candidate;
        out.push_str(&format!(
            r#"{{"grain_index":{},"grain_type":{},"content":{},"evidence_quote":{}}}"#,
            i,
            json_string(&c.grain_type),
            json_string(&c.content),
            json_string(&c.evidence_quote),
        ));
    }
    out.push(']');
    out
}

/// Pull the first JSON object out of `raw` and parse it as a critic response.
pub fn parse_critic_response(raw: &str) -> Result<Vec<Verdict>, String> {
    let json_slice = extract_first_json_object(raw)
        .ok_or_else(|| format!("critic response had no JSON object: {raw}"))?;
    let parsed: CriticJson = serde_json::from_str(json_slice)
        .map_err(|e| format!("critic JSON parse error: {e}; raw={raw}"))?;
    Ok(parsed
        .verdicts
        .into_iter()
        .map(|v| Verdict {
            grain_index: v.grain_index,
            verdict: v.verdict,
            reason: v.reason,
        })
        .collect())
}

/// Apply verdicts to the candidate list. Default-deny: any candidate not
/// explicitly marked `keep` is dropped. Out-of-range indices are ignored.
pub fn apply_verdicts(
    candidates: Vec<ValidatedCandidate>,
    verdicts: &[Verdict],
) -> Vec<KeptCandidate> {
    let mut out = Vec::new();
    for (i, vc) in candidates.into_iter().enumerate() {
        let v = verdicts.iter().find(|v| v.grain_index == i);
        match v {
            Some(v) if v.verdict == "keep" => out.push(KeptCandidate {
                candidate: vc,
                critic_reason: v.reason.clone(),
            }),
            _ => {} // implicit drop
        }
    }
    out
}

/// Minimal JSON string encoder. Sufficient for content/quotes that may
/// contain quotes, backslashes, and control characters.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::client::FakeChatClient;
    use crate::librarian::extract::Candidate;

    fn vc(grain_type: &str, content: &str, evidence: &str) -> ValidatedCandidate {
        ValidatedCandidate {
            candidate: Candidate {
                grain_type: grain_type.to_string(),
                content: content.to_string(),
                evidence_quote: evidence.to_string(),
            },
            evidence_offset: 0,
        }
    }

    #[test]
    fn run_critic_short_circuits_on_empty_input() {
        // Pass a fake with no scripted responses — proves we never call it.
        let fake = FakeChatClient::new(vec![]);
        let r = run_critic(&fake, "anything", Vec::new()).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn parse_critic_response_keeps_and_drops() {
        let raw = r#"{
            "verdicts": [
                {"grain_index": 0, "verdict": "keep", "reason": "real choice"},
                {"grain_index": 1, "verdict": "drop", "reason": "vague"}
            ]
        }"#;
        let v = parse_critic_response(raw).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].verdict, "keep");
        assert_eq!(v[1].verdict, "drop");
    }

    #[test]
    fn apply_verdicts_default_drops_unmentioned() {
        let candidates = vec![
            vc("decision", "a", "qa"),
            vc("insight", "b", "qb"),
            vc("decision", "c", "qc"),
        ];
        let verdicts = vec![Verdict {
            grain_index: 0,
            verdict: "keep".to_string(),
            reason: "ok".to_string(),
        }];
        let kept = apply_verdicts(candidates, &verdicts);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].candidate.candidate.content, "a");
    }

    #[test]
    fn apply_verdicts_treats_non_keep_as_drop() {
        let candidates = vec![vc("decision", "a", "q")];
        let verdicts = vec![Verdict {
            grain_index: 0,
            verdict: "maybe".to_string(),
            reason: "x".to_string(),
        }];
        let kept = apply_verdicts(candidates, &verdicts);
        assert!(kept.is_empty());
    }

    #[test]
    fn run_critic_drops_grain_with_fake_evidence() {
        // The critic returns "drop" — survivor list must be empty even if
        // the candidate passed the deterministic validator.
        let raw = r#"{"verdicts":[{"grain_index":0,"verdict":"drop","reason":"vague"}]}"#;
        let fake = FakeChatClient::new(vec![Ok(raw.to_string())]);
        let candidates = vec![vc("decision", "weak", "phrase")];
        let kept = run_critic(&fake, "phrase appears here", candidates).unwrap();
        assert!(kept.is_empty());
    }

    #[test]
    fn run_critic_keeps_strong_grain() {
        let raw = r#"{"verdicts":[{"grain_index":0,"verdict":"keep","reason":"real decision"}]}"#;
        let fake = FakeChatClient::new(vec![Ok(raw.to_string())]);
        let candidates = vec![vc("decision", "use WAL", "we'll use WAL")];
        let kept = run_critic(&fake, "we'll use WAL mode", candidates).unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].critic_reason, "real decision");
    }

    #[test]
    fn candidates_to_json_escapes_quotes_and_newlines() {
        let candidates = vec![vc("decision", "has \"quote\" and\nnewline", "q")];
        let s = candidates_to_json(&candidates);
        assert!(s.contains(r#"\"quote\""#));
        assert!(s.contains(r"\n"));
    }
}
