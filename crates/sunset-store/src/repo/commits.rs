use chrono::Utc;
use sqlx::SqliteConnection;
use sunset_domain::{Commit, CommitId, ProposalId, UserId};

use crate::Result;

/// Append a commit. Commits are never updated or deleted.
pub async fn insert(
    conn: &mut SqliteConnection,
    user: UserId,
    proposal_id: ProposalId,
    undo_of: Option<CommitId>,
) -> Result<Commit> {
    let commit = Commit {
        id: CommitId::new(),
        proposal_id,
        undo_of,
        created_at: Utc::now(),
    };
    sqlx::query(
        "INSERT INTO commits (id, user_id, proposal_id, undo_of, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(commit.id.to_string())
    .bind(user.to_string())
    .bind(commit.proposal_id.to_string())
    .bind(commit.undo_of.map(|c| c.to_string()))
    .bind(commit.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(commit)
}
