//! Thin wrappers: parse arguments, call the core, turn errors into messages.

use sunset_core::Sunset;
use sunset_core::domain::{
    ConceptId, ConceptPage, ConceptSummary, GraphView, ModelSettings, ProbeReport, Proposal,
    ProposalId, ProviderId, ProviderInput, ProviderProfile, Role, SaveMode, SendMessageResult,
    SessionId, Turn,
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

#[tauri::command]
#[specta::specta]
pub async fn model_settings(sunset: State<'_, Sunset>) -> CmdResult<ModelSettings> {
    sunset.model_settings().await.map_err(err)
}

/// `id` = null creates a provider. The API key is written to the Keychain and never returned.
#[tauri::command]
#[specta::specta]
pub async fn save_provider(
    sunset: State<'_, Sunset>,
    id: Option<ProviderId>,
    input: ProviderInput,
) -> CmdResult<ProviderProfile> {
    sunset.save_provider(id, input).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_provider(sunset: State<'_, Sunset>, id: ProviderId) -> CmdResult<()> {
    sunset.delete_provider(id).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn list_models(sunset: State<'_, Sunset>, id: ProviderId) -> CmdResult<Vec<String>> {
    sunset.list_models(id).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn test_connection(
    sunset: State<'_, Sunset>,
    id: ProviderId,
    model: String,
) -> CmdResult<ProbeReport> {
    sunset.test_connection(id, &model).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn set_role(
    sunset: State<'_, Sunset>,
    role: Role,
    provider_id: ProviderId,
    model: String,
) -> CmdResult<()> {
    sunset
        .set_role(role, provider_id, &model)
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn search_knowledge(
    sunset: State<'_, Sunset>,
    query: String,
    limit: u32,
) -> CmdResult<Vec<ConceptSummary>> {
    sunset.search_knowledge(&query, limit).await.map_err(err)
}
#[tauri::command]
#[specta::specta]
pub async fn list_sessions(
    sunset: State<'_, Sunset>,
    limit: u32,
) -> CmdResult<Vec<sunset_core::domain::Session>> {
    sunset.list_sessions(limit).await.map_err(err)
}
#[tauri::command]
#[specta::specta]
pub async fn export_public(
    sunset: State<'_, Sunset>,
) -> CmdResult<Vec<sunset_core::domain::PublicExportFile>> {
    sunset.export_public().await.map_err(err)
}
#[tauri::command]
#[specta::specta]
pub async fn sync_vault(sunset: State<'_, Sunset>) -> CmdResult<sunset_core::domain::VaultStatus> {
    Ok(sunset.sync_vault().await)
}
#[tauri::command]
#[specta::specta]
pub async fn vault_status(
    sunset: State<'_, Sunset>,
) -> CmdResult<sunset_core::domain::VaultStatus> {
    Ok(sunset.vault_status())
}

#[tauri::command]
#[specta::specta]
pub async fn export_public_to_folder(sunset: State<'_, Sunset>) -> CmdResult<String> {
    sunset.export_public_to_folder().await.map_err(err)
}
