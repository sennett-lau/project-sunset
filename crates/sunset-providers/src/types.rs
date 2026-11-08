use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }
}

/// What shape the answer must have. Adapters pick the best mechanism the model supports
/// (native JSON schema, a tool call, or instructions in the prompt); callers still validate.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ResponseFormat {
    #[default]
    Text,
    Json {
        /// Short identifier, e.g. `curator_output`.
        name: String,
        schema: serde_json::Value,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub response_format: ResponseFormat,
    pub temperature: Option<f32>,
}

impl ChatRequest {
    /// The newest user message, if any.
    pub fn last_user_message(&self) -> Option<&str> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatResponse {
    pub content: String,
    /// The content came from a forced tool call (the structured-output fallback).
    pub via_tool_call: bool,
}

/// One streamed chunk of text.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatDelta {
    pub text: String,
}

/// Whether a feature works. Unknown until probed (docs/architecture.md §4.4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Capability {
    Supported,
    Unsupported,
    #[default]
    NeedsTest,
}

/// Unknown capabilities default to `NeedsTest`; adapters then take the safest path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatCapabilities {
    pub streaming: Capability,
    pub tool_calls: Capability,
    pub json_schema: Capability,
    pub context_window: Option<u32>,
}
