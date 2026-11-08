//! Which model serves each role, and the model settings the user edits
//! (docs/architecture.md §4.3–4.5).
//!
//! There is no fallback: if a role's model fails, the error reaches the user. Nothing is ever
//! re-sent to another provider on its own.

use std::sync::Arc;

use chrono::Utc;
use sunset_domain::{
    CapabilityStatus, ModelCapabilities, ModelSettings, ProbeReport, ProviderId, ProviderInput,
    ProviderKind, ProviderProfile, Role, RoleAssignment,
};
use sunset_providers::adapters::openai_compat::{
    OpenAiCompatConfig, OpenAiCompatModel, list_models as fetch_models,
};
use sunset_providers::local::is_local_url;
use sunset_providers::probe::probe_openai_compat;
use sunset_providers::{Capability, ChatCapabilities, ChatModel};
use sunset_store::repo::models::{self as repo, ProviderRecord};

use crate::{CoreError, Result, Sunset, mock};

const DEMO_NAME: &str = "Demo (offline, scripted)";

/// First run: create the Demo provider and point every unassigned role at it, so the app works
/// before any model is configured.
pub(crate) async fn ensure_defaults(app: &Sunset) -> Result<()> {
    let inner = &app.inner;
    let mut conn = inner.store.conn().await?;
    let providers = repo::list_providers(&mut conn, inner.user).await?;
    let demo = match providers.iter().find(|p| p.kind == ProviderKind::Demo) {
        Some(p) => p.id,
        None => {
            repo::insert_provider(
                &mut conn,
                inner.user,
                ProviderKind::Demo,
                DEMO_NAME,
                "",
                false,
            )
            .await?
            .id
        }
    };
    for role in Role::ALL {
        if repo::role(&mut conn, inner.user, role).await?.is_none() {
            repo::set_role(&mut conn, inner.user, role, demo, mock::DEMO_MODEL).await?;
        }
    }
    Ok(())
}

/// The model currently assigned to `role`.
pub(crate) async fn model_for(app: &Sunset, role: Role) -> Result<Arc<dyn ChatModel>> {
    let inner = &app.inner;
    let mut conn = inner.store.conn().await?;
    let (provider_id, model) = repo::role(&mut conn, inner.user, role)
        .await?
        .ok_or_else(|| CoreError::Conflict(format!("no model is set for the {role:?} role")))?;
    let provider = repo::provider(&mut conn, provider_id)
        .await?
        .ok_or(CoreError::NotFound("provider"))?;
    match provider.kind {
        ProviderKind::Demo => Ok(mock::model(role, inner.demo_chunk_delay)),
        ProviderKind::OpenaiCompatible => {
            let caps = repo::capabilities(&mut conn, provider_id, &model).await?;
            drop(conn);
            let api_key = api_key(app, &provider).await?;
            Ok(Arc::new(OpenAiCompatModel::new(
                inner.http.clone(),
                OpenAiCompatConfig {
                    base_url: provider.base_url,
                    api_key,
                    model,
                    capabilities: chat_caps(&caps),
                },
            )))
        }
    }
}

pub(crate) async fn settings(app: &Sunset) -> Result<ModelSettings> {
    let inner = &app.inner;
    let mut conn = inner.store.conn().await?;
    let providers: Vec<ProviderProfile> = repo::list_providers(&mut conn, inner.user)
        .await?
        .into_iter()
        .map(profile)
        .collect();
    let mut roles = Vec::new();
    for (role, provider_id, model) in repo::roles(&mut conn, inner.user).await? {
        let capabilities = match providers.iter().find(|p| p.id == provider_id) {
            Some(p) if p.kind == ProviderKind::Demo => demo_caps(),
            _ => repo::capabilities(&mut conn, provider_id, &model).await?,
        };
        roles.push(RoleAssignment {
            role,
            provider_id,
            model,
            capabilities,
        });
    }
    roles.sort_by_key(|r| Role::ALL.iter().position(|x| *x == r.role));
    let all_local = roles.iter().all(|r| {
        providers
            .iter()
            .any(|p| p.id == r.provider_id && p.is_local)
    });
    Ok(ModelSettings {
        providers,
        roles,
        all_local,
    })
}

