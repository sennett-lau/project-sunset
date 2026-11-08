//! Thin wrappers: parse arguments, call the core, turn errors into messages.

use sunset_core::Sunset;
use sunset_core::domain::{
    ConceptId, ConceptPage, ConceptSummary, GraphView, Proposal, ProposalId, SaveMode,
    SendMessageResult, SessionId, Turn,
};
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err(e: sunset_core::CoreError) -> String {
    e.to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn send_message(
    sunset: State<'_, Sunset>,
    session_id: Option<SessionId>,
    text: String,
) -> CmdResult<SendMessageResult> {
    sunset.send_message(session_id, &text).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn list_turns(sunset: State<'_, Sunset>, session_id: SessionId) -> CmdResult<Vec<Turn>> {
    sunset.turns(session_id).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn pending_proposals(sunset: State<'_, Sunset>) -> CmdResult<Vec<Proposal>> {
    sunset.pending_proposals().await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn recent_proposals(sunset: State<'_, Sunset>, limit: u32) -> CmdResult<Vec<Proposal>> {
    sunset.recent_proposals(limit).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn approve_proposal(sunset: State<'_, Sunset>, id: ProposalId) -> CmdResult<Proposal> {
    sunset.approve_proposal(id).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn reject_proposal(sunset: State<'_, Sunset>, id: ProposalId) -> CmdResult<Proposal> {
    sunset.reject_proposal(id).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn undo_proposal(sunset: State<'_, Sunset>, id: ProposalId) -> CmdResult<Proposal> {
    sunset.undo_proposal(id).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn list_concepts(sunset: State<'_, Sunset>) -> CmdResult<Vec<ConceptSummary>> {
    sunset.concepts().await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn concept_page(sunset: State<'_, Sunset>, id: ConceptId) -> CmdResult<ConceptPage> {
    sunset.concept_page(id).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn graph(sunset: State<'_, Sunset>) -> CmdResult<GraphView> {
    sunset.graph().await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn save_mode(sunset: State<'_, Sunset>) -> CmdResult<SaveMode> {
    sunset.save_mode().await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn set_save_mode(sunset: State<'_, Sunset>, mode: SaveMode) -> CmdResult<()> {
    sunset.set_save_mode(mode).await.map_err(err)
}
