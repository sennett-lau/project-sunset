//! Conversions between domain types and SQLite column values.

use std::str::FromStr;

use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::Result;

/// Read a TEXT column holding a typed id.
pub fn id<T: FromStr<Err = uuid::Error>>(row: &SqliteRow, col: &str) -> Result<T> {
    let s: String = row.try_get(col)?;
    Ok(s.parse()?)
}

pub fn opt_id<T: FromStr<Err = uuid::Error>>(row: &SqliteRow, col: &str) -> Result<Option<T>> {
    let s: Option<String> = row.try_get(col)?;
    Ok(s.map(|s| s.parse()).transpose()?)
}

/// Read a TEXT column holding JSON.
pub fn json<T: DeserializeOwned>(row: &SqliteRow, col: &str) -> Result<T> {
    let s: String = row.try_get(col)?;
    Ok(serde_json::from_str(&s)?)
}

pub fn to_json<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

/// A unit enum as its serde string, e.g. `RelationKind::IsA` → `IS_A`.
pub fn tag<T: Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(s) => Ok(s),
        other => Ok(other.to_string()),
    }
}

/// Inverse of [`tag`].
pub fn from_tag<T: DeserializeOwned>(row: &SqliteRow, col: &str) -> Result<T> {
    let s: String = row.try_get(col)?;
    Ok(serde_json::from_value(serde_json::Value::String(s))?)
}
