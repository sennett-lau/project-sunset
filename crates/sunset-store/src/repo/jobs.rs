//! Durable background jobs. A job row survives restarts; the core re-runs unfinished ones.

use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use sunset_domain::JobId;

use crate::{Result, codec};

#[derive(Debug, Clone, PartialEq)]
pub struct JobRow {
    pub id: JobId,
    pub kind: String,
    pub payload: serde_json::Value,
    pub attempts: i64,
}

/// Queue a job unless one with the same idempotency key exists. Returns the job id either way.
pub async fn enqueue(
    conn: &mut SqliteConnection,
    kind: &str,
    payload: &serde_json::Value,
    idempotency_key: &str,
) -> Result<JobId> {
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO jobs (id, kind, payload_json, status, idempotency_key, created_at, updated_at) \
         VALUES (?, ?, ?, 'queued', ?, ?, ?) ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(JobId::new().to_string())
    .bind(kind)
    .bind(payload.to_string())
    .bind(idempotency_key)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    let row = sqlx::query("SELECT id FROM jobs WHERE idempotency_key = ?")
        .bind(idempotency_key)
        .fetch_one(&mut *conn)
        .await?;
    codec::id(&row, "id")
}

/// Mark a queued (or interrupted) job as running and count the attempt. Returns `None` if the
/// job is already done or failed.
pub async fn start(conn: &mut SqliteConnection, id: JobId) -> Result<Option<JobRow>> {
    let row = sqlx::query(
        "UPDATE jobs SET status = 'running', attempts = attempts + 1, updated_at = ? \
         WHERE id = ? AND status IN ('queued', 'running') RETURNING *",
    )
    .bind(Utc::now())
    .bind(id.to_string())
    .fetch_optional(&mut *conn)
    .await?;
    row.map(|r| from_row(&r)).transpose()
}

pub async fn finish(conn: &mut SqliteConnection, id: JobId, error: Option<&str>) -> Result<()> {
    sqlx::query("UPDATE jobs SET status = ?, last_error = ?, updated_at = ? WHERE id = ?")
        .bind(if error.is_some() { "failed" } else { "done" })
        .bind(error)
        .bind(Utc::now())
        .bind(id.to_string())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Jobs that were queued or running when the app last stopped, oldest first.
pub async fn unfinished(conn: &mut SqliteConnection) -> Result<Vec<JobId>> {
    sqlx::query("SELECT id FROM jobs WHERE status IN ('queued', 'running') ORDER BY created_at")
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(|r| codec::id(r, "id"))
        .collect()
}

fn from_row(row: &SqliteRow) -> Result<JobRow> {
    Ok(JobRow {
        id: codec::id(row, "id")?,
        kind: row.try_get("kind")?,
        payload: codec::json(row, "payload_json")?,
        attempts: row.try_get("attempts")?,
    })
}
