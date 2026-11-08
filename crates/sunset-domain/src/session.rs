use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{SessionId, Timestamp, TurnId};

/// One conversation episode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: SessionId,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    User,
    Tutor,
}

/// A finalized utterance. Turns are evidence: they are never rewritten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub id: TurnId,
    pub session_id: SessionId,
    pub speaker: Speaker,
    pub text: String,
    pub created_at: Timestamp,
}
