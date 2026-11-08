//! Plain data types shared by every Sunset crate. No I/O and no business logic.
//!
//! Types that cross into the UI derive `specta::Type`; their TypeScript versions are generated
//! into `packages/bindings` (`pnpm gen:bindings`). Product terms follow initial spec Appendix A.

mod article;
mod commit;
mod concept;
mod events;
mod identity;
mod ids;
mod observation;
mod proposal;
mod provider;
mod relation;
mod session;
mod settings;
mod views;
mod visual;

pub use article::ArticleRevision;
pub use commit::Commit;
pub use concept::{Concept, slugify};
pub use events::AppEvent;
pub use identity::{AppIdentity, IdentityError};
pub use ids::{
    ArtifactId, CommitId, ConceptId, JobId, ObservationId, ProposalId, ProviderId, RelationId,
    RevisionId, SessionId, TurnId, UserId,
};
pub use observation::{LearningObservation, ObservationKind, ObservationSource};
pub use proposal::{Proposal, ProposalStatus, ProposedChange};
pub use provider::{
    CapabilityStatus, ModelCapabilities, ModelSettings, ProbeReport, ProviderInput, ProviderKind,
    ProviderProfile, Role, RoleAssignment,
};
pub use relation::{Relation, RelationKind};
pub use session::{Session, Speaker, Turn};
pub use settings::SaveMode;
pub use views::{ConceptPage, ConceptSummary, GraphView, RelationView, SendMessageResult};
pub use visual::{
    DataSeries, EquationStep, ExplorerPreset, FlowEdge, FlowNode, PlotPoint, PlotType, SliderParam,
    VisualArtifact, VisualPayload,
};

/// All timestamps are UTC. Serialized as RFC 3339 strings.
pub type Timestamp = chrono::DateTime<chrono::Utc>;
