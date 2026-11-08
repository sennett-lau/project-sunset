//! Opt-in test against a user-selected real model. Uses an isolated temporary library.
//! SUNSET_TEST_BASE_URL=http://host:port/v1 SUNSET_TEST_MODEL=model \
//! cargo test -p sunset-core --test live_model -- --ignored --nocapture

use std::sync::Arc;
use std::time::{Duration, Instant};

use sunset_core::domain::{AppEvent, Proposal, ProposalStatus, ProviderInput, Role};
use sunset_core::{AppIdentity, MemorySecretStore, Sunset, SunsetConfig};

#[tokio::test]
#[ignore = "requires an explicitly selected live model endpoint"]
async fn live_tutor_curator_approve_restart_undo() {
    let base_url = std::env::var("SUNSET_TEST_BASE_URL").expect("SUNSET_TEST_BASE_URL required");
    let model = std::env::var("SUNSET_TEST_MODEL").expect("SUNSET_TEST_MODEL required");
    let dir = tempfile::tempdir().unwrap();
    let config = || SunsetConfig {
        identity: AppIdentity::new("Live Test", "dev.example.sunset-live-test").unwrap(),
        data_dir: dir.path().to_owned(),
        secrets: Arc::new(MemorySecretStore::default()),
        demo_chunk_delay: Duration::ZERO,
    };
    let app = Sunset::open(config()).await.unwrap();
    let provider = app
        .save_provider(
            None,
            ProviderInput {
                name: "Explicit live test".into(),
                base_url,
                api_key: None,
            },
        )
        .await
        .unwrap();
    let started = Instant::now();
    let report = app.test_connection(provider.id, &model).await.unwrap();
    println!("probe {:?}: {:?}", started.elapsed(), report);
    assert!(report.reachable);
    app.set_role(Role::Tutor, provider.id, &model)
        .await
        .unwrap();
    app.set_role(Role::Curator, provider.id, &model)
        .await
        .unwrap();

    let mut events = app.subscribe();
    let started = Instant::now();
    let question = std::env::var("SUNSET_TEST_QUESTION").unwrap_or_else(|_| {
        "What is a variational autoencoder (VAE)? Explain the encoder, latent space and decoder in fewer than 150 words. I prefer concrete examples.".into()
    });
    let sent = app.send_message(None, &question).await.unwrap();
    let mut answer = String::new();
    let mut first_delta = None;
    let proposal: Proposal = tokio::time::timeout(Duration::from_secs(240), async {
        loop {
            match events.recv().await.unwrap() {
                AppEvent::TutorDelta { text, .. } => {
                    first_delta.get_or_insert_with(|| started.elapsed());
                    answer.push_str(&text);
                }
                AppEvent::ProposalCreated { proposal } => break proposal,
                AppEvent::TutorFailed { message, .. } => panic!("live tutor failed: {message}"),
                AppEvent::JobFailed { message, .. } => panic!("live curator failed: {message}"),
                _ => {}
            }
        }
    })
    .await
    .expect("live loop did not create a proposal within 240 seconds");
    println!(
        "first delta: {first_delta:?}; proposal ready: {:?}; answer: {answer}",
        started.elapsed()
    );
    println!(
        "proposal: {}",
        serde_json::to_string_pretty(&proposal.changes).unwrap()
    );
    assert!(!answer.trim().is_empty());
    assert!(!answer.contains("<think>"));
    assert!(
        app.concepts().await.unwrap().is_empty(),
        "pending changes must remain private to review"
    );
    let committed = app.approve_proposal(proposal.id).await.unwrap();
    assert_eq!(committed.status, ProposalStatus::Committed);
    assert!(!app.concepts().await.unwrap().is_empty());
    assert!(app.turns(sent.session_id).await.unwrap().len() >= 2);
    let concepts = app.concepts().await.unwrap();
    assert!(
        !app.search_knowledge(&concepts[0].title, 10)
            .await
            .unwrap()
            .is_empty()
    );
    let public = app.export_public().await.unwrap();
    let snapshot = app.export_public_to_folder().await.unwrap();
    assert!(
        !std::path::Path::new(&snapshot)
            .join(".private_overlay")
            .exists()
    );
    for file in &public {
        assert_eq!(
            std::fs::read_to_string(std::path::Path::new(&snapshot).join(&file.filename)).unwrap(),
            file.markdown
        );
    }
    assert!(app.vault_status().error.is_none());
    app.close().await;

    let reopened = Sunset::open(config()).await.unwrap();
    assert!(!reopened.concepts().await.unwrap().is_empty());
    assert!(reopened.turns(sent.session_id).await.unwrap().len() >= 2);
    let undone = reopened.undo_proposal(proposal.id).await.unwrap();
    assert_eq!(undone.status, ProposalStatus::Undone);
    assert!(reopened.concepts().await.unwrap().is_empty());
    assert!(
        reopened
            .search_knowledge(&concepts[0].title, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(reopened.export_public().await.unwrap().is_empty());
    reopened.close().await;
}
