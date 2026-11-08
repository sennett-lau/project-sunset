use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use sunset_domain::{CommitId, Proposal, ProposalId, ProposalStatus, UserId};

use crate::{Result, codec};

pub async fn insert(conn: &mut SqliteConnection, user: UserId, p: &Proposal) -> Result<()> {
    sqlx::query(
        "INSERT INTO proposals (id, user_id, session_id, evidence_json, summary, changes_json, \
         status, commit_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(p.id.to_string())
    .bind(user.to_string())
    .bind(p.session_id.to_string())
    .bind(codec::to_json(&p.evidence_turn_ids)?)
    .bind(&p.summary)
    .bind(codec::to_json(&p.changes)?)
    .bind(codec::tag(&p.status)?)
    .bind(p.commit_id.map(|c| c.to_string()))
    .bind(p.created_at)
    .bind(p.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn get(conn: &mut SqliteConnection, id: ProposalId) -> Result<Option<Proposal>> {
    sqlx::query("SELECT * FROM proposals WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| from_row(&r))
        .transpose()
}

/// Newest first.
pub async fn list_by_status(
    conn: &mut SqliteConnection,
    status: ProposalStatus,
) -> Result<Vec<Proposal>> {
    sqlx::query("SELECT * FROM proposals WHERE status = ? ORDER BY created_at DESC, id DESC")
        .bind(codec::tag(&status)?)
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(from_row)
        .collect()
}

/// Newest first, any status.
pub async fn list_recent(conn: &mut SqliteConnection, limit: u32) -> Result<Vec<Proposal>> {
    sqlx::query("SELECT * FROM proposals ORDER BY created_at DESC, id DESC LIMIT ?")
        .bind(limit)
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(from_row)
        .collect()
}

/// Move a proposal from `from` to `to`. Returns false (and changes nothing) if it is not in
/// `from`, so two clicks on Approve cannot both win. Proposal state is a cached view of the
/// commit log, so unlike commits it may be updated.
pub async fn transition(
    conn: &mut SqliteConnection,
    id: ProposalId,
    from: ProposalStatus,
    to: ProposalStatus,
    commit_id: Option<CommitId>,
) -> Result<bool> {
    let done = sqlx::query(
        "UPDATE proposals SET status = ?, commit_id = COALESCE(?, commit_id), updated_at = ? \
         WHERE id = ? AND status = ?",
    )
    .bind(codec::tag(&to)?)
    .bind(commit_id.map(|c| c.to_string()))
    .bind(Utc::now())
    .bind(id.to_string())
    .bind(codec::tag(&from)?)
    .execute(&mut *conn)
    .await?;
    Ok(done.rows_affected() == 1)
}

fn from_row(row: &SqliteRow) -> Result<Proposal> {
    Ok(Proposal {
        id: codec::id(row, "id")?,
        session_id: codec::id(row, "session_id")?,
        evidence_turn_ids: codec::json(row, "evidence_json")?,
        summary: row.try_get("summary")?,
        changes: codec::json(row, "changes_json")?,
        status: codec::from_tag(row, "status")?,
        commit_id: codec::opt_id(row, "commit_id")?,
        created_at: row.try_get("created_at")?,
    })
}
