use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{CommitId, JobId, Proposal, SessionId, Turn, TurnId};

/// Everything the core tells its clients. The desktop shell forwards these as one Tauri event;
/// a server would forward them over WebSocket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AppEvent {
    /// A chunk of the tutor answer being streamed for `turn_id`.
    TutorDelta {
        session_id: SessionId,
        turn_id: TurnId,
        text: String,
    },
    /// The tutor could not answer. Nothing was stored for `turn_id`.
    TutorFailed {
        session_id: SessionId,
        turn_id: TurnId,
        message: String,
    },
    /// A user or tutor turn was stored.
    TurnFinalized {
        turn: Turn,
    },
    ProposalCreated {
        proposal: Proposal,
    },
    /// Status changed (committed, rejected, undone).
    ProposalUpdated {
        proposal: Proposal,
    },
    /// The wiki or graph changed. Refetch.
    KnowledgeChanged {
        commit_id: CommitId,
    },
    /// A background job gave up. The tutor answer is unaffected.
    JobFailed {
        job_id: JobId,
        message: String,
    },
}
