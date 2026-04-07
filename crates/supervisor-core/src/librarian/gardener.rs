//! Gardener stage of the librarian (EMERY-226.002).
//!
//! The gardener is a **propose-only** retirement curator. It looks at a slice
//! of currently-valid memories in a namespace and asks Sonnet which ones
//! should be retired. It never modifies any memory itself — only the
//! `gardener_decide(approve)` path can do that.
//!
//! ## Hard caps enforced in code (not just prompt)
//!
//!   - **20% cap**: at most `ceil(0.2 * batch.len())` proposals per pass.
//!     Even if Sonnet returns more, the surplus is dropped.
//!   - **24-hour rate limit per namespace**: enforced by the higher-level
//!     `service.gardener_run` method against `gardener_runs.started_at`.
//!     This module exposes the check via [`is_rate_limited`].
//!
//! Both caps are belt-and-suspenders: the prompt asks for them, the code
//! enforces them, and the user is the final safety net via approval.

use serde::Deserialize;

use crate::embeddings::anthropic::SONNET_MODEL;
use crate::librarian::client::ChatClient;
use crate::librarian::prompts::{GARDENER_PROMPT_V1, render_gardener};
use crate::librarian::triage::extract_first_json_array;
use crate::models::Memory;

const GARDENER_MAX_TOKENS: u32 = 3000;

/// Hard 20% retirement cap. Computed as ceil(0.2 * batch_size) so that a
/// batch of e.g. 7 memories can still produce 2 proposals.
pub fn max_retirements_for_batch(batch_size: usize) -> usize {
    if batch_size == 0 {
        return 0;
    }
    // ceil(batch_size / 5)
    (batch_size + 4) / 5
}

/// Minimum seconds between gardener runs for the same namespace (24 hours).
pub const GARDENER_RATE_LIMIT_SECS: i64 = 24 * 60 * 60;

/// True if `now - last_started_at < GARDENER_RATE_LIMIT_SECS`.
pub fn is_rate_limited(last_started_at: i64, now: i64) -> bool {
    now.saturating_sub(last_started_at) < GARDENER_RATE_LIMIT_SECS
}

/// One proposal returned by the gardener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub memory_id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
struct ProposalJson {
    memory_id: String,
    #[serde(default)]
    reason: String,
}

/// Run the gardener pass against a slice of memories.
///
/// `context` is a free-form string passed to the prompt for "no longer
/// exists" judgments — typically a recent commit log or list of top-level
/// directories. It can be empty.
///
/// The 20% cap is enforced after parsing, so the caller does not have to
/// trust Sonnet's arithmetic.
pub fn run_gardener(
    client: &dyn ChatClient,
    memories: &[Memory],
    context: &str,
) -> Result<Vec<Proposal>, String> {
    if memories.is_empty() {
        return Ok(Vec::new());
    }
    let max_retirements = max_retirements_for_batch(memories.len());
    let memories_json = memories_to_json(memories);
    let prompt = render_gardener(
        GARDENER_PROMPT_V1,
        &memories_json,
        context,
        max_retirements,
        memories.len(),
    );
    let raw = client.complete(SONNET_MODEL, GARDENER_MAX_TOKENS, &prompt)?;
    let parsed = parse_gardener_response(&raw)?;
    Ok(enforce_cap(parsed, memories, max_retirements))
}

/// Hand-roll a JSON array of `{id, content}` for the gardener prompt. We
/// keep the payload minimal — the gardener does not need timestamps or
/// embeddings to do its job.
pub fn memories_to_json(memories: &[Memory]) -> String {
    let mut out = String::from("[");
    for (i, m) in memories.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"id":{},"content":{}}}"#,
            json_string(&m.id),
            json_string(&m.content),
        ));
    }
    out.push(']');
    out
}

/// Parse a possibly-prose-wrapped JSON array of proposals.
pub fn parse_gardener_response(raw: &str) -> Result<Vec<Proposal>, String> {
    let json_slice = extract_first_json_array(raw)
        .ok_or_else(|| format!("gardener response had no JSON array: {raw}"))?;
    let parsed: Vec<ProposalJson> = serde_json::from_str(json_slice)
        .map_err(|e| format!("gardener JSON parse error: {e}; raw={raw}"))?;
    Ok(parsed
        .into_iter()
        .map(|p| Proposal {
            memory_id: p.memory_id,
            reason: p.reason,
        })
        .collect())
}