/// Create (`id` = None) or edit an OpenAI-compatible provider. The API key, if given, goes to the
/// Keychain and is never returned.
pub(crate) async fn save_provider(
    app: &Sunset,
    id: Option<ProviderId>,
    input: ProviderInput,
) -> Result<ProviderProfile> {
    let inner = &app.inner;
    let name = input.name.trim();
    let base_url = input.base_url.trim().trim_end_matches('/');
    if name.is_empty() {
        return Err(CoreError::Invalid("name is empty".into()));
    }
    let parsed =
        url::Url::parse(base_url).map_err(|e| CoreError::Invalid(format!("base URL: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(CoreError::Invalid(
            "base URL must start with http:// or https://".into(),
        ));
    }

    let mut conn = inner.store.conn().await?;
    let existing = match id {
        Some(id) => Some(
            repo::provider(&mut conn, id)
                .await?
                .ok_or(CoreError::NotFound("provider"))?,
        ),
        None => None,
    };
    if existing
        .as_ref()
        .is_some_and(|p| p.kind == ProviderKind::Demo)
    {
        return Err(CoreError::Conflict(
            "the Demo provider cannot be edited".into(),
        ));
    }
    let id = match &existing {
        Some(p) => p.id,
        None => {
            repo::insert_provider(
                &mut conn,
                inner.user,
                ProviderKind::OpenaiCompatible,
                name,
                base_url,
                false,
            )
            .await?
            .id
        }
    };
    drop(conn);

    let mut has_key = existing.as_ref().is_some_and(|p| p.has_api_key);
    if let Some(key) = input.api_key {
        let key = key.trim().to_owned();
        let account = id.to_string();
        let secrets = inner.secrets.clone();
        has_key = !key.is_empty();
        tokio::task::spawn_blocking(move || {
            if key.is_empty() {
                secrets.delete(&account)
            } else {
                secrets.set(&account, &key)
            }
        })
        .await
        .map_err(|e| CoreError::Conflict(e.to_string()))??;
        inner.key_cache.lock().expect("lock").remove(&id);
    }

    let mut conn = inner.store.conn().await?;
    repo::update_provider(&mut conn, id, name, base_url, has_key).await?;
    let saved = repo::provider(&mut conn, id)
        .await?
        .ok_or(CoreError::NotFound("provider"))?;
    Ok(profile(saved))
}

pub(crate) async fn delete_provider(app: &Sunset, id: ProviderId) -> Result<()> {
    let inner = &app.inner;
    let mut conn = inner.store.conn().await?;
    let provider = repo::provider(&mut conn, id)
        .await?
        .ok_or(CoreError::NotFound("provider"))?;
    if provider.kind == ProviderKind::Demo {
        return Err(CoreError::Conflict(
            "the Demo provider cannot be removed".into(),
        ));
    }
    let in_use: Vec<String> = repo::roles(&mut conn, inner.user)
        .await?
        .into_iter()
        .filter(|(_, p, _)| *p == id)
        .map(|(role, _, _)| format!("{role:?}").to_lowercase())
        .collect();
    if !in_use.is_empty() {
        return Err(CoreError::Conflict(format!(
            "still used by the {} role; pick another model for it first",
            in_use.join(" and ")
        )));
    }
    repo::delete_provider(&mut conn, id).await?;
    drop(conn);
    let secrets = inner.secrets.clone();
    let account = id.to_string();
    tokio::task::spawn_blocking(move || secrets.delete(&account))
        .await
        .map_err(|e| CoreError::Conflict(e.to_string()))??;
    inner.key_cache.lock().expect("lock").remove(&id);
    Ok(())
}

pub(crate) async fn list_models(app: &Sunset, id: ProviderId) -> Result<Vec<String>> {
    let provider = load(app, id).await?;
    match provider.kind {
        ProviderKind::Demo => Ok(vec![mock::DEMO_MODEL.to_owned()]),
        ProviderKind::OpenaiCompatible => {
            let key = api_key(app, &provider).await?;
            Ok(fetch_models(&app.inner.http, &provider.base_url, key.as_deref()).await?)
        }
    }
}

