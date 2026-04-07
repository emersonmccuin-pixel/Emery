//! Thin Anthropic Messages API client for memory reconciliation.
//!
//! POST https://api.anthropic.com/v1/messages
//! Used exclusively for the memory reconciliation Haiku call in EMERY-217.003.
//! Mirrors the Voyage client pattern: blocking reqwest, typed errors.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Anthropic Messages API endpoint.
const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

/// Anthropic API version header value.
const API_VERSION: &str = "2023-06-01";

/// Model used for reconciliation decisions.
pub const RECONCILER_MODEL: &str = "claude-haiku-4-5-20251001";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum AnthropicError {
    #[error("vault is locked — unlock the vault to use memory reconciliation")]
    VaultLocked,

    #[error("ANTHROPIC_API_KEY not found in vault — set it via emery_vault_set")]
    KeyMissing,

    #[error("HTTP error from Anthropic API ({status}): {body}")]
    HttpError { status: u16, body: String },

    #[error("rate limited by Anthropic API (HTTP 429)")]
    RateLimited,

    #[error("unexpected Anthropic API response: {0}")]
    BadResponse(String),

    #[error("network error calling Anthropic API: {0}")]
    Network(String),
}

// ---------------------------------------------------------------------------
// Request / response shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<Message<'a>>,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Thin synchronous client for the Anthropic Messages API.
pub struct AnthropicClient {
    api_key: String,
    client: reqwest::blocking::Client,
}

impl AnthropicClient {
    /// Construct a client with the given API key.
    pub fn new(api_key: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest blocking client");
        Self { api_key, client }
    }

    /// Send a single user turn and return the assistant's text response.
    pub fn complete(&self, user_prompt: &str) -> Result<String, AnthropicError> {
        let body = MessagesRequest {
            model: RECONCILER_MODEL,
            max_tokens: 64,
            messages: vec![Message {
                role: "user",
                content: user_prompt,
            }],
        };

        let response = self
            .client
            .post(ENDPOINT)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| AnthropicError::Network(e.to_string()))?;

        let status = response.status().as_u16();

        if status == 429 {
            return Err(AnthropicError::RateLimited);
        }

        if !response.status().is_success() {
            let body_text = response.text().unwrap_or_default();
            return Err(AnthropicError::HttpError {
                status,
                body: body_text,
            });
        }

        let parsed: MessagesResponse = response.json().map_err(|e| {
            AnthropicError::BadResponse(format!("failed to parse response JSON: {e}"))
        })?;

        let text = parsed
            .content
            .into_iter()
            .filter(|b| b.block_type == "text")
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty() {
            return Err(AnthropicError::BadResponse(
                "response contained no text content".to_string(),
            ));
        }

        Ok(text)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_error_display() {
        assert!(AnthropicError::VaultLocked.to_string().contains("locked"));
        assert!(AnthropicError::KeyMissing
            .to_string()
            .contains("ANTHROPIC_API_KEY"));
        assert!(AnthropicError::RateLimited.to_string().contains("429"));
        let he = AnthropicError::HttpError {
            status: 401,
            body: "unauthorized".into(),
        };
        assert!(he.to_string().contains("401"));
        let br = AnthropicError::BadResponse("oops".into());
        assert!(br.to_string().contains("oops"));
    }
}
