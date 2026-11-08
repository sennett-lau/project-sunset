use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ConceptId, ObservationId, Timestamp, TurnId};

/// What a personal-overlay entry records. Self-check questions are deliberately absent: they
/// are not in v1 (docs/architecture.md §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    /// A question the learner asked about this concept.
    Question,
    /// Something the learner said they found confusing.
    Confusion,
    /// Self-reported familiarity ("I already know this").
    Familiarity,
    /// An explanation style that helped, e.g. "a concrete analogy".
    PreferredExplanation,
}

/// Where an observation came from (initial spec Part I §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    SelfReported,
    Observed,
    ModelInferred,
}

/// One entry of the private learning overlay. Never part of a shareable article or export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LearningObservation {
    pub id: ObservationId,
    pub concept_id: ConceptId,
    pub kind: ObservationKind,
    pub source: ObservationSource,
    pub note: String,
    /// The turns this observation is based on.
    pub evidence_turn_ids: Vec<TurnId>,
    pub created_at: Timestamp,
}
