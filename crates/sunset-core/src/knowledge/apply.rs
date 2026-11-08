//! Commit, reject and undo. Every change goes through a commit; nothing is deleted.

use std::collections::HashMap;

use sunset_domain::{
    AppEvent, ConceptId, Proposal, ProposalId, ProposalStatus, ProposedChange, RevisionId, slugify,
};
use sunset_store::Tx;
use sunset_store::repo::{commits, knowledge as kn, proposals};

use crate::{CoreError, Result, Sunset};

pub async fn approve(app: &Sunset, id: ProposalId) -> Result<Proposal> {
    let _guard = app.inner.knowledge_lock.lock().await;
    let inner = &app.inner;
    let mut tx = inner.store.begin().await?;
    let proposal = load(&mut tx, id).await?;
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

pub async fn reject(app: &Sunset, id: ProposalId) -> Result<Proposal> {
    let _guard = app.inner.knowledge_lock.lock().await;
    let inner = &app.inner;
    let mut tx = inner.store.begin().await?;
    let proposal = load(&mut tx, id).await?;
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
    approve(app, id).await
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
