//! Phase 0 exit test (docs/architecture.md §8): "What is a VAE?" → streamed answer → one mock
//! proposal (concept + article, relations, personal observation, concept_flow visual) →
//! approve / reject / undo changes the wiki.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sunset_core::domain::{
    AppEvent, ObservationKind, Proposal, ProposalStatus, ProposedChange, RelationKind, SaveMode,
    Speaker, VisualPayload,
};
use sunset_core::{AppIdentity, MemorySecretStore, Sunset, SunsetConfig};
use tokio::sync::broadcast::Receiver;

async fn open(dir: &Path) -> Sunset {
    Sunset::open(SunsetConfig {
        identity: AppIdentity::new("Test", "dev.example.test").unwrap(),
        data_dir: dir.to_path_buf(),
        secrets: Arc::new(MemorySecretStore::default()),
        demo_chunk_delay: Duration::ZERO,
    })
    .await
    .unwrap()
}

/// Wait for the next event matching `f`, failing after a few seconds.
async fn next<T>(rx: &mut Receiver<AppEvent>, mut f: impl FnMut(AppEvent) -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(t) = f(rx.recv().await.unwrap()) {
                return t;
            }
        }
    })
    .await
    .expect("timed out waiting for event")
}

async fn ask(app: &Sunset, rx: &mut Receiver<AppEvent>, question: &str) -> Proposal {
    app.send_message(None, question).await.unwrap();
    next(rx, |e| match e {
        AppEvent::ProposalCreated { proposal } => Some(proposal),
        _ => None,
    })
    .await
}

#[tokio::test]
async fn ask_stream_propose_approve_undo() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(dir.path()).await;
    let mut rx = app.subscribe();

    let sent = app.send_message(None, "What is a VAE?").await.unwrap();
    assert_eq!(sent.user_turn.speaker, Speaker::User);

    // The answer streams in pieces, then the full tutor turn is stored.
    let mut streamed = String::new();
    let tutor_turn = next(&mut rx, |e| match e {
        AppEvent::TutorDelta { turn_id, text, .. } if turn_id == sent.tutor_turn_id => {
            streamed.push_str(&text);
            None
        }
        AppEvent::TurnFinalized { turn } if turn.speaker == Speaker::Tutor => Some(turn),
        _ => None,
    })
    .await;
    assert!(streamed.contains("variational autoencoder"));
    assert_eq!(tutor_turn.text, streamed);
    assert_eq!(tutor_turn.id, sent.tutor_turn_id);

    // Curation runs afterwards and waits for approval.
    let proposal = next(&mut rx, |e| match e {
        AppEvent::ProposalCreated { proposal } => Some(proposal),
        _ => None,
    })
    .await;
    assert_eq!(proposal.status, ProposalStatus::AwaitingApproval);
    assert_eq!(
        proposal.evidence_turn_ids,
        [sent.user_turn.id, tutor_turn.id]
    );
    assert!(proposal.changes.iter().any(|c| matches!(
        c,
        ProposedChange::AddVisual {
            payload: VisualPayload::ConceptFlow { .. },
            ..
        }
    )));
    assert_eq!(app.pending_proposals().await.unwrap().len(), 1);
    assert!(
        app.concepts().await.unwrap().is_empty(),
        "nothing saved before approval"
    );

    assert!(app.search_knowledge("VAE", 10).await.unwrap().is_empty());

    // Approve: the wiki and graph change.
    let approved = app
        .approve_proposal(proposal.id, proposal.revision)
        .await
        .unwrap();
    assert_eq!(approved.status, ProposalStatus::Committed);
    assert!(app.pending_proposals().await.unwrap().is_empty());
    let concepts = app.concepts().await.unwrap();
    assert_eq!(concepts.len(), 3);
    let vae = concepts
        .iter()
        .find(|c| c.slug == "variational-autoencoder")
        .unwrap();

    let page = app.concept_page(vae.id).await.unwrap();
    assert_eq!(page.concept.aliases, ["VAE"]);
    assert_eq!(page.article.as_ref().unwrap().version, 1);
    let kinds: Vec<RelationKind> = page.outgoing.iter().map(|r| r.relation.kind).collect();
    assert_eq!(kinds, [RelationKind::IsA, RelationKind::Uses]);
    assert!(!app.search_knowledge("VAE", 10).await.unwrap().is_empty());
    let exported = app.export_public().await.unwrap();
    assert!(
        exported
            .iter()
            .all(|f| !f.markdown.contains("What is a VAE?"))
    );
    let vault_files: Vec<_> = exported
        .iter()
        .map(|f| app.data_dir().join("vault").join(&f.filename))
        .collect();
    assert!(vault_files.iter().all(|p| p.exists()));
    assert!(app.vault_status().error.is_none());
    assert_eq!(page.overlay.len(), 1);
    assert_eq!(page.overlay[0].kind, ObservationKind::Question);
    assert!(page.overlay[0].note.contains("What is a VAE?"));
    assert!(
        page.overlay[0]
            .evidence_turn_ids
            .contains(&sent.user_turn.id)
    );
    assert_eq!(page.visuals.len(), 1);
    assert_eq!(
        page.visuals[0].revision_id,
        page.article.as_ref().map(|a| a.id)
    );

    let graph = app.graph().await.unwrap();
    assert_eq!((graph.nodes.len(), graph.edges.len()), (3, 2));

    // Approving twice is refused.
    assert!(
        app.approve_proposal(proposal.id, proposal.revision)
            .await
            .is_err()
    );

    // Undo: a compensating commit hides everything again.
    let undone = app.undo_proposal(proposal.id).await.unwrap();
    assert_eq!(undone.status, ProposalStatus::Undone);
    assert!(app.concepts().await.unwrap().is_empty());
    assert!(app.concept_page(vae.id).await.is_err());
    assert!(app.search_knowledge("VAE", 10).await.unwrap().is_empty());
    assert!(app.export_public().await.unwrap().is_empty());
    assert!(vault_files.iter().all(|p| !p.exists()));
    assert_eq!(app.graph().await.unwrap().edges.len(), 0);

    // Ask again and reject: nothing is saved.
    let again = ask(&app, &mut rx, "Can you explain VAE again?").await;
    let rejected = app.reject_proposal(again.id, again.revision).await.unwrap();
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert!(app.concepts().await.unwrap().is_empty());
    assert!(
        app.undo_proposal(again.id).await.is_err(),
        "a rejected proposal has no commit"
    );

    // Ask a third time and approve: the article starts again at a fresh concept.
    let third = ask(&app, &mut rx, "vae?").await;
    app.approve_proposal(third.id, third.revision)
        .await
        .unwrap();
    assert_eq!(app.concepts().await.unwrap().len(), 3);
}

