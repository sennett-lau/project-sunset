//! Commit, reject and undo. Every change goes through a commit; nothing is deleted.

use std::collections::HashMap;

use sunset_domain::{
    AppEvent, ConceptId, Proposal, ProposalEdit, ProposalId, ProposalStatus, ProposedChange,
    RevisionId, slugify,
};
use sunset_store::Tx;
use sunset_store::repo::{commits, knowledge as kn, proposals};

use crate::{CoreError, Result, Sunset};

pub async fn approve(app: &Sunset, id: ProposalId, expected_revision: u32) -> Result<Proposal> {
    let _guard = app.inner.knowledge_lock.lock().await;
    let inner = &app.inner;
    let mut tx = inner.store.begin().await?;
    let proposal = load(&mut tx, id).await?;
    check_pending_revision(&proposal, expected_revision)?;
    check_article_base(&mut tx, &proposal).await?;
    let commit = commits::insert(&mut tx, inner.user, id, None).await?;
    if !proposals::transition(
        &mut tx,
        id,
        ProposalStatus::AwaitingApproval,
        ProposalStatus::Committed,
        Some(commit.id),
    )
    .await?
    {
        return Err(conflict(&proposal, "approved"));
    }
    apply_changes(app, &mut tx, &proposal, commit.id).await?;
    let updated = load(&mut tx, id).await?;
    sunset_store::search::refresh(&mut tx).await?;
    tx.commit().await?;
    app.sync_vault_after_commit().await;
    inner.bus.emit(AppEvent::ProposalUpdated {
        proposal: updated.clone(),
    });
    inner.bus.emit(AppEvent::KnowledgeChanged {
        commit_id: commit.id,
    });
    Ok(updated)
}

pub async fn reject(app: &Sunset, id: ProposalId, expected_revision: u32) -> Result<Proposal> {
    let _guard = app.inner.knowledge_lock.lock().await;
    let inner = &app.inner;
    let mut tx = inner.store.begin().await?;
    let proposal = load(&mut tx, id).await?;
    check_pending_revision(&proposal, expected_revision)?;
    if !proposals::transition(
        &mut tx,
        id,
        ProposalStatus::AwaitingApproval,
        ProposalStatus::Rejected,
        None,
    )
    .await?
    {
        return Err(conflict(&proposal, "rejected"));
    }
    let updated = load(&mut tx, id).await?;
    tx.commit().await?;
    inner.bus.emit(AppEvent::ProposalUpdated {
        proposal: updated.clone(),
    });
    Ok(updated)
}

pub async fn undo(app: &Sunset, id: ProposalId) -> Result<Proposal> {
    let _guard = app.inner.knowledge_lock.lock().await;
    let inner = &app.inner;
    let mut tx = inner.store.begin().await?;
    let proposal = load(&mut tx, id).await?;
    let Some(original) = proposal.commit_id else {
        return Err(conflict(&proposal, "undone"));
    };
    let undo = commits::insert(&mut tx, inner.user, id, Some(original)).await?;
    if !proposals::transition(
        &mut tx,
        id,
        ProposalStatus::Committed,
        ProposalStatus::Undone,
        None,
    )
    .await?
    {
        return Err(conflict(&proposal, "undone"));
    }
    kn::retract_commit(&mut tx, original, undo.id).await?;
    let updated = load(&mut tx, id).await?;
    sunset_store::search::refresh(&mut tx).await?;
    tx.commit().await?;
    app.sync_vault_after_commit().await;
    inner.bus.emit(AppEvent::ProposalUpdated {
        proposal: updated.clone(),
    });
    inner
        .bus
        .emit(AppEvent::KnowledgeChanged { commit_id: undo.id });
    Ok(updated)
}

/// Commit a just-created proposal without asking (auto-save mode).
pub async fn auto_commit(app: &Sunset, id: ProposalId) -> Result<Proposal> {
    approve(app, id, 0).await
}

