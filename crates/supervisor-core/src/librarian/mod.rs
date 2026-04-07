//! High-bar session librarian — capture loop (EMERY-226.001).
//!
//! Pipeline (per session, post-completion):
//!
//!   1. **Triage** (Haiku) — score 0..3. Score 0 short-circuits.
//!   2. **Extract** (Sonnet) — produce zero or more candidate grains, each with
//!      a verbatim evidence anchor.
//!   3. **Evidence anchor validator** (deterministic Rust) — drop any candidate
//!      whose `evidence_quote` is not a literal substring of the transcript.
//!      First line of defense against hallucinated evidence.
//!   4. **Critic** (Haiku, deletion-only) — second line of defense; drops weak grains.
//!   5. **Reconciliation** — survivors are passed to `service.memory_add` which
//!      uses the EMERY-217.003 reconciler (ADD/UPDATE/SUPERSEDE/NOOP).
//!   6. **Audit log** — every stage writes to `librarian_runs` /
//!      `librarian_candidates`, including failures.
//!
//! Status: skeleton landed in EMERY-226.001 first commit. The triage stage is
//! complete; extract/critic/audit/orchestrator/session-completion wiring are
//! the next subtasks.

pub mod client;
pub mod prompts;
pub mod triage;
