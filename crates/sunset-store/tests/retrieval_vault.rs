use std::collections::BTreeMap;
use sunset_domain::{ObservationKind, ObservationSource, ProposalId};
use sunset_store::{
    Store,
    repo::{commits, knowledge as kn, users},
    search, vault,
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

#[tokio::test]
async fn mirror_preserves_edits_unmanaged_files_and_refuses_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    let store = Store::open(&base.join("db")).await.unwrap();
    let mut conn = store.conn().await.unwrap();
    let root = base.join("vault");
    let desired = BTreeMap::from([
        ("concept.md".into(), "first".into()),
        (".private_overlay/concept.md".into(), "secret".into()),
    ]);
    assert!(
        vault::sync(&mut conn, &root, &desired)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read_to_string(root.join("concept.md")).unwrap(),
        "first"
    );
    assert!(
        vault::sync(&mut conn, &root, &desired)
            .await
            .unwrap()
            .is_empty()
    );
    std::fs::write(root.join("concept.md"), "user edit").unwrap();
    std::fs::write(root.join("unmanaged.md"), "user file").unwrap();
    let conflicts = vault::sync(&mut conn, &root, &BTreeMap::new())
        .await
        .unwrap();
    assert_eq!(conflicts, ["concept.md"]);
    assert_eq!(
        std::fs::read_to_string(root.join("concept.md")).unwrap(),
        "user edit"
    );
    assert!(root.join("unmanaged.md").exists());
    assert!(!root.join(".private_overlay/concept.md").exists());
    let target = base.join("outside");
    std::fs::write(&target, "unchanged").unwrap();
    std::os::unix::fs::symlink(&target, root.join("link.md")).unwrap();
    let unsafe_files = BTreeMap::from([
        ("link.md".into(), "bad".into()),
        ("../outside.md".into(), "bad".into()),
    ]);
    assert_eq!(
        vault::sync(&mut conn, &root, &unsafe_files)
            .await
            .unwrap()
            .len(),
        3
    );
    assert_eq!(std::fs::read_to_string(target).unwrap(), "unchanged");
    let linked_root = base.join("linked");
    std::os::unix::fs::symlink(&root, &linked_root).unwrap();
    assert!(
        vault::sync(&mut conn, &linked_root, &desired)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn mirror_recovers_an_interrupted_rename_before_a_new_revision() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    let store = Store::open(&base.join("db")).await.unwrap();
    let mut conn = store.conn().await.unwrap();
    let root = base.join("vault");
    let first = BTreeMap::from([("concept.md".into(), "first".into())]);
    vault::sync(&mut conn, &root, &first).await.unwrap();
    // Simulate crash after persisted intent + rename, before manifest acknowledgement.
    sqlx::query("UPDATE vault_files SET pending_content='second' WHERE path='concept.md'")
        .execute(&mut *conn)
        .await
        .unwrap();
    std::fs::write(root.join("concept.md"), "second").unwrap();
    let newest = BTreeMap::from([("concept.md".into(), "third".into())]);
    assert!(
        vault::sync(&mut conn, &root, &newest)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read_to_string(root.join("concept.md")).unwrap(),
        "third"
    );
}

#[test]
fn native_exports_reject_traversal_and_symlink_destinations() {
    use sunset_domain::PublicExportFile;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let bad = PublicExportFile {
        filename: "../escape.md".into(),
        markdown: "bad".into(),
    };
    assert!(vault::export_snapshot(&root.join("exports"), &[bad]).is_err());
    assert!(!root.join("escape.md").exists());
    let outside = root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("exports")).unwrap();
    let good = PublicExportFile {
        filename: "safe.md".into(),
        markdown: "public".into(),
    };
    assert!(vault::export_snapshot(&root.join("exports"), &[good]).is_err());
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
}