pub async fn edit(app: &Sunset, id: ProposalId, edit: ProposalEdit) -> Result<Proposal> {
    let _guard = app.inner.knowledge_lock.lock().await;
    let inner = &app.inner;
    let mut tx = inner.store.begin().await?;
    let original = load(&mut tx, id).await?;
    check_pending_revision(&original, edit.expected_revision)?;
    check_article_base(&mut tx, &original).await?;
    if let Some(base) = &original.article_base {
        let concept = kn::concept(&mut tx, base.concept_id)
            .await?
            .ok_or(CoreError::NotFound("concept"))?;
        check_article_change(&edit.changes, &concept.slug)?;
    }
    if original.revision == u32::MAX {
        return Err(CoreError::Conflict(
            "proposal has too many revisions".into(),
        ));
    }
    let draft = super::curator::ProposalDraft {
        summary: edit.summary,
        changes: edit.changes,
    };
    let names = super::validate::NameIndex::new(&kn::live_concepts(&mut tx, inner.user).await?);
    // Validate exactly what the learner reviewed; do not silently normalize edited targets.
    super::validate::draft(&draft, &names).map_err(CoreError::Invalid)?;
    if !proposals::edit(
        &mut tx,
        id,
        edit.expected_revision,
        &draft.summary,
        &draft.changes,
    )
    .await?
    {
        return Err(CoreError::Conflict(
            "proposal changed; reload before editing".into(),
        ));
    }
    let updated = load(&mut tx, id).await?;
    tx.commit().await?;
    inner.bus.emit(AppEvent::ProposalUpdated {
        proposal: updated.clone(),
    });
    Ok(updated)
}

fn check_pending_revision(p: &Proposal, expected: u32) -> Result<()> {
    if p.status != ProposalStatus::AwaitingApproval {
        return Err(conflict(p, "changed"));
    }
    if p.revision != expected {
        return Err(CoreError::Conflict(
            "proposal changed since you opened it; review the latest version".into(),
        ));
    }
    Ok(())
}

async fn load(tx: &mut Tx, id: ProposalId) -> Result<Proposal> {
    proposals::get(tx, id)
        .await?
        .ok_or(CoreError::NotFound("proposal"))
}

fn conflict(p: &Proposal, action: &str) -> CoreError {
    CoreError::Conflict(format!("proposal is {:?} and cannot be {action}", p.status))
}

/// Write each change. Concept slugs resolve to concepts created earlier in this proposal or
/// already live in the library. If a referenced concept has since been undone, the whole
/// approval fails and nothing is written.
async fn apply_changes(
    app: &Sunset,
    tx: &mut Tx,
    proposal: &Proposal,
    commit: sunset_domain::CommitId,
) -> Result<()> {
    let user = app.inner.user;
    let mut ids: HashMap<String, ConceptId> = HashMap::new();
    let mut revisions: HashMap<ConceptId, RevisionId> = HashMap::new();

    for change in &proposal.changes {
        match change {
            ProposedChange::UpsertConcept {
                slug,
                title,
                aliases,
            } => {
                // An existing concept is reused; names it does not have yet become aliases
                // (undoable with the rest of this commit).
                let id = match kn::concept_by_slug(tx, user, slug).await? {
                    Some(c) => {
                        let mut known: Vec<String> = std::iter::once(&c.slug)
                            .chain(std::iter::once(&c.title))
                            .chain(&c.aliases)
                            .map(|n| slugify(n))
                            .collect();
                        for name in std::iter::once(title).chain(aliases) {
                            let key = slugify(name);
                            if !key.is_empty() && !known.contains(&key) {
                                kn::insert_alias(tx, commit, c.id, name.trim()).await?;
                                known.push(key);
                            }
                        }
                        c.id
                    }
                    None => {
                        kn::insert_concept(tx, user, commit, slug, title, aliases)
                            .await?
                            .id
                    }
                };
                ids.insert(slug.clone(), id);
            }
            ProposedChange::ReviseArticle {
                concept,
                summary,
                body_md,
            } => {
                let cid = resolve(tx, app, &mut ids, concept).await?;
                let rev = kn::insert_revision(tx, commit, cid, summary, body_md).await?;
                revisions.insert(cid, rev.id);
            }
            ProposedChange::AddRelation {
                from,
                to,
                kind,
                label,
            } => {
                let from = resolve(tx, app, &mut ids, from).await?;
                let to = resolve(tx, app, &mut ids, to).await?;
                if !kn::relation_exists(tx, from, to, *kind).await? {
                    kn::insert_relation(
                        tx,
                        user,
                        commit,
                        from,
                        to,
                        *kind,
                        label.as_deref(),
                        &proposal.evidence_turn_ids,
                    )
                    .await?;
                }
            }
            ProposedChange::AddObservation {
                concept,
                kind,
                source,
                note,
            } => {
                let cid = resolve(tx, app, &mut ids, concept).await?;
                kn::insert_observation(
                    tx,
                    user,
                    commit,
                    cid,
                    *kind,
                    *source,
                    note,
                    &proposal.evidence_turn_ids,
                )
                .await?;
            }
            ProposedChange::AddVisual {
                concept,
                title,
                payload,
            } => {
                let cid = resolve(tx, app, &mut ids, concept).await?;
                let revision = match revisions.get(&cid) {
                    Some(r) => Some(*r),
                    None => kn::current_revision(tx, cid).await?.map(|r| r.id),
                };
                kn::insert_visual(tx, commit, cid, revision, title, payload).await?;
            }
        }
    }
    Ok(())
}