#[tokio::test]
async fn auto_mode_commits_without_asking() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(dir.path()).await;
    app.set_save_mode(SaveMode::Auto).await.unwrap();
    let mut rx = app.subscribe();

    let created = ask(&app, &mut rx, "What is a VAE?").await;
    let updated = next(&mut rx, |e| match e {
        AppEvent::ProposalUpdated { proposal } if proposal.id == created.id => Some(proposal),
        _ => None,
    })
    .await;
    assert_eq!(updated.status, ProposalStatus::Committed);
    assert_eq!(app.concepts().await.unwrap().len(), 3);
}

#[tokio::test]
async fn small_talk_proposes_nothing_and_proposals_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(dir.path()).await;
    let mut rx = app.subscribe();

    // Small talk: the tutor answers, the curator proposes nothing.
    let hello = app.send_message(None, "hello").await.unwrap();
    next(&mut rx, |e| match e {
        AppEvent::TurnFinalized { turn } if turn.id == hello.tutor_turn_id => Some(()),
        _ => None,
    })
    .await;

    // Same session, real question.
    app.send_message(Some(hello.session_id), "What is a VAE?")
        .await
        .unwrap();
    let proposal = next(&mut rx, |e| match e {
        AppEvent::ProposalCreated { proposal } => Some(proposal),
        _ => None,
    })
    .await;
    assert_eq!(
        app.pending_proposals().await.unwrap(),
        std::slice::from_ref(&proposal)
    );
    assert_eq!(app.turns(hello.session_id).await.unwrap().len(), 4);
    assert_eq!(app.list_sessions(10).await.unwrap()[0].id, hello.session_id);
    app.close().await;

    let reopened = open(dir.path()).await;
    assert_eq!(reopened.pending_proposals().await.unwrap(), [proposal]);
}

#[tokio::test]
async fn empty_messages_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(dir.path()).await;
    assert!(app.send_message(None, "   ").await.is_err());
}

