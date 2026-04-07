//! Smoke test for EMERY-217.002 — Core embeddings + semantic search.
//!
//! Run with: cargo run -p supervisor-core --example embedding_smoke_test
//!
//! Requires: vault unlocked and VOYAGE_API_KEY set in global scope.

use supervisor_core::{
    AppPaths, DatabaseSet, DiagnosticsHub, DocumentSearchRequest, SessionRegistry,
    SupervisorService, VaultService, WorkItemSearchRequest,
};

fn main() -> anyhow::Result<()> {
    let paths = AppPaths::discover()?;
    let databases = DatabaseSet::initialize(&paths)?;
    let diagnostics = DiagnosticsHub::from_env(&paths)?;
    let registry = SessionRegistry::new(diagnostics.clone());
    let vault = VaultService::new(databases.clone());
    let service = SupervisorService::new(databases, registry, diagnostics, vault);

    // ── Backfill ──────────────────────────────────────────────────────────────
    println!("=== Running backfill ===");
    service.backfill_embeddings();

    // ── Work item search ──────────────────────────────────────────────────────
    println!("\n=== Work item search: 'dispatcher coordination' ===");
    let results = service.search_work_items(WorkItemSearchRequest {
        query_text: "dispatcher coordination".into(),
        limit: Some(8),
        threshold: Some(0.0),
        namespace: None,
    })?;

    if results.is_empty() {
        println!("  (no results — possibly no embeddings yet)");
    }
    for r in &results {
        println!(
            "  [{:.3}] {} — {} ({}) | cosine={:.3} recency={:.3} sw={:.3}",
            r.final_score, r.callsign, r.title, r.status,
            r.cosine, r.recency_decay, r.status_weight
        );
    }

    // ── Document search ───────────────────────────────────────────────────────
    println!("\n=== Document search: 'voyage embeddings temporal memory' ===");
    let doc_results = service.search_documents(DocumentSearchRequest {
        query_text: "voyage embeddings temporal memory".into(),
        limit: Some(5),
        threshold: Some(0.0),
        namespace: None,
    })?;

    if doc_results.is_empty() {
        println!("  (no results — possibly no embeddings yet)");
    }
    for r in &doc_results {
        println!(
            "  [{:.3}] {} — {} ({}) | cosine={:.3} recency={:.3}",
            r.final_score, r.slug, r.title, r.doc_type,
            r.cosine, r.recency_decay
        );
    }

    println!("\n=== Smoke test complete ===");
    Ok(())
}
