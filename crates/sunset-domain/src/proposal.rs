use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    CommitId, ObservationKind, ObservationSource, ProposalId, RelationKind, SessionId, Timestamp,
    TurnId, VisualPayload,
};

/// Lifecycle after validation (initial spec Part II §7.4). Candidates that never become
/// proposals stay inside the curator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    AwaitingApproval,
    Committed,
    Rejected,
    Undone,
}

/// A reviewable bundle of changes. The AI only ever produces these; the core applies them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub id: ProposalId,
    pub session_id: SessionId,
    /// Turns the proposal is based on (user question and tutor answer).
    pub evidence_turn_ids: Vec<TurnId>,
    pub summary: String,
    pub changes: Vec<ProposedChange>,
    pub status: ProposalStatus,
    /// Set once committed; undo targets this commit.
    pub commit_id: Option<CommitId>,
    pub created_at: Timestamp,
}

/// One change inside a proposal. Concepts are referred to by slug, because a proposal may
/// create a concept and link to it in the same bundle.
///
/// This is also the shape the curator model must emit, so field names are kept plain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ProposedChange {
    /// Create the concept if no concept has this slug; otherwise add missing aliases.
    UpsertConcept {
        slug: String,
        title: String,
        aliases: Vec<String>,
    },
    /// Add a new article revision.
    ReviseArticle {
        concept: String,
        summary: String,
        body_md: String,
    },
    AddRelation {
        from: String,
        to: String,
        kind: RelationKind,
        label: Option<String>,
    },
    /// Private overlay entry. Evidence turns are filled in by the core, not the model.
    AddObservation {
        concept: String,
        kind: ObservationKind,
        source: ObservationSource,
        note: String,
    },
    AddVisual {
        concept: String,
        title: String,
        payload: VisualPayload,
    },
}
