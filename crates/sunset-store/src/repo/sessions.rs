use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use sunset_domain::{Session, SessionId, Speaker, Turn, TurnId, UserId};

use crate::{Result, codec};

pub async fn create(conn: &mut SqliteConnection, user: UserId) -> Result<Session> {
    let session = Session {
        id: SessionId::new(),
        created_at: Utc::now(),
    };
    sqlx::query("INSERT INTO sessions (id, user_id, created_at) VALUES (?, ?, ?)")
        .bind(session.id.to_string())
        .bind(user.to_string())
        .bind(session.created_at)
        .execute(&mut *conn)
        .await?;
    Ok(session)
}

pub async fn exists(conn: &mut SqliteConnection, id: SessionId) -> Result<bool> {
    let row = sqlx::query("SELECT 1 FROM sessions WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(&mut *conn)
        .await?;
    Ok(row.is_some())
}

/// Insert a finalized turn. `id` is chosen by the caller so streamed deltas can refer to it
/// before the turn is stored.
pub async fn insert_turn(
    conn: &mut SqliteConnection,
    id: TurnId,
    session_id: SessionId,
    speaker: Speaker,
    text: &str,
) -> Result<Turn> {
    let turn = Turn {
        id,
        session_id,
        speaker,
        text: text.to_owned(),
        created_at: Utc::now(),
    };
    sqlx::query(
        "INSERT INTO turns (id, session_id, speaker, text, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(turn.id.to_string())
    .bind(turn.session_id.to_string())
    .bind(codec::tag(&turn.speaker)?)
    .bind(&turn.text)
    .bind(turn.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(turn)
}

pub async fn get_turn(conn: &mut SqliteConnection, id: TurnId) -> Result<Option<Turn>> {
    sqlx::query("SELECT * FROM turns WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| turn_from_row(&r))
        .transpose()
}

pub async fn list_turns(conn: &mut SqliteConnection, session_id: SessionId) -> Result<Vec<Turn>> {
    sqlx::query("SELECT * FROM turns WHERE session_id = ? ORDER BY created_at, id")
        .bind(session_id.to_string())
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(turn_from_row)
        .collect()
}

fn turn_from_row(row: &SqliteRow) -> Result<Turn> {
    Ok(Turn {
        id: codec::id(row, "id")?,
        session_id: codec::id(row, "session_id")?,
        speaker: codec::from_tag(row, "speaker")?,
        text: row.try_get("text")?,
        created_at: row.try_get("created_at")?,
    })
}