async fn resolve(
    tx: &mut Tx,
    app: &Sunset,
    ids: &mut HashMap<String, ConceptId>,
    slug: &str,
) -> Result<ConceptId> {
    if let Some(id) = ids.get(slug) {
        return Ok(*id);
    }
    let concept = kn::concept_by_slug(tx, app.inner.user, slug)
        .await?
        .ok_or_else(|| {
            CoreError::Conflict(format!(
                "this proposal refers to concept {slug:?}, which no longer exists"
            ))
        })?;
    ids.insert(slug.to_owned(), concept.id);
    Ok(concept.id)
}

fn check_article_change(changes: &[ProposedChange], slug: &str) -> Result<()> {
    if matches!(changes, [ProposedChange::ReviseArticle { concept, .. }] if concept == slug) {
        Ok(())
    } else {
        Err(CoreError::Invalid(
            "an article edit must revise only its original article".into(),
        ))
    }
}

async fn check_article_base(tx: &mut Tx, proposal: &Proposal) -> Result<()> {
    if let Some(base) = &proposal.article_base {
        let current = kn::current_revision(tx, base.concept_id).await?;
        if current.as_ref().map(|r| r.id) != Some(base.revision_id) {
            return Err(CoreError::Conflict("article changed since this edit was started; reopen the current article and propose a new edit".into()));
        }
        let concept = kn::concept(tx, base.concept_id)
            .await?
            .ok_or(CoreError::NotFound("concept"))?;
        check_article_change(&proposal.changes, &concept.slug)?;
    }
    Ok(())
}

pub async fn propose_article_edit(
    app: &Sunset,
    concept_id: ConceptId,
    expected_revision_id: RevisionId,
    summary: String,
    body_md: String,
) -> Result<Proposal> {
    let _guard = app.inner.knowledge_lock.lock().await;
    let mut tx = app.inner.store.begin().await?;
    let concept = kn::concept(&mut tx, concept_id)
        .await?
        .ok_or(CoreError::NotFound("concept"))?;
    let article = kn::current_revision(&mut tx, concept_id)
        .await?
        .ok_or(CoreError::NotFound("article"))?;
    if article.id != expected_revision_id {
        return Err(CoreError::Conflict("article changed since you opened the editor; reopen the current article before editing".into()));
    }
    let source = proposals::for_commit(&mut tx, article.commit_id)
        .await?
        .ok_or(CoreError::NotFound("source proposal"))?;
    let draft = super::curator::ProposalDraft {
        summary: format!("Edit article: {}", concept.title),
        changes: vec![ProposedChange::ReviseArticle {
            concept: concept.slug,
            summary,
            body_md,
        }],
    };
    let names = super::validate::NameIndex::new(&kn::live_concepts(&mut tx, app.inner.user).await?);
    super::validate::draft(&draft, &names).map_err(CoreError::Invalid)?;
    let proposal = Proposal {
        id: ProposalId::new(),
        session_id: source.session_id,
        evidence_turn_ids: vec![],
        summary: draft.summary,
        changes: draft.changes,
        revision: 0,
        article_base: Some(sunset_domain::ArticleEditBase {
            concept_id,
            revision_id: article.id,
        }),
        status: ProposalStatus::AwaitingApproval,
        commit_id: None,
        created_at: chrono::Utc::now(),
    };
    proposals::insert(&mut tx, app.inner.user, &proposal).await?;
    tx.commit().await?;
    app.inner.bus.emit(AppEvent::ProposalCreated {
        proposal: proposal.clone(),
    });
    Ok(proposal)
}
