//! LLM client abstraction for the librarian (EMERY-226.001).
//!
//! Production uses [`AnthropicChatClient`] which delegates to the existing
//! [`crate::embeddings::anthropic::AnthropicClient`]. Tests use [`FakeChatClient`]
//! to drive the pipeline without making real API calls.
//!
//! The trait is intentionally minimal: a single method that takes a model
//! identifier, a max-tokens budget, and a prompt string, and returns the
//! assistant's text response or an error string.

use crate::embeddings::anthropic::{AnthropicClient, AnthropicError};

/// Minimal chat-completion abstraction so the pipeline can be unit-tested
/// without hitting the real Anthropic API.
pub trait ChatClient: Send + Sync {
    fn complete(&self, model: &str, max_tokens: u32, prompt: &str) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// Production: wraps the real Anthropic client
// ---------------------------------------------------------------------------

pub struct AnthropicChatClient {
    inner: AnthropicClient,
}

impl AnthropicChatClient {
    pub fn new(api_key: String) -> Self {
        Self {
            inner: AnthropicClient::new(api_key),
        }
    }
}

impl ChatClient for AnthropicChatClient {
    fn complete(&self, model: &str, max_tokens: u32, prompt: &str) -> Result<String, String> {
        match self.inner.complete_with(model, max_tokens, prompt) {
            Ok(text) => Ok(text),
            Err(AnthropicError::RateLimited) => Err("rate_limited".to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Test fake — scripted responses, no network
// ---------------------------------------------------------------------------

#[cfg(test)]
pub struct FakeChatClient {
    /// Responses returned in order. Each call pops the next entry.
    /// If empty, returns Err("fake_exhausted").
    responses: std::sync::Mutex<std::collections::VecDeque<Result<String, String>>>,
}

#[cfg(test)]
impl FakeChatClient {
    pub fn new(responses: Vec<Result<String, String>>) -> Self {
        Self {
            responses: std::sync::Mutex::new(responses.into()),
        }
    }
}

#[cfg(test)]
impl ChatClient for FakeChatClient {
    fn complete(&self, _model: &str, _max_tokens: u32, _prompt: &str) -> Result<String, String> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err("fake_exhausted".to_string()))
    }
}
