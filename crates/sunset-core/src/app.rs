use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sunset_domain::{
    AppEvent, AppIdentity, ConceptId, ConceptPage, ConceptSummary, GraphView, ModelSettings,
    ProbeReport, Proposal, ProposalEdit, ProposalId, ProposalStatus, ProposalVersion, ProviderId,
    ProviderInput, ProviderProfile, RelationView, Role, SaveMode, SendMessageResult, SessionId,
    Turn, UserId,
};
use sunset_providers::HttpClient;
use sunset_providers::adapters::openai_compat::http_client;
use sunset_providers::secrets::SecretStore;
use sunset_store::Store;
use sunset_store::repo::{knowledge as kn, proposals, sessions, users};
use tokio::sync::broadcast;

use crate::bus::EventBus;
use crate::{CoreError, Result, conversation, jobs, knowledge, models};

pub struct SunsetConfig {
    pub identity: AppIdentity,
    /// Chosen by the host. On macOS: `~/Library/Application Support/<bundle id>`.
    pub data_dir: PathBuf,
    /// Where API keys live. The desktop app passes the macOS Keychain.
    pub secrets: Arc<dyn SecretStore>,
    /// Pace of the Demo provider's streamed answer. Zero in tests.
    pub demo_chunk_delay: Duration,
}

/// Entry point of the core. Cheap to clone; clones share state.
#[derive(Clone)]
pub struct Sunset {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub knowledge_lock: tokio::sync::Mutex<()>,
    pub vault_lock: tokio::sync::Mutex<()>,
    pub vault_status: Mutex<sunset_domain::VaultStatus>,
    pub identity: AppIdentity,
    pub data_dir: PathBuf,
    pub store: Store,
    pub user: UserId,
    pub bus: EventBus,
    pub http: HttpClient,
    pub secrets: Arc<dyn SecretStore>,
    pub key_cache: Mutex<HashMap<ProviderId, Option<String>>>,
    pub demo_chunk_delay: Duration,
}

impl Sunset {
    /// Open the library in `data_dir`, migrate it, and resume unfinished background jobs.
    pub async fn open(config: SunsetConfig) -> Result<Self> {
        let store = Store::open(&config.data_dir.join("library.sqlite")).await?;
        let (user, _) = users::local_user(&mut *store.conn().await?).await?;
        let sunset = Self {
            inner: Arc::new(Inner {
                knowledge_lock: tokio::sync::Mutex::new(()),
                vault_lock: tokio::sync::Mutex::new(()),
                vault_status: Mutex::new(sunset_domain::VaultStatus {
                    path: config.data_dir.join("vault").to_string_lossy().into(),
                    conflicts: vec![],
                    error: None,
                }),
                identity: config.identity,
                data_dir: std::fs::canonicalize(&config.data_dir)
                    .map_err(sunset_store::StoreError::from)?,
                store,
                user,
                bus: EventBus::new(),
                http: http_client(),
                secrets: config.secrets,
                key_cache: Mutex::new(HashMap::new()),
                demo_chunk_delay: config.demo_chunk_delay,
            }),
        };
        models::ensure_defaults(&sunset).await?;
        let mut tx = sunset.inner.store.begin().await?;
        sunset_store::search::refresh(&mut tx).await?;
        tx.commit().await?;
        sunset.sync_vault().await;
        jobs::resume(&sunset).await?;
        Ok(sunset)
    }

    pub async fn search_knowledge(&self, query: &str, limit: u32) -> Result<Vec<ConceptSummary>> {
        Ok(sunset_store::search::search(
            &mut *self.inner.store.conn().await?,
            self.inner.user,
            query,
            limit,
        )
        .await?)
    }

    pub async fn list_sessions(&self, limit: u32) -> Result<Vec<sunset_domain::Session>> {
        Ok(sessions::list(&mut *self.inner.store.conn().await?, self.inner.user, limit).await?)
    }

    /// Render only approved public knowledge, directly from SQLite. No mirror files are read.
    pub async fn export_public(&self) -> Result<Vec<sunset_domain::PublicExportFile>> {
        let _guard = self.inner.knowledge_lock.lock().await;
        let mut files = vec![];
        for c in self.concepts().await? {
            files.push(sunset_store::vault::public_file(
                &self.concept_page(c.id).await?,
            ));
        }
        files.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(files)
    }

