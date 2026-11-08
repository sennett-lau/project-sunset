use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::{Sqlite, Transaction, pool::PoolConnection};

use crate::Result;

/// Handle to one library database.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Open (or create) the database file and apply pending migrations.
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(sqlx::Error::Io)?;
        }
        let options = SqliteConnectOptions::from_str("sqlite:")?
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn conn(&self) -> Result<PoolConnection<Sqlite>> {
        Ok(self.pool.acquire().await?)
    }

    /// Start a transaction. Dropping it without [`Tx::commit`] rolls back.
    pub async fn begin(&self) -> Result<Tx> {
        Ok(Tx(self.pool.begin().await?))
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// A database transaction. Derefs to a connection, so `repo` functions accept `&mut tx`.
pub struct Tx(Transaction<'static, Sqlite>);

impl Tx {
    pub async fn commit(self) -> Result<()> {
        Ok(self.0.commit().await?)
    }
}

impl std::ops::Deref for Tx {
    type Target = sqlx::SqliteConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Tx {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