#[tokio::test]
async fn edit_review_approve_preserves_evidence_and_history() {
    use sunset_core::domain::ProposalEdit;
    let dir = tempfile::tempdir().unwrap();
    let app = open(dir.path()).await;
    let mut rx = app.subscribe();
    let original = ask(&app, &mut rx, "What is a VAE?").await;
    let mut edit = ProposalEdit {
        expected_revision: original.revision,
        summary: "My corrected explanation".into(),
        changes: original.changes.clone(),
    };
    edit.changes
        .retain(|c| !matches!(c, ProposedChange::AddObservation { .. }));
    for change in &mut edit.changes {
        if let ProposedChange::ReviseArticle { body_md, .. } = change {
            *body_md = "A learner-reviewed explanation of the model.".into();
        }
    }
    // Invalid references never alter the pending content or create a history entry.
    let mut invalid = edit.clone();
    invalid.changes.push(ProposedChange::ReviseArticle {
        concept: "missing-concept".into(),
        summary: "Invalid".into(),
        body_md: "Bad reference".into(),
    });
    assert!(app.edit_proposal(original.id, invalid).await.is_err());
    assert_eq!(app.pending_proposals().await.unwrap()[0], original);
    assert_eq!(app.proposal_versions(original.id).await.unwrap().len(), 1);

    let edited = app.edit_proposal(original.id, edit.clone()).await.unwrap();
    assert_eq!(edited.revision, 1);
    assert_eq!(edited.evidence_turn_ids, original.evidence_turn_ids);
    assert_eq!(edited.session_id, original.session_id);
    assert_eq!(edited.created_at, original.created_at);
    assert_eq!(edited.status, ProposalStatus::AwaitingApproval);
    assert!(app.concepts().await.unwrap().is_empty());
    assert!(app.export_public().await.unwrap().is_empty());
    // A stale reviewer cannot overwrite, approve or reject content they have not seen.
    assert!(app.edit_proposal(original.id, edit.clone()).await.is_err());
    assert!(
        app.approve_proposal(original.id, original.revision)
            .await
            .is_err()
    );
    assert!(
        app.reject_proposal(original.id, original.revision)
            .await
            .is_err()
    );
    let history = app.proposal_versions(original.id).await.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].changes, edited.changes);
    assert_eq!(history[1].changes, original.changes);
    assert_eq!(history[1].summary, original.summary);

    app.close().await;
    let app = open(dir.path()).await;
    assert_eq!(app.proposal_versions(original.id).await.unwrap(), history);
    assert_eq!(app.pending_proposals().await.unwrap()[0], edited);
    app.approve_proposal(edited.id, edited.revision)
        .await
        .unwrap();
    let vae = app
        .concepts()
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.slug == "variational-autoencoder")
        .unwrap();
    let page = app.concept_page(vae.id).await.unwrap();
    assert_eq!(
        page.article.unwrap().body_md,
        "A learner-reviewed explanation of the model."
    );
    assert!(page.overlay.is_empty());
    assert!(
        app.export_public()
            .await
            .unwrap()
            .iter()
            .any(|f| f.markdown.contains("learner-reviewed"))
    );
    edit.expected_revision = edited.revision;
    assert!(app.edit_proposal(edited.id, edit).await.is_err());
    assert_eq!(app.proposal_versions(original.id).await.unwrap(), history);
    app.undo_proposal(edited.id).await.unwrap();
    assert!(app.export_public().await.unwrap().is_empty());
}

#[tokio::test]
async fn competing_edit_and_decision_have_exactly_one_winner() {
    use sunset_core::domain::ProposalEdit;
    for (approve, edit_first) in [(true, true), (true, false), (false, true), (false, false)] {
        let dir = tempfile::tempdir().unwrap();
        let app = open(dir.path()).await;
        let original = ask(&app, &mut app.subscribe(), "What is a VAE?").await;
        let edit = ProposalEdit {
            expected_revision: original.revision,
            summary: "Concurrent correction".into(),
            changes: original.changes.clone(),
        };
        let decision = async {
            if approve {
                app.approve_proposal(original.id, original.revision).await
            } else {
                app.reject_proposal(original.id, original.revision).await
            }
        };
        let (edited, decided) = if edit_first {
            tokio::join!(app.edit_proposal(original.id, edit), decision)
        } else {
            let (decided, edited) = tokio::join!(decision, app.edit_proposal(original.id, edit));
            (edited, decided)
        };
        assert_ne!(edited.is_ok(), decided.is_ok());
        let current = app.recent_proposals(1).await.unwrap().remove(0);
        if edited.is_ok() {
            assert_eq!(current.revision, 1);
            assert_eq!(current.status, ProposalStatus::AwaitingApproval);
            assert!(app.concepts().await.unwrap().is_empty());
        } else {
            assert_eq!(current.revision, 0);
            assert_eq!(
                current.status,
                if approve {
                    ProposalStatus::Committed
                } else {
                    ProposalStatus::Rejected
                }
            );
        }
    }
}
