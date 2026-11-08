//! Concepts, article revisions, relations, learning observations and visual artifacts.
//!
//! Reads return live rows only (not retracted by an undo). Writes happen inside a commit.

use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use sunset_domain::{
    ArticleRevision, ArtifactId, CommitId, Concept, ConceptId, ConceptSummary, LearningObservation,
    ObservationId, ObservationKind, ObservationSource, Relation, RelationId, RelationKind,
    RevisionId, TurnId, UserId, VisualArtifact, VisualPayload,
};

use crate::{Result, codec};

// ---- concepts ---------------------------------------------------------------------------------

pub async fn insert_concept(
    conn: &mut SqliteConnection,
    user: UserId,
    commit: CommitId,
    slug: &str,
    title: &str,
    aliases: &[String],
) -> Result<Concept> {
    let concept = Concept {
        id: ConceptId::new(),
        slug: slug.to_owned(),
        title: title.to_owned(),
        aliases: aliases.to_vec(),
        created_at: Utc::now(),
    };
    sqlx::query(
        "INSERT INTO concepts (id, user_id, slug, title, aliases_json, created_at, \
         created_by_commit_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(concept.id.to_string())
    .bind(user.to_string())
    .bind(&concept.slug)
    .bind(&concept.title)
    .bind(codec::to_json(&concept.aliases)?)
    .bind(concept.created_at)
    .bind(commit.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(concept)
}

/// Record an alias learned after the concept was created.
pub async fn insert_alias(
    conn: &mut SqliteConnection,
    commit: CommitId,
    concept_id: ConceptId,
    alias: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO concept_aliases (id, concept_id, alias, created_by_commit_id) VALUES (?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::now_v7().to_string())
    .bind(concept_id.to_string())
    .bind(alias)
    .bind(commit.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Every live concept with all of its aliases. A personal library is small enough to match
/// names in memory.
pub async fn live_concepts(conn: &mut SqliteConnection, user: UserId) -> Result<Vec<Concept>> {
    let rows = sqlx::query(
        "SELECT * FROM concepts WHERE user_id = ? AND retracted_by_commit_id IS NULL \
         ORDER BY title COLLATE NOCASE",
    )
    .bind(user.to_string())
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(with_aliases(conn, concept_from_row(row)?).await?);
    }
    Ok(out)
}

async fn with_aliases(conn: &mut SqliteConnection, mut concept: Concept) -> Result<Concept> {
    let extra: Vec<String> = sqlx::query_scalar(
        "SELECT alias FROM concept_aliases WHERE concept_id = ? \
         AND retracted_by_commit_id IS NULL ORDER BY id",
    )
    .bind(concept.id.to_string())
    .fetch_all(&mut *conn)
    .await?;
    for alias in extra {
        if !concept.aliases.contains(&alias) {
            concept.aliases.push(alias);
        }
    }
    Ok(concept)
}

pub async fn concept_by_slug(
    conn: &mut SqliteConnection,
    user: UserId,
    slug: &str,
) -> Result<Option<Concept>> {
    let row = sqlx::query(
        "SELECT * FROM concepts WHERE user_id = ? AND slug = ? AND retracted_by_commit_id IS NULL",
    )
    .bind(user.to_string())
    .bind(slug)
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some(r) => Ok(Some(with_aliases(conn, concept_from_row(&r)?).await?)),
        None => Ok(None),
    }
}

pub async fn concept(conn: &mut SqliteConnection, id: ConceptId) -> Result<Option<Concept>> {
    let row = sqlx::query("SELECT * FROM concepts WHERE id = ? AND retracted_by_commit_id IS NULL")
        .bind(id.to_string())
        .fetch_optional(&mut *conn)
        .await?;
    match row {
        Some(r) => Ok(Some(with_aliases(conn, concept_from_row(&r)?).await?)),
        None => Ok(None),
    }
}

/// Live concepts, by title.
pub async fn list_concepts(
    conn: &mut SqliteConnection,
    user: UserId,
) -> Result<Vec<ConceptSummary>> {
    sqlx::query(
        "SELECT id, slug, title FROM concepts WHERE user_id = ? AND retracted_by_commit_id IS NULL \
         ORDER BY title COLLATE NOCASE",
    )
    .bind(user.to_string())
    .fetch_all(&mut *conn)
    .await?
    .iter()
    .map(summary_from_row)
    .collect()
}

fn concept_from_row(row: &SqliteRow) -> Result<Concept> {
    Ok(Concept {
        id: codec::id(row, "id")?,
        slug: row.try_get("slug")?,
        title: row.try_get("title")?,
        aliases: codec::json(row, "aliases_json")?,
        created_at: row.try_get("created_at")?,
    })
}

fn summary_from_row(row: &SqliteRow) -> Result<ConceptSummary> {
    Ok(ConceptSummary {
        id: codec::id(row, "id")?,
        slug: row.try_get("slug")?,
        title: row.try_get("title")?,
    })
}

// ---- article revisions ------------------------------------------------------------------------

/// Add the next revision. Versions count every revision ever made, including undone ones.
pub async fn insert_revision(
    conn: &mut SqliteConnection,
    commit: CommitId,
    concept_id: ConceptId,
    summary: &str,
    body_md: &str,
) -> Result<ArticleRevision> {
    let version: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM article_revisions WHERE concept_id = ?",
    )
    .bind(concept_id.to_string())
    .fetch_one(&mut *conn)
    .await?;
    let rev = ArticleRevision {
        id: RevisionId::new(),
        concept_id,
        version: u32::try_from(version).unwrap_or(u32::MAX),
        summary: summary.to_owned(),
        body_md: body_md.to_owned(),
        commit_id: commit,
        created_at: Utc::now(),
    };
    sqlx::query(
        "INSERT INTO article_revisions (id, concept_id, version, summary, body_md, commit_id, \
         created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(rev.id.to_string())
    .bind(rev.concept_id.to_string())
    .bind(version)
    .bind(&rev.summary)
    .bind(&rev.body_md)
    .bind(rev.commit_id.to_string())
    .bind(rev.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(rev)
}

/// The newest live revision.
pub async fn current_revision(
    conn: &mut SqliteConnection,
    concept_id: ConceptId,
) -> Result<Option<ArticleRevision>> {
    let row = sqlx::query(
        "SELECT * FROM article_revisions WHERE concept_id = ? AND retracted_by_commit_id IS NULL \
         ORDER BY version DESC LIMIT 1",
    )
    .bind(concept_id.to_string())
    .fetch_optional(&mut *conn)
    .await?;
    row.map(|row| -> Result<ArticleRevision> {
        let version: i64 = row.try_get("version")?;
        Ok(ArticleRevision {
            id: codec::id(&row, "id")?,
            concept_id: codec::id(&row, "concept_id")?,
            version: u32::try_from(version).unwrap_or(u32::MAX),
            summary: row.try_get("summary")?,
            body_md: row.try_get("body_md")?,
            commit_id: codec::id(&row, "commit_id")?,
            created_at: row.try_get("created_at")?,
        })
    })
    .transpose()
}

// ---- relations --------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn insert_relation(
    conn: &mut SqliteConnection,
    user: UserId,
    commit: CommitId,
    from: ConceptId,
    to: ConceptId,
    kind: RelationKind,
    label: Option<&str>,
    evidence: &[TurnId],
) -> Result<Relation> {
    let rel = Relation {
        id: RelationId::new(),
        from,
        to,
        kind,
        label: label.map(str::to_owned),
        evidence_turn_ids: evidence.to_vec(),
    };
    sqlx::query(
        "INSERT INTO relations (id, user_id, from_concept_id, kind, to_concept_id, label, \
         evidence_json, created_by_commit_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(rel.id.to_string())
    .bind(user.to_string())
    .bind(rel.from.to_string())
    .bind(codec::tag(&rel.kind)?)
    .bind(rel.to.to_string())
    .bind(rel.label.as_deref())
    .bind(codec::to_json(&rel.evidence_turn_ids)?)
    .bind(commit.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(rel)
}

pub async fn relation_exists(
    conn: &mut SqliteConnection,
    from: ConceptId,
    to: ConceptId,
    kind: RelationKind,
) -> Result<bool> {
    let row = sqlx::query(
        "SELECT 1 FROM relations WHERE from_concept_id = ? AND to_concept_id = ? AND kind = ? \
         AND retracted_by_commit_id IS NULL",
    )
    .bind(from.to_string())
    .bind(to.to_string())
    .bind(codec::tag(&kind)?)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.is_some())
}

/// Live relations whose two ends are both live concepts. A macro so the SQL stays a literal.
macro_rules! live_relations {
    ($filter:literal) => {
        concat!(
            "SELECT r.* FROM relations r ",
            "JOIN concepts a ON a.id = r.from_concept_id AND a.retracted_by_commit_id IS NULL ",
            "JOIN concepts b ON b.id = r.to_concept_id AND b.retracted_by_commit_id IS NULL ",
            "WHERE r.retracted_by_commit_id IS NULL AND ",
            $filter,
            " ORDER BY r.id"
        )
    };
}

pub async fn relations_from(conn: &mut SqliteConnection, id: ConceptId) -> Result<Vec<Relation>> {
    sqlx::query(live_relations!("r.from_concept_id = ?"))
        .bind(id.to_string())
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(relation_from_row)
        .collect()
}

pub async fn relations_to(conn: &mut SqliteConnection, id: ConceptId) -> Result<Vec<Relation>> {
    sqlx::query(live_relations!("r.to_concept_id = ?"))
        .bind(id.to_string())
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(relation_from_row)
        .collect()
}

pub async fn all_relations(conn: &mut SqliteConnection, user: UserId) -> Result<Vec<Relation>> {
    sqlx::query(live_relations!("r.user_id = ?"))
        .bind(user.to_string())
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(relation_from_row)
        .collect()
}

fn relation_from_row(row: &SqliteRow) -> Result<Relation> {
    Ok(Relation {
        id: codec::id(row, "id")?,
        from: codec::id(row, "from_concept_id")?,
        to: codec::id(row, "to_concept_id")?,
        kind: codec::from_tag(row, "kind")?,
        label: row.try_get("label")?,
        evidence_turn_ids: codec::json(row, "evidence_json")?,
    })
}

// ---- learning observations (private overlay) --------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn insert_observation(
    conn: &mut SqliteConnection,
    user: UserId,
    commit: CommitId,
    concept_id: ConceptId,
    kind: ObservationKind,
    source: ObservationSource,
    note: &str,
    evidence: &[TurnId],
) -> Result<LearningObservation> {
    let obs = LearningObservation {
        id: ObservationId::new(),
        concept_id,
        kind,
        source,
        note: note.to_owned(),
        evidence_turn_ids: evidence.to_vec(),
        created_at: Utc::now(),
    };
    sqlx::query(
        "INSERT INTO learning_observations (id, user_id, concept_id, kind, source, note, \
         evidence_json, created_at, created_by_commit_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(obs.id.to_string())
    .bind(user.to_string())
    .bind(obs.concept_id.to_string())
    .bind(codec::tag(&obs.kind)?)
    .bind(codec::tag(&obs.source)?)
    .bind(&obs.note)
    .bind(codec::to_json(&obs.evidence_turn_ids)?)
    .bind(obs.created_at)
    .bind(commit.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(obs)
}

pub async fn observations(
    conn: &mut SqliteConnection,
    concept_id: ConceptId,
) -> Result<Vec<LearningObservation>> {
    sqlx::query(
        "SELECT * FROM learning_observations WHERE concept_id = ? \
         AND retracted_by_commit_id IS NULL ORDER BY created_at, id",
    )
    .bind(concept_id.to_string())
    .fetch_all(&mut *conn)
    .await?
    .iter()
    .map(|row| {
        Ok(LearningObservation {
            id: codec::id(row, "id")?,
            concept_id: codec::id(row, "concept_id")?,
            kind: codec::from_tag(row, "kind")?,
            source: codec::from_tag(row, "source")?,
            note: row.try_get("note")?,
            evidence_turn_ids: codec::json(row, "evidence_json")?,
            created_at: row.try_get("created_at")?,
        })
    })
    .collect()
}

// ---- visual artifacts -------------------------------------------------------------------------

pub const RENDERER_VERSION: u32 = 1;

pub async fn insert_visual(
    conn: &mut SqliteConnection,
    commit: CommitId,
    concept_id: ConceptId,
    revision_id: Option<RevisionId>,
    title: &str,
    payload: &VisualPayload,
) -> Result<VisualArtifact> {
    let visual = VisualArtifact {
        id: ArtifactId::new(),
        concept_id,
        revision_id,
        title: title.to_owned(),
        payload: payload.clone(),
        renderer_version: RENDERER_VERSION,
        created_at: Utc::now(),
    };
    let payload_json = serde_json::to_value(&visual.payload)?;
    let kind = payload_json["kind"].as_str().unwrap_or_default().to_owned();
    sqlx::query(
        "INSERT INTO visual_artifacts (id, concept_id, revision_id, title, kind, payload_json, \
         renderer_version, created_at, created_by_commit_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(visual.id.to_string())
    .bind(visual.concept_id.to_string())
    .bind(visual.revision_id.map(|r| r.to_string()))
    .bind(&visual.title)
    .bind(kind)
    .bind(payload_json.to_string())
    .bind(visual.renderer_version)
    .bind(visual.created_at)
    .bind(commit.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(visual)
}

pub async fn visuals(
    conn: &mut SqliteConnection,
    concept_id: ConceptId,
) -> Result<Vec<VisualArtifact>> {
    sqlx::query(
        "SELECT * FROM visual_artifacts WHERE concept_id = ? AND retracted_by_commit_id IS NULL \
         ORDER BY created_at, id",
    )
    .bind(concept_id.to_string())
    .fetch_all(&mut *conn)
    .await?
    .iter()
    .map(|row| {
        let version: i64 = row.try_get("renderer_version")?;
        Ok(VisualArtifact {
            id: codec::id(row, "id")?,
            concept_id: codec::id(row, "concept_id")?,
            revision_id: codec::opt_id(row, "revision_id")?,
            title: row.try_get("title")?,
            payload: codec::json(row, "payload_json")?,
            renderer_version: u32::try_from(version).unwrap_or(u32::MAX),
            created_at: row.try_get("created_at")?,
        })
    })
    .collect()
}

// ---- undo -------------------------------------------------------------------------------------

/// Hide every knowledge row made by `commit`, marking it with the undo commit. Returns how many
/// rows were hidden. Nothing is deleted.
pub async fn retract_commit(
    conn: &mut SqliteConnection,
    commit: CommitId,
    undo: CommitId,
) -> Result<u64> {
    const RETRACT: [&str; 6] = [
        "UPDATE concepts SET retracted_by_commit_id = ? \
         WHERE created_by_commit_id = ? AND retracted_by_commit_id IS NULL",
        "UPDATE article_revisions SET retracted_by_commit_id = ? \
         WHERE commit_id = ? AND retracted_by_commit_id IS NULL",
        "UPDATE relations SET retracted_by_commit_id = ? \
         WHERE created_by_commit_id = ? AND retracted_by_commit_id IS NULL",
        "UPDATE learning_observations SET retracted_by_commit_id = ? \
         WHERE created_by_commit_id = ? AND retracted_by_commit_id IS NULL",
        "UPDATE visual_artifacts SET retracted_by_commit_id = ? \
         WHERE created_by_commit_id = ? AND retracted_by_commit_id IS NULL",
        "UPDATE concept_aliases SET retracted_by_commit_id = ? \
         WHERE created_by_commit_id = ? AND retracted_by_commit_id IS NULL",
    ];
    let mut hidden = 0;
    for sql in RETRACT {
        hidden += sqlx::query(sql)
            .bind(undo.to_string())
            .bind(commit.to_string())
            .execute(&mut *conn)
            .await?
            .rows_affected();
    }
    Ok(hidden)
}
