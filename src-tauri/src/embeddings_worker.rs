//! Background worker that recomputes Voyage embeddings for dirty work items.
//!
//! Consumers push work-item ids to the returned [`mpsc::Sender`]. The worker
//! coalesces notifications inside a short dedup window before spending a
//! Voyage API call. All failures are logged + retried up to three times with
//! exponential backoff; the worker never panics.

use std::collections::HashSet;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use crate::db::AppState;
use crate::embeddings::EmbeddingsService;

/// Time window used to dedup bursts of notifications for the same work item
/// (e.g. create immediately followed by update).
const DEDUP_WINDOW: Duration = Duration::from_millis(250);
const MAX_ATTEMPTS: u32 = 3;
const INITIAL_BACKOFF: Duration = Duration::from_millis(500);

/// Spawn the embeddings worker thread and return the channel sender.
pub fn spawn(state: AppState) -> Sender<i64> {
    let (tx, rx) = mpsc::channel::<i64>();
    thread::Builder::new()
        .name("embeddings-worker".to_string())
        .spawn(move || {
            loop {
                // Block until the first id arrives, then collect any follow-up
                // ids inside the dedup window before processing.
                let Ok(first_id) = rx.recv() else {
                    log::info!("embeddings worker shutting down (sender closed)");
                    return;
                };
                let mut batch: HashSet<i64> = HashSet::new();
                batch.insert(first_id);

                let deadline = std::time::Instant::now() + DEDUP_WINDOW;
                loop {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match rx.recv_timeout(remaining) {
                        Ok(id) => {
                            batch.insert(id);
                        }
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => {
                            log::info!("embeddings worker shutting down (sender closed)");
                            // Drain what we have then exit.
                            process_batch(&state, batch);
                            return;
                        }
                    }
                }

                process_batch(&state, batch);
            }
        })
        .expect("failed to spawn embeddings-worker thread");
    tx
}

fn process_batch(state: &AppState, batch: HashSet<i64>) {
    // Each batch builds its own EmbeddingsService so a transient HTTP client
    // problem doesn't permanently poison the worker.
    let service = match EmbeddingsService::new(state.clone()) {
        Ok(service) => service,
        Err(error) => {
            log::warn!(
                "embeddings worker: failed to build service: {}",
                error.message
            );
            return;
        }
    };

    for id in batch {
        embed_with_retry(&service, id);
    }
}

fn embed_with_retry(service: &EmbeddingsService, work_item_id: i64) {
    let mut attempt = 0_u32;
    let mut delay = INITIAL_BACKOFF;
    loop {
        attempt += 1;
        match service.embed_work_item(work_item_id) {
            Ok(outcome) => {
                if outcome.changed {
                    log::debug!(
                        "embeddings worker: embedded work_item#{} (attempt {})",
                        work_item_id,
                        attempt,
                    );
                }
                return;
            }
            Err(error) => {
                log::warn!(
                    "embeddings worker: work_item#{} attempt {}/{} failed: {}",
                    work_item_id,
                    attempt,
                    MAX_ATTEMPTS,
                    error.message,
                );
                if attempt >= MAX_ATTEMPTS {
                    return;
                }
                thread::sleep(delay);
                delay = delay.saturating_mul(2);
            }
        }
    }
}
