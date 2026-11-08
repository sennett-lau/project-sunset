//! All Sunset business logic: conversation, knowledge capture, proposals, revisions and jobs.
//!
//! This crate must not depend on Tauri, axum or any UI. The desktop shell (and later
//! `sunset-server`) construct a [`Sunset`] and call into it. See `docs/architecture.md` §2.2.

use std::path::{Path, PathBuf};

pub use sunset_domain::AppIdentity;

/// Entry point of the core. One instance per running app.
#[derive(Debug)]
pub struct Sunset {
    identity: AppIdentity,
    data_dir: PathBuf,
}

impl Sunset {
    /// `data_dir` is chosen by the host (on macOS: `~/Library/Application Support/<bundle id>`).
    pub fn new(identity: AppIdentity, data_dir: impl Into<PathBuf>) -> Self {
        Self {
            identity,
            data_dir: data_dir.into(),
        }
    }

    pub fn identity(&self) -> &AppIdentity {
        &self.identity
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}