    /// A fresh portable public snapshot under the app-owned exports directory.
    pub async fn export_public_to_folder(&self) -> Result<String> {
        let files = self.export_public().await?;
        let path =
            sunset_store::vault::export_snapshot(&self.inner.data_dir.join("exports"), &files)?;
        Ok(path.to_string_lossy().into_owned())
    }

    pub fn vault_status(&self) -> sunset_domain::VaultStatus {
        self.inner
            .vault_status
            .lock()
            .expect("vault status lock")
            .clone()
    }

    /// Retryable derived output. A mirror failure never misreports a committed DB change as failed.
    pub async fn sync_vault(&self) -> sunset_domain::VaultStatus {
        let _guard = self.inner.knowledge_lock.lock().await;
        self.sync_vault_after_commit().await
    }

    pub(crate) async fn sync_vault_after_commit(&self) -> sunset_domain::VaultStatus {
        let _lock = self.inner.vault_lock.lock().await;
        let result: Result<Vec<String>> = async {
            let mut desired = std::collections::BTreeMap::new();
            for c in self.concepts().await? {
                let page = self.concept_page(c.id).await?;
                let file = sunset_store::vault::public_file(&page);
                if !page.overlay.is_empty() {
                    desired.insert(
                        format!(".private_overlay/{}", file.filename),
                        format!(
                            "# Private learning observations\n\n```json\n{}\n```\n",
                            serde_json::to_string_pretty(&page.overlay)
                                .map_err(sunset_store::StoreError::from)?
                        ),
                    );
                }
                desired.insert(file.filename, file.markdown);
            }
            Ok(sunset_store::vault::sync(
                &mut *self.inner.store.conn().await?,
                &self.inner.data_dir.join("vault"),
                &desired,
            )
            .await?)
        }
        .await;
        let mut status = self.inner.vault_status.lock().expect("vault status lock");
        match result {
            Ok(conflicts) => {
                status.conflicts = conflicts;
                status.error = None;
            }
            Err(e) => status.error = Some(e.to_string()),
        }
        status.clone()
    }

    pub fn identity(&self) -> &AppIdentity {
        &self.inner.identity
    }

    pub fn data_dir(&self) -> &Path {
        &self.inner.data_dir
    }

