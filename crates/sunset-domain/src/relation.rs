use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ConceptId, RelationId, TurnId};

/// Constrained relation vocabulary (initial spec Part II §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationKind {
    IsA,
    PartOf,
    Uses,
    DependsOn,
    Explains,
    RelatedTo,
    ContrastsWith,
    AppearsInSource,
}

/// A typed, directed edge between two concepts: `from --kind--> to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub id: RelationId,
    pub from: ConceptId,
    pub to: ConceptId,
    pub kind: RelationKind,
    /// Human-editable label, e.g. "learns representation".
    pub label: Option<String>,
    pub evidence_turn_ids: Vec<TurnId>,
}
