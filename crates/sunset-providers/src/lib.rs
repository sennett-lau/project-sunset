//! AI model access: provider traits, adapters, capability probing and role routing.
//!
//! This crate is a plain client library. It must not know about concepts, articles or proposals.
//! See `docs/architecture.md` §4.

pub mod adapters;
mod traits;
mod types;

pub use traits::{ChatModel, ChatStream, Embedder};
pub use types::{
    Capability, ChatCapabilities, ChatDelta, ChatMessage, ChatRequest, ChatResponse, ChatRole,
    ResponseFormat,
};

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("request to the model failed: {0}")]
    Request(String),
    #[error("the model does not support {0}")]
    Unsupported(&'static str),
    #[error("the model returned an unusable response: {0}")]
    BadResponse(String),
}

pub type Result<T, E = ProviderError> = std::result::Result<T, E>;
