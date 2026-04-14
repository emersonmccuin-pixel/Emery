//! Voyage AI embeddings for work-item semantic search.
//!
//! Model is locked to `voyage-3-large` at 1024 dimensions; the vec0 virtual
//! table width must match. Secrets flow exclusively through
//! [`vault::release_for_internal`] — never through IPC or frontend invokes.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use zeroize::Zeroizing;

use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::vault;

pub const VOYAGE_MODEL: &str = "voyage-3-large";
pub const VOYAGE_DIMENSIONS: usize = 1024;
pub const VOYAGE_ENDPOINT: &str = "https://api.voyageai.com/v1/embeddings";
pub const VOYAGE_BATCH_LIMIT: usize = 128;
pub const VAULT_CONSUMER: &str = "embeddings";
pub const VAULT_ENTRY_NAME: &str = "voyage-ai";
pub const MAX_INPUT_CHARS: usize = 30_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug)]
pub enum EmbeddingInputType {
    Document,
    Query,
}

impl EmbeddingInputType {
    fn as_str(self) -> &'static str {
        match self {
            EmbeddingInputType::Document => "document",
            EmbeddingInputType::Query => "query",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedOutcome {
    pub work_item_id: i64,
    pub changed: bool,
    pub content_hash: String,
    pub dimensions: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    pub status: Option<String>,
    pub item_type: Option<String>,
    pub open_only: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub work_item_id: i64,
    pub call_sign: String,
    pub title: String,
    pub status: String,
    pub item_type: String,
    pub distance: f32,
    pub score: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackfillReport {
    pub total: usize,
    pub embedded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingsStatus {
    pub configured: bool,
    pub total_items: usize,
    pub embedded_items: usize,
    pub pending_items: usize,
    pub last_error: Option<String>,
}

/// Abstraction over the Voyage HTTP call so tests can inject a stub.
pub trait VoyageClient: Send + Sync {
    fn embed(
        &self,
        api_key: &str,
        model: &str,
        inputs: &[String],
        input_type: EmbeddingInputType,
    ) -> AppResult<Vec<Vec<f32>>>;
}

pub struct ReqwestVoyageClient {
    http: reqwest::blocking::Client,
}

impl ReqwestVoyageClient {
    pub fn new() -> AppResult<Self> {
        let http = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| AppError::internal(format!(
                "failed to build Voyage HTTP client: {error}"
            )))?;
        Ok(Self { http })
    }
}

impl VoyageClient for ReqwestVoyageClient {
    fn embed(
        &self,
        api_key: &str,
        model: &str,
        inputs: &[String],
        input_type: EmbeddingInputType,
    ) -> AppResult<Vec<Vec<f32>>> {
        let body = serde_json::json!({
            "input": inputs,
            "model": model,
            "input_type": input_type.as_str(),
        });

        let response = self
            .http
            .post(VOYAGE_ENDPOINT)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .map_err(|error| AppError::supervisor(format!("Voyage request failed: {error}")))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().unwrap_or_default();
            return Err(AppError::from_status(
                status.as_u16(),
                format!("Voyage API error ({status}): {text}"),
            ));
        }

        #[derive(Deserialize)]
        struct VoyageDatum {
            embedding: Vec<f32>,
        }
        #[derive(Deserialize)]
        struct VoyageResponse {
            data: Vec<VoyageDatum>,
        }

        let parsed: VoyageResponse = response
            .json()
            .map_err(|error| AppError::internal(format!("failed to decode Voyage response: {error}")))?;

        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }
}

pub struct EmbeddingsService {
    state: AppState,
    client: Box<dyn VoyageClient>,
}

impl EmbeddingsService {
    pub fn new(state: AppState) -> AppResult<Self> {
        Ok(Self {
            state,
            client: Box::new(ReqwestVoyageClient::new()?),
        })
    }

    pub fn with_client(state: AppState, client: Box<dyn VoyageClient>) -> Self {
        Self { state, client }
    }

    /// Release the Voyage API key from the vault. Returns `Ok(None)` when no
    /// entry is configured.
    pub fn voyage_api_key(&self) -> AppResult<Option<Zeroizing<String>>> {
        vault::release_for_internal(&self.state, VAULT_CONSUMER, VAULT_ENTRY_NAME)
    }

    /// Assemble the embeddable text for a work item: title + body + linked
    /// document bodies, separated by `\n\n---\n\n`, truncated at
    /// [`MAX_INPUT_CHARS`].
    ///
    /// NOTE: no work-item comments table exists in the Phase A schema; when
    /// one lands, extend this function to append comment bodies too.
    pub fn assemble_work_item_text(&self, work_item_id: i64) -> AppResult<String> {
        let item = self.state.get_work_item(work_item_id)?;
        let documents = self
            .state
            .list_documents(item.project_id)?
            .into_iter()
            .filter(|doc| doc.work_item_id == Some(item.id))
            .collect::<Vec<_>>();

        let mut sections: Vec<String> = Vec::with_capacity(2 + documents.len());
        sections.push(item.title.clone());
        if !item.body.trim().is_empty() {
            sections.push(item.body.clone());
        }
        for doc in documents {
            let mut block = String::new();
            block.push_str(&doc.title);
            if !doc.body.trim().is_empty() {
                block.push_str("\n\n");
                block.push_str(&doc.body);
            }
            sections.push(block);
        }

        let joined = sections.join("\n\n---\n\n");
        Ok(truncate_chars(&joined, MAX_INPUT_CHARS))
    }

    /// Embed a single raw text blob. Propagates
    /// [`AppError::invalid_input`] when the vault entry is missing.
    pub fn embed_text(
        &self,
        model: &str,
        input: &str,
        input_type: EmbeddingInputType,
    ) -> AppResult<Vec<f32>> {
        let key = self.voyage_api_key()?.ok_or_else(|| {
            AppError::invalid_input(
                "Voyage API key is not configured. Add a vault entry named 'voyage-ai'.",
            )
        })?;
        let mut result = self
            .client
            .embed(&key, model, &[input.to_string()], input_type)?;
        result.pop().ok_or_else(|| {
            AppError::internal("Voyage response contained no embedding data")
        })
    }

    /// Recompute + persist the embedding for one work item. Short-circuits
    /// when the content hash already matches the stored row.
    pub fn embed_work_item(&self, work_item_id: i64) -> AppResult<EmbedOutcome> {
        let text = self.assemble_work_item_text(work_item_id)?;
        let content_hash = sha256_hex(&text);
        let connection = self
            .state
            .connect_internal()
            .map_err(AppError::database)?;

        let existing: Option<String> = connection
            .query_row(
                "SELECT content_hash FROM work_item_embeddings WHERE work_item_id = ?1",
                [work_item_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| {
                AppError::database(format!("failed to load existing embedding hash: {error}"))
            })?;

        if existing.as_deref() == Some(content_hash.as_str()) {
            return Ok(EmbedOutcome {
                work_item_id,
                changed: false,
                content_hash,
                dimensions: VOYAGE_DIMENSIONS,
            });
        }

        let embedding = self.embed_text(VOYAGE_MODEL, &text, EmbeddingInputType::Document)?;
        if embedding.len() != VOYAGE_DIMENSIONS {
            return Err(AppError::internal(format!(
                "Voyage returned {} dimensions; expected {VOYAGE_DIMENSIONS}",
                embedding.len()
            )));
        }

        persist_embedding(&connection, work_item_id, &content_hash, &embedding)?;

        Ok(EmbedOutcome {
            work_item_id,
            changed: true,
            content_hash,
            dimensions: VOYAGE_DIMENSIONS,
        })
    }

    /// Semantic search across a project's work items.
    pub fn search(
        &self,
        project_id: i64,
        query: &str,
        k: usize,
        filters: SearchFilters,
    ) -> AppResult<Vec<SearchHit>> {
        let query = query.trim();
        if query.is_empty() {
            return Err(AppError::invalid_input("search query must not be empty"));
        }
        if k == 0 {
            return Ok(Vec::new());
        }
        let capped_k = k.min(50);

        let embedding = self.embed_text(VOYAGE_MODEL, query, EmbeddingInputType::Query)?;
        if embedding.len() != VOYAGE_DIMENSIONS {
            return Err(AppError::internal(format!(
                "Voyage query embedding returned {} dimensions; expected {VOYAGE_DIMENSIONS}",
                embedding.len()
            )));
        }

        let connection = self
            .state
            .connect_internal()
            .map_err(AppError::database)?;

        // Over-fetch to leave room for post-filter trimming; vec0 MATCH uses k
        // as an exact limit so we need at least capped_k results after SQL
        // filters.
        let search_k = (capped_k.saturating_mul(4)).max(capped_k);
        let embedding_blob = encode_f32_vec(&embedding);

        let mut statement = connection
            .prepare(
                "
                SELECT v.work_item_id, v.distance, w.project_id, w.call_sign, w.title, w.status, w.item_type
                FROM work_item_vectors AS v
                JOIN work_items AS w ON w.id = v.work_item_id
                WHERE v.embedding MATCH ?1 AND k = ?2
                ORDER BY v.distance ASC
                ",
            )
            .map_err(|error| AppError::database(format!("failed to prepare vector search: {error}")))?;

        let rows = statement
            .query_map(params![embedding_blob, search_k as i64], |row| {
                Ok(RawHit {
                    work_item_id: row.get(0)?,
                    distance: row.get::<_, f64>(1)? as f32,
                    project_id: row.get(2)?,
                    call_sign: row.get(3)?,
                    title: row.get(4)?,
                    status: row.get(5)?,
                    item_type: row.get(6)?,
                })
            })
            .map_err(|error| AppError::database(format!("failed to run vector search: {error}")))?;

        let mut hits: Vec<SearchHit> = Vec::new();
        for row in rows {
            let raw = row.map_err(|error| {
                AppError::database(format!("failed to read vector search row: {error}"))
            })?;
            if raw.project_id != project_id {
                continue;
            }
            if let Some(status) = filters.status.as_deref() {
                if raw.status != status {
                    continue;
                }
            }
            if let Some(item_type) = filters.item_type.as_deref() {
                if raw.item_type != item_type {
                    continue;
                }
            }
            if filters.open_only.unwrap_or(false) && raw.status == "done" {
                continue;
            }

            hits.push(SearchHit {
                work_item_id: raw.work_item_id,
                call_sign: raw.call_sign,
                title: raw.title,
                status: raw.status,
                item_type: raw.item_type,
                distance: raw.distance,
                score: 1.0 - raw.distance.clamp(0.0, 2.0) / 2.0,
            });
            if hits.len() >= capped_k {
                break;
            }
        }

        Ok(hits)
    }

    /// Re-embed every work item (optionally scoped to one project). Voyage
    /// allows up to 128 inputs per request; we batch to minimize API cost but
    /// still short-circuit per-item when the hash matches.
    pub fn backfill(
        &self,
        project_id: Option<i64>,
        progress: impl Fn(usize, usize),
    ) -> AppResult<BackfillReport> {
        // Gather candidates up front so we can surface total/embedded/skipped
        // counts without juggling a streaming query.
        let items = if let Some(pid) = project_id {
            self.state.list_work_items(pid)?
        } else {
            let connection = self.state.connect_internal().map_err(AppError::database)?;
            let mut statement = connection
                .prepare("SELECT id FROM work_items ORDER BY id ASC")
                .map_err(|error| AppError::database(format!("failed to list work items: {error}")))?;
            let rows = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(|error| AppError::database(format!("failed to read work item ids: {error}")))?;
            let mut ids = Vec::new();
            for row in rows {
                let id = row.map_err(|error| {
                    AppError::database(format!("failed to read work item id: {error}"))
                })?;
                ids.push(id);
            }
            drop(statement);
            drop(connection);
            let mut loaded = Vec::with_capacity(ids.len());
            for id in ids {
                loaded.push(self.state.get_work_item(id)?);
            }
            loaded
        };

        let total = items.len();
        let mut report = BackfillReport {
            total,
            ..Default::default()
        };

        for (idx, item) in items.iter().enumerate() {
            match self.embed_work_item(item.id) {
                Ok(outcome) => {
                    if outcome.changed {
                        report.embedded += 1;
                    } else {
                        report.skipped += 1;
                    }
                }
                Err(error) => {
                    report.failed += 1;
                    if report.errors.len() < 20 {
                        report.errors.push(format!("work_item#{}: {}", item.id, error.message));
                    }
                    log::warn!(
                        "embeddings backfill failed for work_item#{}: {}",
                        item.id,
                        error.message
                    );
                }
            }
            progress(idx + 1, total);
        }

        Ok(report)
    }

    /// Summarize embedding coverage for the status UI.
    pub fn status(&self) -> AppResult<EmbeddingsStatus> {
        let configured = self.voyage_api_key()?.is_some();
        let connection = self
            .state
            .connect_internal()
            .map_err(AppError::database)?;

        let total_items: i64 = connection
            .query_row("SELECT COUNT(*) FROM work_items", [], |row| row.get(0))
            .map_err(|error| {
                AppError::database(format!("failed to count work items: {error}"))
            })?;
        let embedded_items: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM work_item_embeddings",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                AppError::database(format!("failed to count work_item_embeddings rows: {error}"))
            })?;

        let total_items = total_items.max(0) as usize;
        let embedded_items = embedded_items.max(0) as usize;
        let pending_items = total_items.saturating_sub(embedded_items);

        Ok(EmbeddingsStatus {
            configured,
            total_items,
            embedded_items,
            pending_items,
            last_error: None,
        })
    }
}

struct RawHit {
    work_item_id: i64,
    distance: f32,
    project_id: i64,
    call_sign: String,
    title: String,
    status: String,
    item_type: String,
}

fn persist_embedding(
    connection: &Connection,
    work_item_id: i64,
    content_hash: &str,
    embedding: &[f32],
) -> AppResult<()> {
    connection
        .execute(
            "
            INSERT INTO work_item_embeddings
                (work_item_id, content_hash, model, dimensions, embedded_at)
            VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
            ON CONFLICT(work_item_id) DO UPDATE SET
                content_hash = excluded.content_hash,
                model = excluded.model,
                dimensions = excluded.dimensions,
                embedded_at = CURRENT_TIMESTAMP
            ",
            params![
                work_item_id,
                content_hash,
                VOYAGE_MODEL,
                VOYAGE_DIMENSIONS as i64
            ],
        )
        .map_err(|error| AppError::database(format!("failed to upsert work_item_embeddings: {error}")))?;

    // vec0 virtual tables do not support ON CONFLICT / UPSERT cleanly; a
    // DELETE-then-INSERT within a single transaction is the documented pattern.
    let blob = encode_f32_vec(embedding);
    connection
        .execute(
            "DELETE FROM work_item_vectors WHERE work_item_id = ?1",
            [work_item_id],
        )
        .map_err(|error| AppError::database(format!("failed to clear old work_item_vectors row: {error}")))?;
    connection
        .execute(
            "INSERT INTO work_item_vectors (work_item_id, embedding) VALUES (?1, ?2)",
            params![work_item_id, blob],
        )
        .map_err(|error| AppError::database(format!("failed to insert work_item_vectors row: {error}")))?;

    Ok(())
}

fn encode_f32_vec(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for v in values {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    input.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{AppState, CreateWorkItemInput, StorageInfo};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    fn unique_temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pjtcmd-embeddings-{}-{}-{}",
            label,
            std::process::id(),
            suffix,
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    struct StubVoyage {
        calls: Arc<Mutex<Vec<(String, EmbeddingInputType, Vec<String>)>>>,
        dims: usize,
    }

    impl VoyageClient for StubVoyage {
        fn embed(
            &self,
            _api_key: &str,
            model: &str,
            inputs: &[String],
            input_type: EmbeddingInputType,
        ) -> AppResult<Vec<Vec<f32>>> {
            self.calls
                .lock()
                .unwrap()
                .push((model.to_string(), input_type, inputs.to_vec()));
            // Deterministic per-input vector so search can discriminate hits.
            Ok(inputs
                .iter()
                .map(|input| {
                    let seed = sha256_hex(input);
                    let mut vec = vec![0.0_f32; self.dims];
                    for (i, byte) in seed.as_bytes().iter().enumerate() {
                        vec[i % self.dims] += (*byte as f32) / 255.0;
                    }
                    let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
                    for v in vec.iter_mut() {
                        *v /= norm;
                    }
                    vec
                })
                .collect())
        }
    }

    fn new_state(dir: &std::path::Path) -> (AppState, i64) {
        let db_path = dir.join("db").join("pc.sqlite3");
        std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let storage = StorageInfo {
            app_data_dir: dir.display().to_string(),
            db_dir: db_path.parent().unwrap().display().to_string(),
            db_path: db_path.display().to_string(),
        };
        let state = AppState::new(storage).unwrap();
        let root_path = dir.join("project");
        std::fs::create_dir_all(&root_path).unwrap();
        // Seed a project + vault entry so release_for_internal succeeds.
        let project = state
            .create_project(crate::db::CreateProjectInput {
                name: "Demo".to_string(),
                root_path: root_path.display().to_string(),
                work_item_prefix: None,
            })
            .unwrap();

        let conn = state.connect_internal().unwrap();
        crate::vault::upsert_entry(
            &conn,
            std::path::Path::new(&state.storage().app_data_dir),
            crate::vault::UpsertVaultEntryInput {
                id: None,
                name: VAULT_ENTRY_NAME.to_string(),
                kind: "api-key".to_string(),
                description: Some("voyage test".to_string()),
                scope_tags: vec![],
                gate_policy: Some("auto".to_string()),
                value: Some("test-key".to_string()),
            },
        )
        .unwrap();

        (state, project.id)
    }

    #[test]
    fn embed_text_round_trips_via_stub() {
        let dir = unique_temp_dir("embed-text");
        let (state, _project_id) = new_state(&dir);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let stub = StubVoyage {
            calls: calls.clone(),
            dims: VOYAGE_DIMENSIONS,
        };
        let svc = EmbeddingsService::with_client(state, Box::new(stub));

        let embedding = svc
            .embed_text(VOYAGE_MODEL, "hello world", EmbeddingInputType::Document)
            .expect("embed_text succeeds");

        assert_eq!(embedding.len(), VOYAGE_DIMENSIONS);
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, VOYAGE_MODEL);
        assert!(matches!(calls[0].1, EmbeddingInputType::Document));
        assert_eq!(calls[0].2, vec!["hello world".to_string()]);
    }

    #[test]
    fn embed_work_item_skips_when_hash_unchanged() {
        let dir = unique_temp_dir("skip-unchanged");
        let (state, project_id) = new_state(&dir);
        let item = state
            .create_work_item(CreateWorkItemInput {
                project_id,
                parent_work_item_id: None,
                title: "Refresh architecture".to_string(),
                body: "Investigate targeted refresh churn.".to_string(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .unwrap();

        let calls = Arc::new(Mutex::new(Vec::new()));
        let stub = StubVoyage {
            calls: calls.clone(),
            dims: VOYAGE_DIMENSIONS,
        };
        let svc = EmbeddingsService::with_client(state, Box::new(stub));

        let first = svc.embed_work_item(item.id).unwrap();
        assert!(first.changed, "first embed writes a row");

        let second = svc.embed_work_item(item.id).unwrap();
        assert!(!second.changed, "no changes -> no Voyage call");
        assert_eq!(calls.lock().unwrap().len(), 1, "Voyage called exactly once");
    }

    #[test]
    fn search_round_trip_finds_embedded_item() {
        let dir = unique_temp_dir("search-round-trip");
        let (state, project_id) = new_state(&dir);
        let item = state
            .create_work_item(CreateWorkItemInput {
                project_id,
                parent_work_item_id: None,
                title: "Refresh architecture".to_string(),
                body: "Investigate targeted refresh churn.".to_string(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .unwrap();

        let calls = Arc::new(Mutex::new(Vec::new()));
        let stub = StubVoyage {
            calls: calls.clone(),
            dims: VOYAGE_DIMENSIONS,
        };
        let svc = EmbeddingsService::with_client(state, Box::new(stub));

        svc.embed_work_item(item.id).unwrap();

        // Same text as the stored item — the stub produces identical vectors so
        // the query should land on top of the stored embedding.
        let hits = svc
            .search(
                project_id,
                "Refresh architecture\n\n---\n\nInvestigate targeted refresh churn.",
                5,
                SearchFilters::default(),
            )
            .unwrap();

        assert!(!hits.is_empty(), "at least one hit");
        assert_eq!(hits[0].work_item_id, item.id);
        assert_eq!(hits[0].call_sign, item.call_sign);
        assert!(hits[0].distance >= 0.0);
    }

    #[test]
    fn search_applies_status_filter() {
        let dir = unique_temp_dir("search-status-filter");
        let (state, project_id) = new_state(&dir);
        let done = state
            .create_work_item(CreateWorkItemInput {
                project_id,
                parent_work_item_id: None,
                title: "Already complete".to_string(),
                body: "Finished refresh architecture work.".to_string(),
                item_type: "task".to_string(),
                status: "done".to_string(),
            })
            .unwrap();

        let calls = Arc::new(Mutex::new(Vec::new()));
        let stub = StubVoyage {
            calls,
            dims: VOYAGE_DIMENSIONS,
        };
        let svc = EmbeddingsService::with_client(state, Box::new(stub));
        svc.embed_work_item(done.id).unwrap();

        let hits = svc
            .search(
                project_id,
                "refresh",
                5,
                SearchFilters {
                    status: None,
                    item_type: None,
                    open_only: Some(true),
                },
            )
            .unwrap();

        assert!(
            hits.is_empty(),
            "open_only filter must drop done work items, got {hits:?}"
        );
    }

    #[test]
    fn delete_work_item_clears_vector_row() {
        let dir = unique_temp_dir("delete-clears-vec");
        let (state, project_id) = new_state(&dir);
        let item = state
            .create_work_item(CreateWorkItemInput {
                project_id,
                parent_work_item_id: None,
                title: "Transient".to_string(),
                body: "Will be deleted.".to_string(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .unwrap();

        let stub = StubVoyage {
            calls: Arc::new(Mutex::new(Vec::new())),
            dims: VOYAGE_DIMENSIONS,
        };
        let svc = EmbeddingsService::with_client(state.clone(), Box::new(stub));
        svc.embed_work_item(item.id).unwrap();

        state.delete_work_item(item.id).unwrap();

        let conn = state.connect_internal().unwrap();
        let metadata_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM work_item_embeddings WHERE work_item_id = ?1",
                [item.id],
                |row| row.get(0),
            )
            .unwrap();
        let vector_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM work_item_vectors WHERE work_item_id = ?1",
                [item.id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(metadata_count, 0, "cascade drops work_item_embeddings");
        assert_eq!(vector_count, 0, "explicit delete drops work_item_vectors");
    }
}
