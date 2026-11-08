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

pub use app::{Providers, Sunset, SunsetConfig};
pub use error::{CoreError, Result};
pub use sunset_domain as domain;
pub use sunset_domain::AppIdentity;
