//! SQLite persistence (the source of truth), and later search and the Markdown vault mirror.
//!
//! [`Store`] owns the connection pool. The `repo` modules are plain functions over a
//! `&mut SqliteConnection`, so the core can run several of them in one transaction.
//! See `docs/architecture.md` §3.

mod codec;
mod db;
pub mod repo;

pub use db::{Store, Tx};
pub use sqlx::SqliteConnection;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("stored JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("stored id is invalid: {0}")]
    Id(#[from] uuid::Error),
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;
