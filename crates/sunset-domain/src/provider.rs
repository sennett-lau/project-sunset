//! Model endpoints, role assignments and probed capabilities (docs/architecture.md §4.3–4.4).
//! API keys are never part of these types except as write-only input.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ProviderId, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Built-in scripted models. Offline; knows one topic. Used until a real model is set up.
    Demo,
    /// Anything that speaks `/v1/chat/completions`: Ollama, LM Studio, llama.cpp, vLLM,
    /// OpenRouter, OpenAI, a self-hosted server.
    OpenaiCompatible,
}

/// A configured endpoint. The API key, if any, lives in the macOS Keychain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: ProviderId,
    pub name: String,
    pub kind: ProviderKind,
    /// e.g. `http://localhost:11434/v1`. Empty for `demo`.
    pub base_url: String,
    pub has_api_key: bool,
    /// The endpoint is on this Mac or a private network, so data does not leave your machines.
    pub is_local: bool,
    pub created_at: Timestamp,
}

/// What the settings form sends. `api_key`: `None` keeps the stored key, `Some("")` removes it.
/// The key goes straight to the Keychain and is never sent back to the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
}

/// A job the app needs a model for. Each role is configured separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Answers and teaches; streamed to the user.
    Tutor,
    /// Turns exchanges into structured proposals.
    Curator,
}

impl Role {
    pub const ALL: [Role; 2] = [Role::Tutor, Role::Curator];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Supported,
    Unsupported,
    /// Not probed yet. The app uses the safest path until "Test connection" runs.
    #[default]
    NeedsTest,
}

/// What a model can do, as found by the last probe.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    pub streaming: CapabilityStatus,
    pub json_schema: CapabilityStatus,
    pub tool_calls: CapabilityStatus,
    pub probed_at: Option<Timestamp>,
}

/// Which provider and model a role uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RoleAssignment {
    pub role: Role,
    pub provider_id: ProviderId,
    pub model: String,
    pub capabilities: ModelCapabilities,
}

/// Everything the model settings screen shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelSettings {
    pub providers: Vec<ProviderProfile>,
    pub roles: Vec<RoleAssignment>,
    /// True only when every role uses a local provider (docs/architecture.md §4.5).
    pub all_local: bool,
}

/// Result of "Test connection".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReport {
    pub provider_id: ProviderId,
    pub model: String,
    /// The model answered a basic request.
    pub reachable: bool,
    pub capabilities: ModelCapabilities,
    /// Human-readable findings, e.g. "JSON schema output: rejected (HTTP 400)".
    pub notes: Vec<String>,
}