/// Enforce the 20% cap and drop proposals whose memory_id is not in the
/// input batch (defense against hallucinated IDs).
pub fn enforce_cap(
    proposals: Vec<Proposal>,
    memories: &[Memory],
    max_retirements: usize,
) -> Vec<Proposal> {
    let valid_ids: std::collections::HashSet<&str> =
        memories.iter().map(|m| m.id.as_str()).collect();
    proposals
        .into_iter()
        .filter(|p| valid_ids.contains(p.memory_id.as_str()))
        .take(max_retirements)
        .collect()
}

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

    fn make_memory(id: &str, content: &str) -> Memory {
        Memory {
            id: id.to_string(),
            namespace: "EMERY".to_string(),
            content: content.to_string(),
            source_ref: None,
            embedding_model: None,
            input_hash: None,
            valid_from: 0,
            valid_to: None,
            supersedes_id: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn cap_is_twenty_percent_ceil() {
        assert_eq!(max_retirements_for_batch(0), 0);
        assert_eq!(max_retirements_for_batch(1), 1);
        assert_eq!(max_retirements_for_batch(5), 1);
        assert_eq!(max_retirements_for_batch(7), 2);
        assert_eq!(max_retirements_for_batch(10), 2);
        assert_eq!(max_retirements_for_batch(100), 20);
        assert_eq!(max_retirements_for_batch(101), 21);
    }

    #[test]
    fn rate_limit_blocks_within_24h() {
        let day = 24 * 60 * 60;
        assert!(is_rate_limited(1_000_000, 1_000_000 + day - 1));
        assert!(!is_rate_limited(1_000_000, 1_000_000 + day));
        assert!(!is_rate_limited(1_000_000, 1_000_000 + day + 1));
    }

    #[test]
    fn parse_empty_array_is_ok() {
        let r = parse_gardener_response("[]").unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn parse_with_prose_wrapper() {
        let raw = r#"Here are my proposals:
        [
          {"memory_id":"mem_1","reason":"superseded by mem_5"}
        ]
        done."#;
        let r = parse_gardener_response(raw).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].memory_id, "mem_1");
    }

    #[test]
    fn enforce_cap_drops_overage() {
        let mems: Vec<Memory> = (0..10).map(|i| make_memory(&format!("mem_{i}"), "x")).collect();
        let proposals: Vec<Proposal> = (0..10)
            .map(|i| Proposal {
                memory_id: format!("mem_{i}"),
                reason: "x".to_string(),
            })
            .collect();
        let kept = enforce_cap(proposals, &mems, max_retirements_for_batch(mems.len()));
        assert_eq!(kept.len(), 2); // 10 * 0.2 = 2
    }

    #[test]
    fn enforce_cap_drops_unknown_ids() {
        let mems = vec![make_memory("mem_1", "x"), make_memory("mem_2", "y")];
        let proposals = vec![
            Proposal {
                memory_id: "mem_1".to_string(),
                reason: "ok".to_string(),
            },
            Proposal {
                memory_id: "mem_999_hallucinated".to_string(),
                reason: "fake".to_string(),
            },
        ];
        let kept = enforce_cap(proposals, &mems, 5);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].memory_id, "mem_1");
    }

    #[test]
    fn run_gardener_short_circuits_on_empty_batch() {
        let fake = FakeChatClient::new(vec![]);
        let r = run_gardener(&fake, &[], "ctx").unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn run_gardener_respects_20_percent_cap() {
        // Build 100 memories, have the LLM "propose" all 100, expect 20 back.
        let mems: Vec<Memory> = (0..100)
            .map(|i| make_memory(&format!("mem_{i}"), "stale"))
            .collect();
        let mut payload = String::from("[");
        for i in 0..100 {
            if i > 0 {
                payload.push(',');
            }
            payload.push_str(&format!(r#"{{"memory_id":"mem_{i}","reason":"stale"}}"#));
        }
        payload.push(']');
        let fake = FakeChatClient::new(vec![Ok(payload)]);
        let r = run_gardener(&fake, &mems, "").unwrap();
        assert_eq!(r.len(), 20);
    }

    #[test]
    fn run_gardener_propagates_llm_error() {
        let mems = vec![make_memory("mem_1", "x")];
        let fake = FakeChatClient::new(vec![Err("rate_limited".to_string())]);
        let err = run_gardener(&fake, &mems, "").unwrap_err();
        assert_eq!(err, "rate_limited");
    }
}
