//! Literal lexical search. No query text is interpreted as FTS syntax; no private notes indexed.
use crate::{Result, codec};
use sqlx::{Row, SqliteConnection};
use sunset_domain::{ConceptSummary, UserId};

pub async fn refresh(conn: &mut SqliteConnection) -> Result<()> {
    sqlx::query("DELETE FROM knowledge_fts")
        .execute(&mut *conn)
        .await?;
    sqlx::query("INSERT INTO knowledge_fts SELECT id, user_id, text FROM searchable_knowledge")
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub async fn search(
    conn: &mut SqliteConnection,
    user: UserId,
    query: &str,
    limit: u32,
) -> Result<Vec<ConceptSummary>> {
    let terms: Vec<String> = query
        .chars()
        .take(512)
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .take(24)
        .map(str::to_lowercase)
        .collect();
    if terms.is_empty() || limit == 0 {
        return Ok(vec![]);
    }
    let mut ids = std::collections::BTreeMap::<String, usize>::new();
    for term in terms {
        let rows: Vec<String> = if term.chars().count() >= 3 {
            sqlx::query_scalar("SELECT concept_id FROM knowledge_fts WHERE knowledge_fts MATCH ? AND user_id = ? ORDER BY rank LIMIT 200")
                .bind(format!("\"{}\"", term.replace('"', "\"\""))).bind(user.to_string()).fetch_all(&mut *conn).await?
        } else {
            // FTS trigrams cannot match one/two-character words, including short CJK terms.
            sqlx::query_scalar("SELECT id FROM searchable_knowledge WHERE user_id = ? AND instr(lower(text), ?) > 0 ORDER BY id LIMIT 200")
                .bind(user.to_string()).bind(&term).fetch_all(&mut *conn).await?
        };
        for id in rows {
            *ids.entry(id).or_default() += term.chars().count().min(12);
        }
    }
    let mut result = vec![];
    for (id, score) in ids {
        if let Some(row) = sqlx::query("SELECT id, slug, title FROM concepts WHERE id = ? AND user_id = ? AND retracted_by_commit_id IS NULL")
            .bind(id).bind(user.to_string()).fetch_optional(&mut *conn).await? {
            result.push((score, ConceptSummary { id: codec::id(&row, "id")?, slug: row.try_get("slug")?, title: row.try_get("title")? }));
        }
    }
    result.sort_by(|(sa, a), (sb, b)| {
        sb.cmp(sa)
            .then(a.title.cmp(&b.title))
            .then(a.id.to_string().cmp(&b.id.to_string()))
    });
    result.truncate(limit.min(100) as usize);
    Ok(result.into_iter().map(|(_, c)| c).collect())
}
