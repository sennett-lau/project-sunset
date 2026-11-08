use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use sunset_core::domain::{ProviderInput, Role};
use sunset_core::{AppIdentity, MemorySecretStore, Sunset, SunsetConfig};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn speech_is_opt_in_pins_destination_and_creates_no_conversation() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let config = || SunsetConfig {
        identity: AppIdentity::new("Speech Test", "dev.example.speech-test").unwrap(),
        data_dir: dir.path().to_owned(),
        secrets: secrets.clone(),
        demo_chunk_delay: Duration::ZERO,
    };
    let app = Sunset::open(config()).await.unwrap();
    assert!(app.prepare_speech().await.is_err());
    let settings = app.model_settings().await.unwrap();
    assert_eq!(settings.roles.len(), 2);
    assert!(
        app.set_role(Role::Stt, settings.providers[0].id, "scripted")
            .await
            .is_err()
    );

    let original = MockServer::start().await;
    let replacement = MockServer::start().await;
    let provider = app
        .save_provider(
            None,
            ProviderInput {
                name: "Speech endpoint".into(),
                base_url: format!("{}/v1", original.uri()),
                api_key: Some("test-only-key".into()),
            },
        )
        .await
        .unwrap();
    app.set_role(Role::Stt, provider.id, "asr-model")
        .await
        .unwrap();
    assert!(app.delete_provider(provider.id).await.is_err());
    let prepared = app.prepare_speech().await.unwrap();
    // Editing the provider after capture begins must never reroute the existing recording.
    app.save_provider(
        Some(provider.id),
        ProviderInput {
            name: "Changed endpoint".into(),
            base_url: format!("{}/v1", replacement.uri()),
            api_key: Some("replacement-key".into()),
        },
    )
    .await
    .unwrap();
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .and(header("authorization", "Bearer test-only-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"text": "What is a VAE?"})))
        .expect(1)
        .mount(&original)
        .await;
    let mut wav = vec![0; 46];
    wav[..4].copy_from_slice(b"RIFF");
    wav[8..12].copy_from_slice(b"WAVE");
    assert_eq!(prepared.transcribe(wav).await.unwrap(), "What is a VAE?");
    assert!(replacement.received_requests().await.unwrap().is_empty());
    assert!(app.list_sessions(10).await.unwrap().is_empty());
    assert!(app.pending_proposals().await.unwrap().is_empty());
    drop(app);
    let app = Sunset::open(config()).await.unwrap();
    assert!(
        app.model_settings()
            .await
            .unwrap()
            .roles
            .iter()
            .any(|r| r.role == Role::Stt && r.model == "asr-model")
    );
    assert!(app.prepare_speech().await.is_ok());
    app.clear_speech_model().await.unwrap();
    assert!(app.prepare_speech().await.is_err());
    assert_eq!(app.model_settings().await.unwrap().roles.len(), 2);
    app.delete_provider(provider.id).await.unwrap();
}
