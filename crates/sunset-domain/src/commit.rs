use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{CommitId, ProposalId, Timestamp};

/// An append-only record of applied changes. Undo is a new commit with `undo_of` set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub id: CommitId,
    pub proposal_id: ProposalId,
    pub undo_of: Option<CommitId>,
    pub created_at: Timestamp,
}
