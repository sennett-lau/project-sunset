//! A deterministic model for tests and Phase 0. The caller supplies the answers, so this crate
//! stays free of any product knowledge.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;

use crate::{
    Capability, ChatCapabilities, ChatDelta, ChatModel, ChatRequest, ChatResponse, ChatStream,
    Result,
};

type Responder = dyn Fn(&ChatRequest) -> String + Send + Sync;

pub struct MockChatModel {
    id: String,
    responder: Arc<Responder>,
    chunk_delay: Duration,
    capabilities: ChatCapabilities,
}

impl MockChatModel {
    /// `responder` returns the full answer for a request. Streaming splits it into words.
    pub fn new(
        id: impl Into<String>,
        responder: impl Fn(&ChatRequest) -> String + Send + Sync + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            responder: Arc::new(responder),
            chunk_delay: Duration::ZERO,
            capabilities: ChatCapabilities {
                streaming: Capability::Supported,
                tool_calls: Capability::Unsupported,
                json_schema: Capability::Supported,
                context_window: None,
            },
        }
    }

    /// Pause between streamed chunks, so the UI shows typing. Zero in tests.
    pub fn with_chunk_delay(mut self, delay: Duration) -> Self {
        self.chunk_delay = delay;
        self
    }
}

#[async_trait]
impl ChatModel for MockChatModel {
    fn model_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &ChatCapabilities {
        &self.capabilities
    }

    async fn complete(&self, request: ChatRequest) -> Result<ChatResponse> {
        Ok(ChatResponse {
            content: (self.responder)(&request),
            via_tool_call: false,
        })
    }

    async fn stream(&self, request: ChatRequest) -> Result<ChatStream> {
        let chunks = split_words((self.responder)(&request));
        let delay = self.chunk_delay;
        Ok(futures::stream::iter(chunks)
            .then(move |text| async move {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                Ok(ChatDelta { text })
            })
            .boxed())
    }
}

/// Split into chunks that each end after a run of whitespace, so joining them restores the text.
fn split_words(text: String) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut in_space = false;
    for c in text.chars() {
        if in_space && !c.is_whitespace() {
            chunks.push(std::mem::take(&mut current));
        }
        in_space = c.is_whitespace();
        current.push(c);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChatMessage;

    #[tokio::test]
    async fn streams_the_full_answer_in_chunks() {
        let model = MockChatModel::new("mock/echo", |req| {
            format!("You said: {}", req.last_user_message().unwrap_or(""))
        });
        let req = ChatRequest {
            messages: vec![ChatMessage::user("hello there")],
            ..Default::default()
        };
        let chunks: Vec<String> = model
            .stream(req.clone())
            .await
            .unwrap()
            .map(|d| d.unwrap().text)
            .collect()
            .await;
        assert_eq!(chunks, ["You ", "said: ", "hello ", "there"]);
        assert_eq!(model.complete(req).await.unwrap().content, chunks.concat());
    }
}
