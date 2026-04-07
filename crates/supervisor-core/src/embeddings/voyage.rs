//! Minimal HTTP client for the Voyage AI embeddings API.
//!
//! POST https://api.voyageai.com/v1/embeddings
//! Documentation: https://docs.voyageai.com/reference/embeddings-api

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default embedding model used when none is specified.
pub const DEFAULT_MODEL: &str = "voyage-3";

/// Voyage AI embeddings API endpoint.
const ENDPOINT: &str = "https://api.voyageai.com/v1/embeddings";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum VoyageError {
    #[error("vault is locked — unlock the vault to use embeddings")]
    VaultLocked,

    #[error("VOYAGE_API_KEY not found in vault — set it via emery_vault_set")]
    KeyMissing,

    #[error("HTTP error from Voyage API ({status}): {body}")]
    HttpError { status: u16, body: String },

    #[error("rate limited by Voyage API (HTTP 429)")]
    RateLimited,

    #[error("unexpected Voyage API response: {0}")]
    BadResponse(String),

    #[error("network error calling Voyage API: {0}")]
    Network(String),
}

// ---------------------------------------------------------------------------
// Request / response shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct EmbeddingRequest<'a> {
    input: &'a [String],
    model: &'a str,
    input_type: &'static str,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    // index is present in the response but unused here
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Thin synchronous client for the Voyage AI embeddings API.
pub struct VoyageClient {
    api_key: String,
    client: reqwest::blocking::Client,
}

impl VoyageClient {
    /// Construct a client with the given API key.
    pub fn new(api_key: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest blocking client");
        Self { api_key, client }
    }

    /// Embed a batch of input strings using `model`.
    ///
    /// Returns one `Vec<f32>` per input, in the same order.
    pub fn embed_batch(
        &self,
        inputs: &[String],
        model: &str,
    ) -> Result<Vec<Vec<f32>>, VoyageError> {
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let body = EmbeddingRequest {
            input: inputs,
            model,
            input_type: "document",
        };

        let response = self
            .client
            .post(ENDPOINT)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| VoyageError::Network(e.to_string()))?;

        let status = response.status().as_u16();

        if status == 429 {
            return Err(VoyageError::RateLimited);
        }

        if !response.status().is_success() {
            let body_text = response.text().unwrap_or_default();
            return Err(VoyageError::HttpError {
                status,
                body: body_text,
            });
        }

        let parsed: EmbeddingResponse = response.json().map_err(|e| {
            VoyageError::BadResponse(format!("failed to parse response JSON: {e}"))
        })?;

        if parsed.data.len() != inputs.len() {
            return Err(VoyageError::BadResponse(format!(
                "expected {} embeddings, got {}",
                inputs.len(),
                parsed.data.len()
            )));
        }

        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }
}

// ---------------------------------------------------------------------------
// Unit tests (mocked via a fake ENDPOINT swap — compile-time only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voyage_error_display() {
        assert!(VoyageError::VaultLocked.to_string().contains("locked"));
        assert!(VoyageError::KeyMissing.to_string().contains("VOYAGE_API_KEY"));
        assert!(VoyageError::RateLimited.to_string().contains("429"));
        let he = VoyageError::HttpError {
            status: 401,
            body: "unauthorized".into(),
        };
        assert!(he.to_string().contains("401"));
        let br = VoyageError::BadResponse("oops".into());
        assert!(br.to_string().contains("oops"));
    }

    #[test]
    fn embed_batch_empty_input_returns_empty() {
        // Does not hit the network — empty input short-circuits.
        let client = VoyageClient::new("fake_key".into());
        let result = client.embed_batch(&[], DEFAULT_MODEL);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
