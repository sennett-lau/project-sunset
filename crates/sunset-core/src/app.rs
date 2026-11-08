use std::path::{Path, PathBuf};
use std::sync::Arc;

use sunset_domain::{
    AppEvent, AppIdentity, ConceptId, ConceptPage, ConceptSummary, GraphView, Proposal, ProposalId,
    ProposalStatus, RelationView, SaveMode, SendMessageResult, SessionId, Turn, UserId,
};
use sunset_providers::ChatModel;
use sunset_store::Store;
use sunset_store::repo::{knowledge as kn, proposals, sessions, users};
use tokio::sync::broadcast;

use crate::bus::EventBus;
use crate::{CoreError, Result, conversation, jobs, knowledge};

/// The model behind each role (docs/architecture.md §4.3).
#[derive(Clone)]
pub struct Providers {
    pub tutor: Arc<dyn ChatModel>,
    pub curator: Arc<dyn ChatModel>,
}

pub struct SunsetConfig {
    pub identity: AppIdentity,
    /// Chosen by the host. On macOS: `~/Library/Application Support/<bundle id>`.
    pub data_dir: PathBuf,
    pub providers: Providers,
}

/// Entry point of the core. Cheap to clone; clones share state.
#[derive(Clone)]
pub struct Sunset {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub identity: AppIdentity,
    pub data_dir: PathBuf,
    pub store: Store,
    pub user: UserId,
    pub providers: Providers,
    pub bus: EventBus,
}

impl Sunset {
    /// Open the library in `data_dir`, migrate it, and resume unfinished background jobs.
    pub async fn open(config: SunsetConfig) -> Result<Self> {
        let store = Store::open(&config.data_dir.join("library.sqlite")).await?;
        let (user, _) = users::local_user(&mut *store.conn().await?).await?;
        let sunset = Self {
            inner: Arc::new(Inner {
                identity: config.identity,
                data_dir: config.data_dir,
                store,
                user,
                providers: config.providers,
                bus: EventBus::new(),
            }),
        };
        jobs::resume(&sunset).await?;
        Ok(sunset)
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

    pub async fn approve_proposal(&self, id: ProposalId) -> Result<Proposal> {
        knowledge::apply::approve(self, id).await
    }

    pub async fn reject_proposal(&self, id: ProposalId) -> Result<Proposal> {
        knowledge::apply::reject(self, id).await
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