    /// Receive every event from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.inner.bus.subscribe()
    }

    /// Close the database. Running background tasks may fail afterwards.
    pub async fn close(&self) {
        self.inner.store.close().await;
    }

    // ---- conversation -----------------------------------------------------------------------

    /// Store the user's message and start the tutor answer. Returns at once; the answer arrives
    /// as `tutor_delta` events, then `turn_finalized`. Curation runs after that in the
    /// background and never delays the answer.
    pub async fn send_message(
        &self,
        session_id: Option<SessionId>,
        text: &str,
    ) -> Result<SendMessageResult> {
        conversation::send_message(self, session_id, text).await
    }

    pub async fn turns(&self, session_id: SessionId) -> Result<Vec<Turn>> {
        Ok(sessions::list_turns(&mut *self.inner.store.conn().await?, session_id).await?)
    }

    // ---- proposals --------------------------------------------------------------------------

    pub async fn pending_proposals(&self) -> Result<Vec<Proposal>> {
        let mut conn = self.inner.store.conn().await?;
        Ok(proposals::list_by_status(&mut conn, ProposalStatus::AwaitingApproval).await?)
    }

    /// Newest first, any status. For the review history.
    pub async fn recent_proposals(&self, limit: u32) -> Result<Vec<Proposal>> {
        Ok(proposals::list_recent(&mut *self.inner.store.conn().await?, limit).await?)
    }

    pub async fn approve_proposal(
        &self,
        id: ProposalId,
        expected_revision: u32,
    ) -> Result<Proposal> {
        knowledge::apply::approve(self, id, expected_revision).await
    }

    pub async fn reject_proposal(
        &self,
        id: ProposalId,
        expected_revision: u32,
    ) -> Result<Proposal> {
        knowledge::apply::reject(self, id, expected_revision).await
    }

    pub async fn edit_proposal(&self, id: ProposalId, edit: ProposalEdit) -> Result<Proposal> {
        knowledge::apply::edit(self, id, edit).await
    }

    pub async fn proposal_versions(&self, id: ProposalId) -> Result<Vec<ProposalVersion>> {
        Ok(proposals::versions(&mut *self.inner.store.conn().await?, id).await?)
    }

    /// Undo a committed proposal with a compensating commit.
    pub async fn undo_proposal(&self, id: ProposalId) -> Result<Proposal> {
        knowledge::apply::undo(self, id).await
    }

    pub async fn save_mode(&self) -> Result<SaveMode> {
        Ok(users::save_mode(&mut *self.inner.store.conn().await?, self.inner.user).await?)
    }

    pub async fn set_save_mode(&self, mode: SaveMode) -> Result<()> {
        let mut conn = self.inner.store.conn().await?;
        Ok(users::set_save_mode(&mut conn, self.inner.user, mode).await?)
    }

    // ---- models -----------------------------------------------------------------------------

    /// Resolve the speech destination before capture, keeping it fixed until transcription ends.
    pub async fn prepare_speech(&self) -> Result<crate::PreparedSpeech> {
        models::prepare_speech(self).await
    }

    pub async fn model_settings(&self) -> Result<ModelSettings> {
        models::settings(self).await
    }

    /// Create (`id` = None) or edit a provider. An API key goes to the Keychain only.
    pub async fn save_provider(
        &self,
        id: Option<ProviderId>,
        input: ProviderInput,
    ) -> Result<ProviderProfile> {
        models::save_provider(self, id, input).await
    }

    pub async fn delete_provider(&self, id: ProviderId) -> Result<()> {
        models::delete_provider(self, id).await
    }

    /// Model ids the provider offers, if its server lists them.
    pub async fn list_models(&self, id: ProviderId) -> Result<Vec<String>> {
        models::list_models(self, id).await
    }

    /// Probe what `model` supports and remember it.
    pub async fn test_connection(&self, id: ProviderId, model: &str) -> Result<ProbeReport> {
        models::test_connection(self, id, model).await
    }

    pub async fn set_role(&self, role: Role, id: ProviderId, model: &str) -> Result<()> {
        models::set_role(self, role, id, model).await
    }

    // ---- wiki and graph ---------------------------------------------------------------------

    pub async fn concepts(&self) -> Result<Vec<ConceptSummary>> {
        Ok(kn::list_concepts(&mut *self.inner.store.conn().await?, self.inner.user).await?)
    }

    pub async fn concept_page(&self, id: ConceptId) -> Result<ConceptPage> {
        let mut conn = self.inner.store.conn().await?;
        let concept = kn::concept(&mut conn, id)
            .await?
            .ok_or(CoreError::NotFound("concept"))?;
        let article = kn::current_revision(&mut conn, id).await?;
        let mut outgoing = Vec::new();
        for relation in kn::relations_from(&mut conn, id).await? {
            if let Some(other) = kn::concept(&mut conn, relation.to).await? {
                outgoing.push(RelationView {
                    other: summary(&other),
                    relation,
                });
            }
        }
        let mut incoming = Vec::new();
        for relation in kn::relations_to(&mut conn, id).await? {
            if let Some(other) = kn::concept(&mut conn, relation.from).await? {
                incoming.push(RelationView {
                    other: summary(&other),
                    relation,
                });
            }
        }
        Ok(ConceptPage {
            overlay: kn::observations(&mut conn, id).await?,
            visuals: kn::visuals(&mut conn, id).await?,
            concept,
            article,
            outgoing,
            incoming,
        })
    }

    pub async fn graph(&self) -> Result<GraphView> {
        let mut conn = self.inner.store.conn().await?;
        Ok(GraphView {
            nodes: kn::list_concepts(&mut conn, self.inner.user).await?,
            edges: kn::all_relations(&mut conn, self.inner.user).await?,
        })
    }
}

fn summary(c: &sunset_domain::Concept) -> ConceptSummary {
    ConceptSummary {
        id: c.id,
        slug: c.slug.clone(),
        title: c.title.clone(),
    }
}
