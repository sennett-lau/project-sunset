use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::{ChatCapabilities, ChatDelta, ChatRequest, ChatResponse, Result};

pub type ChatStream = BoxStream<'static, Result<ChatDelta>>;

/// A chat/completions model behind any provider.
#[async_trait]
pub trait ChatModel: Send + Sync {
    /// Identifies provider and model, e.g. `ollama/qwen3:8b`. Recorded with results.
    fn model_id(&self) -> &str;

    fn capabilities(&self) -> &ChatCapabilities;

    async fn complete(&self, request: ChatRequest) -> Result<ChatResponse>;

    async fn stream(&self, request: ChatRequest) -> Result<ChatStream>;
}

/// Text embeddings for retrieval.
#[async_trait]
pub trait Embedder: Send + Sync {
    fn model_id(&self) -> &str;

    fn dimension(&self) -> usize;

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}
