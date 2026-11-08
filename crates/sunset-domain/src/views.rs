//! Read models returned to clients.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    ArticleRevision, Concept, ConceptId, LearningObservation, Relation, SessionId, Turn, TurnId,
    VisualArtifact,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConceptSummary {
    pub id: ConceptId,
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelationView {
    pub relation: Relation,
    /// The concept at the other end.
    pub other: ConceptSummary,
}

/// Everything the wiki shows for one concept. `overlay` is private and shown separately.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConceptPage {
    pub concept: Concept,
    pub article: Option<ArticleRevision>,
    pub outgoing: Vec<RelationView>,
    pub incoming: Vec<RelationView>,
    pub overlay: Vec<LearningObservation>,
    pub visuals: Vec<VisualArtifact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphView {
    pub nodes: Vec<ConceptSummary>,
    pub edges: Vec<Relation>,
}

/// Returned at once by `send_message`; the answer arrives as `tutor_delta` events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResult {
    pub session_id: SessionId,
    pub user_turn: Turn,
    pub tutor_turn_id: TurnId,
}

/// Public Markdown only. Never includes private observations, transcripts or evidence turn IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PublicExportFile {
    pub filename: String,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub path: String,
    /// User-edited or unmanaged paths left untouched.
    pub conflicts: Vec<String>,
    pub error: Option<String>,
}
