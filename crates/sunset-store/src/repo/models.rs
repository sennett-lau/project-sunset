//! Providers, role assignments and probed capabilities. Secrets are not stored here.

use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use sunset_domain::{ModelCapabilities, ProviderId, ProviderKind, Role, Timestamp, UserId};

use crate::{Result, codec};

/// A provider as stored. The core adds derived fields such as "is local".
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRecord {
    pub id: ProviderId,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub has_api_key: bool,
    pub created_at: Timestamp,
}

pub async fn list_providers(
    conn: &mut SqliteConnection,
    user: UserId,
) -> Result<Vec<ProviderRecord>> {
    sqlx::query("SELECT * FROM providers WHERE user_id = ? ORDER BY created_at, id")
        .bind(user.to_string())
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(provider_from_row)
        .collect()
}

pub async fn provider(
    conn: &mut SqliteConnection,
    id: ProviderId,
) -> Result<Option<ProviderRecord>> {
    sqlx::query("SELECT * FROM providers WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| provider_from_row(&r))
        .transpose()
}

pub async fn insert_provider(
    conn: &mut SqliteConnection,
    user: UserId,
    kind: ProviderKind,
    name: &str,
    base_url: &str,
    has_api_key: bool,
) -> Result<ProviderRecord> {
    let record = ProviderRecord {
        id: ProviderId::new(),
        name: name.to_owned(),
        kind,
        base_url: base_url.to_owned(),
        has_api_key,
        created_at: Utc::now(),
    };
    sqlx::query(
        "INSERT INTO providers (id, user_id, name, kind, base_url, has_api_key, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(record.id.to_string())
    .bind(user.to_string())
    .bind(&record.name)
    .bind(codec::tag(&record.kind)?)
    .bind(&record.base_url)
    .bind(record.has_api_key)
    .bind(record.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(record)
}

pub async fn update_provider(
    conn: &mut SqliteConnection,
    id: ProviderId,
    name: &str,
    base_url: &str,
    has_api_key: bool,
) -> Result<()> {
    sqlx::query("UPDATE providers SET name = ?, base_url = ?, has_api_key = ? WHERE id = ?")
        .bind(name)
        .bind(base_url)
        .bind(has_api_key)
        .bind(id.to_string())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub async fn delete_provider(conn: &mut SqliteConnection, id: ProviderId) -> Result<()> {
    sqlx::query("DELETE FROM providers WHERE id = ?")
        .bind(id.to_string())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

fn provider_from_row(row: &SqliteRow) -> Result<ProviderRecord> {
    Ok(ProviderRecord {
        id: codec::id(row, "id")?,
        name: row.try_get("name")?,
        kind: codec::from_tag(row, "kind")?,
        base_url: row.try_get("base_url")?,
        has_api_key: row.try_get("has_api_key")?,
        created_at: row.try_get("created_at")?,
    })
}

// ---- capabilities -----------------------------------------------------------------------------

/// Defaults to "needs test" when the model was never probed.
pub async fn capabilities(
    conn: &mut SqliteConnection,
    provider: ProviderId,
    model: &str,
) -> Result<ModelCapabilities> {
    let row = sqlx::query(
        "SELECT capabilities_json FROM model_capabilities WHERE provider_id = ? AND model = ?",
    )
    .bind(provider.to_string())
    .bind(model)
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some(r) => codec::json(&r, "capabilities_json"),
        None => Ok(ModelCapabilities::default()),
    }
}

pub async fn set_capabilities(
    conn: &mut SqliteConnection,
    provider: ProviderId,
    model: &str,
    caps: &ModelCapabilities,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO model_capabilities (provider_id, model, capabilities_json) VALUES (?, ?, ?) \
         ON CONFLICT (provider_id, model) DO UPDATE SET capabilities_json = excluded.capabilities_json",
    )
    .bind(provider.to_string())
    .bind(model)
    .bind(codec::to_json(caps)?)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

// ---- roles ------------------------------------------------------------------------------------

pub async fn roles(
    conn: &mut SqliteConnection,
    user: UserId,
) -> Result<Vec<(Role, ProviderId, String)>> {
    sqlx::query("SELECT role, provider_id, model FROM role_assignments WHERE user_id = ?")
        .bind(user.to_string())
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(|r| {
            Ok((
                codec::from_tag(r, "role")?,
                codec::id(r, "provider_id")?,
                r.try_get("model")?,
            ))
        })
        .collect()
}

pub async fn role(
    conn: &mut SqliteConnection,
    user: UserId,
    role: Role,
) -> Result<Option<(ProviderId, String)>> {
    sqlx::query("SELECT provider_id, model FROM role_assignments WHERE user_id = ? AND role = ?")
        .bind(user.to_string())
        .bind(codec::tag(&role)?)
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| Ok((codec::id(&r, "provider_id")?, r.try_get("model")?)))
        .transpose()
}

pub async fn set_role(
    conn: &mut SqliteConnection,
    user: UserId,
    role: Role,
    provider: ProviderId,
    model: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO role_assignments (user_id, role, provider_id, model) VALUES (?, ?, ?, ?) \
         ON CONFLICT (user_id, role) DO UPDATE SET provider_id = excluded.provider_id, \
         model = excluded.model",
    )
    .bind(user.to_string())
    .bind(codec::tag(&role)?)
    .bind(provider.to_string())
    .bind(model)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn clear_role(conn: &mut SqliteConnection, user: UserId, role: Role) -> Result<()> {
    sqlx::query("DELETE FROM role_assignments WHERE user_id = ? AND role = ?")
        .bind(user.to_string())
        .bind(codec::tag(&role)?)
        .execute(conn)
        .await?;
    Ok(())
}
