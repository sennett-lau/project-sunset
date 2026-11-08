use sunset_domain::{ObservationKind, ObservationSource, ProposalId};
use sunset_store::{
    Store,
    repo::{commits, knowledge as kn, users},
    search,
};

#[tokio::test]
async fn lexical_multilingual_literal_current_and_private_exclusion() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("db")).await.unwrap();
    let mut tx = store.begin().await.unwrap();
    let (user, _) = users::local_user(&mut tx).await.unwrap();
    let commit = commits::insert(&mut tx, user, ProposalId::new(), None)
        .await
        .unwrap();
    let c = kn::insert_concept(
        &mut tx,
        user,
        commit.id,
        "vae",
        "Variational autoencoder",
        &["變分自編碼器".into(), "VAE".into()],
    )
    .await
    .unwrap();
    kn::insert_revision(
        &mut tx,
        commit.id,
        c.id,
        "latent distribution",
        "分佈 模型 exactterm",
    )
    .await
    .unwrap();
    kn::insert_observation(
        &mut tx,
        user,
        commit.id,
        c.id,
        ObservationKind::Confusion,
        ObservationSource::SelfReported,
        "PRIVATESECRET",
        &[],
    )
    .await
    .unwrap();
    search::refresh(&mut tx).await.unwrap();
    for term in [
        "VAE",
        "變分自編碼器",
        "分佈",
        "模型",
        "exactterm",
        "\"VAE\"",
        "vae* OR NEAR(foo)",
    ] {
        assert_eq!(
            search::search(&mut tx, user, term, 10).await.unwrap()[0].id,
            c.id,
            "{term}"
        );
    }
    assert!(
        search::search(&mut tx, user, "PRIVATESECRET", 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        search::search(&mut tx, user, "\" * ()", 10)
            .await
            .unwrap()
            .is_empty()
    );
    kn::insert_revision(&mut tx, commit.id, c.id, "replacement", "newterm")
        .await
        .unwrap();
    search::refresh(&mut tx).await.unwrap();
    assert!(
        search::search(&mut tx, user, "exactterm", 10)
            .await
            .unwrap()
            .is_empty()
    );
    let undo = commits::insert(&mut tx, user, commit.proposal_id, Some(commit.id))
        .await
        .unwrap();
    kn::retract_commit(&mut tx, commit.id, undo.id)
        .await
        .unwrap();
    search::refresh(&mut tx).await.unwrap();
    assert!(
        search::search(&mut tx, user, "VAE", 10)
            .await
            .unwrap()
            .is_empty()
    );
    tx.commit().await.unwrap();
}
