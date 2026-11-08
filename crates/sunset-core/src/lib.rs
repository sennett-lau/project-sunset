//! All Sunset business logic: conversation, knowledge capture, proposals, revisions and jobs.
//!
//! This crate must not depend on Tauri, axum or any UI. The desktop shell (and later
//! `sunset-server`) build a [`Sunset`] and call into it. See `docs/architecture.md` §2.2.

mod app;
mod bus;
mod conversation;
mod error;
mod jobs;
mod knowledge;
pub mod mock;
mod models;

pub use app::{Sunset, SunsetConfig};
pub use error::{CoreError, Result};
pub use models::PreparedSpeech;
pub use sunset_domain as domain;
pub use sunset_domain::AppIdentity;
#[cfg(target_os = "macos")]
pub use sunset_providers::secrets::KeychainSecretStore;
pub use sunset_providers::secrets::{MemorySecretStore, SecretStore};
