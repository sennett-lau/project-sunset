use sunset_domain::{ProposalId, RelationKind};
use sunset_store::Store;
use sunset_store::repo::{commits, knowledge, users};

#[tokio::test]
async fn retract_hides_rows_and_frees_the_slug() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("library.sqlite"))
        .await
        .unwrap();
    let mut conn = store.conn().await.unwrap();
    let (user, _) = users::local_user(&mut conn).await.unwrap();

    let first = commits::insert(&mut conn, user, ProposalId::new(), None)
        .await
        .unwrap();
    let vae = knowledge::insert_concept(&mut conn, user, first.id, "vae", "VAE", &[])
        .await
        .unwrap();
    let ae =
        knowledge::insert_concept(&mut conn, user, first.id, "autoencoder", "Autoencoder", &[])
            .await
            .unwrap();
    knowledge::insert_revision(&mut conn, first.id, vae.id, "A generative model.", "Body")
        .await
        .unwrap();
    knowledge::insert_relation(
        &mut conn,
        user,
        first.id,
        vae.id,
        ae.id,
        RelationKind::IsA,
        None,
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        knowledge::list_concepts(&mut conn, user)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        knowledge::all_relations(&mut conn, user)
            .await
            .unwrap()
            .len(),
        1
    );

    let undo = commits::insert(&mut conn, user, first.proposal_id, Some(first.id))
        .await
        .unwrap();
    let hidden = knowledge::retract_commit(&mut conn, first.id, undo.id)
        .await
        .unwrap();
    assert_eq!(hidden, 4);
    assert!(
        knowledge::list_concepts(&mut conn, user)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        knowledge::current_revision(&mut conn, vae.id)
            .await
            .unwrap()
            .is_none()
    );

    // The slug is free again, and revision numbers keep counting.
    let again = commits::insert(&mut conn, user, ProposalId::new(), None)
        .await
        .unwrap();
    knowledge::insert_concept(&mut conn, user, again.id, "vae", "VAE", &[])
        .await
        .unwrap();
}