/// Probe `model` on a provider and remember what it supports.
pub(crate) async fn test_connection(
    app: &Sunset,
    id: ProviderId,
    model: &str,
) -> Result<ProbeReport> {
    let model = model.trim();
    if model.is_empty() {
        return Err(CoreError::Invalid("model is empty".into()));
    }
    let provider = load(app, id).await?;
    if provider.kind == ProviderKind::Demo {
        return Ok(ProbeReport {
            provider_id: id,
            model: model.to_owned(),
            reachable: true,
            capabilities: demo_caps(),
            notes: vec!["Demo model: scripted, always available".into()],
        });
    }
    let key = api_key(app, &provider).await?;
    let outcome =
        probe_openai_compat(&app.inner.http, &provider.base_url, key.as_deref(), model).await;
    let capabilities = ModelCapabilities {
        streaming: status(outcome.capabilities.streaming),
        json_schema: status(outcome.capabilities.json_schema),
        tool_calls: status(outcome.capabilities.tool_calls),
        probed_at: outcome.reachable.then(Utc::now),
    };
    if outcome.reachable {
        let mut conn = app.inner.store.conn().await?;
        repo::set_capabilities(&mut conn, id, model, &capabilities).await?;
    }
    Ok(ProbeReport {
        provider_id: id,
        model: model.to_owned(),
        reachable: outcome.reachable,
        capabilities,
        notes: outcome.notes,
    })
}

pub(crate) async fn set_role(app: &Sunset, role: Role, id: ProviderId, model: &str) -> Result<()> {
    let model = model.trim();
    if model.is_empty() {
        return Err(CoreError::Invalid("model is empty".into()));
    }
    load(app, id).await?;
    let mut conn = app.inner.store.conn().await?;
    Ok(repo::set_role(&mut conn, app.inner.user, role, id, model).await?)
}

async fn load(app: &Sunset, id: ProviderId) -> Result<ProviderRecord> {
    repo::provider(&mut *app.inner.store.conn().await?, id)
        .await?
        .ok_or(CoreError::NotFound("provider"))
}

/// The provider's API key, cached after the first Keychain read.
async fn api_key(app: &Sunset, provider: &ProviderRecord) -> Result<Option<String>> {
    if !provider.has_api_key {
        return Ok(None);
    }
    let inner = &app.inner;
    if let Some(key) = inner.key_cache.lock().expect("lock").get(&provider.id) {
        return Ok(key.clone());
    }
    let secrets = inner.secrets.clone();
    let account = provider.id.to_string();
    let key = tokio::task::spawn_blocking(move || secrets.get(&account))
        .await
        .map_err(|e| CoreError::Conflict(e.to_string()))??;
    inner
        .key_cache
        .lock()
        .expect("lock")
        .insert(provider.id, key.clone());
    Ok(key)
}

fn profile(p: ProviderRecord) -> ProviderProfile {
    ProviderProfile {
        is_local: p.kind == ProviderKind::Demo || is_local_url(&p.base_url),
        id: p.id,
        name: p.name,
        kind: p.kind,
        base_url: p.base_url,
        has_api_key: p.has_api_key,
        created_at: p.created_at,
    }
}

fn demo_caps() -> ModelCapabilities {
    ModelCapabilities {
        streaming: CapabilityStatus::Supported,
        json_schema: CapabilityStatus::Supported,
        tool_calls: CapabilityStatus::Unsupported,
        probed_at: None,
    }
}

fn status(c: Capability) -> CapabilityStatus {
    match c {
        Capability::Supported => CapabilityStatus::Supported,
        Capability::Unsupported => CapabilityStatus::Unsupported,
        Capability::NeedsTest => CapabilityStatus::NeedsTest,
    }
}

fn capability(c: CapabilityStatus) -> Capability {
    match c {
        CapabilityStatus::Supported => Capability::Supported,
        CapabilityStatus::Unsupported => Capability::Unsupported,
        CapabilityStatus::NeedsTest => Capability::NeedsTest,
    }
}

fn chat_caps(c: &ModelCapabilities) -> ChatCapabilities {
    ChatCapabilities {
        streaming: capability(c.streaming),
        json_schema: capability(c.json_schema),
        tool_calls: capability(c.tool_calls),
        context_window: None,
    }
}
