use chrono::Utc;
use sqlx::SqliteConnection;
use sunset_domain::{SaveMode, UserId};

use crate::{Result, codec};

/// The single local user, created on first use.
pub async fn local_user(conn: &mut SqliteConnection) -> Result<(UserId, SaveMode)> {
    if let Some(row) = sqlx::query("SELECT id, save_mode FROM users ORDER BY created_at LIMIT 1")
        .fetch_optional(&mut *conn)
        .await?
    {
        return Ok((codec::id(&row, "id")?, codec::from_tag(&row, "save_mode")?));
    }
    let id = UserId::new();
    let mode = SaveMode::default();
    sqlx::query("INSERT INTO users (id, save_mode, created_at) VALUES (?, ?, ?)")
        .bind(id.to_string())
        .bind(codec::tag(&mode)?)
        .bind(Utc::now())
        .execute(&mut *conn)
        .await?;
    Ok((id, mode))
}

pub async fn set_save_mode(
    conn: &mut SqliteConnection,
    user: UserId,
    mode: SaveMode,
) -> Result<()> {
    sqlx::query("UPDATE users SET save_mode = ? WHERE id = ?")
        .bind(codec::tag(&mode)?)
        .bind(user.to_string())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub async fn save_mode(conn: &mut SqliteConnection, user: UserId) -> Result<SaveMode> {
    let row = sqlx::query("SELECT save_mode FROM users WHERE id = ?")
        .bind(user.to_string())
        .fetch_one(&mut *conn)
        .await?;
    codec::from_tag(&row, "save_mode")
}
