//! AI model access: provider traits, adapters, capability probing and secrets.
//!
//! This crate is a plain client library. It must not know about concepts, articles or proposals.
//! See `docs/architecture.md` §4.

pub mod adapters;
pub mod local;
pub mod probe;
pub mod secrets;
mod sse;
pub mod structured;
mod traits;
mod types;

pub use traits::{ChatModel, ChatStream, Embedder};
pub use types::{
    Capability, ChatCapabilities, ChatDelta, ChatMessage, ChatRequest, ChatResponse, ChatRole,
    ResponseFormat,
};

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("could not reach the model: {0}")]
    Request(String),
    #[error("{}", status_message(*status, message))]
    Status { status: u16, message: String },
    #[error("the model does not support {0}")]
    Unsupported(&'static str),
    #[error("the model returned an unusable response: {0}")]
    BadResponse(String),
    #[error("keychain error: {0}")]
    Secret(String),
}

fn status_message(status: u16, message: &str) -> String {
    let hint = match status {
        401 | 403 => "the API key was rejected",
        404 => "not found; check the base URL and model name",
        429 => "rate limited or out of quota",
        _ => "the server returned an error",
    };
    if message.is_empty() {
        format!("{hint} (HTTP {status})")
    } else {
        format!("{hint} (HTTP {status}): {message}")
    }
}

impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        // Avoid printing the URL twice; reqwest's Display already includes it.
        Self::Request(e.to_string())
    }
}

pub type Result<T, E = ProviderError> = std::result::Result<T, E>;
