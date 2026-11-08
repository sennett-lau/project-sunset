//! Phase 1 exit test (docs/architecture.md §8): the same loop with a real OpenAI-compatible
//! provider. A local fake server plays the model; the pipeline is unchanged.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use sunset_core::domain::{
    AppEvent, CapabilityStatus, Proposal, ProposalStatus, ProviderInput, ProviderKind, Role,
};
use sunset_core::{AppIdentity, MemorySecretStore, SecretStore, Sunset, SunsetConfig};
use tokio::sync::broadcast::Receiver;
use wiremock::matchers::{body_partial_json, body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Setup {
    app: Sunset,
    secrets: Arc<MemorySecretStore>,
    server: MockServer,
    _dir: tempfile::TempDir,
}

async fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let app = Sunset::open(SunsetConfig {
        identity: AppIdentity::new("Test", "dev.example.test").unwrap(),
        data_dir: dir.path().to_path_buf(),
        secrets: secrets.clone(),
        demo_chunk_delay: Duration::ZERO,
    })
    .await
    .unwrap();
    Setup {
        app,
        secrets,
        server: MockServer::start().await,
        _dir: dir,
    }
}

fn sse(text: &str) -> ResponseTemplate {
    let mut body = String::new();
    for word in text.split_inclusive(' ') {
        let chunk = json!({ "choices": [{ "delta": { "content": word } }] });
        body.push_str(&format!("data: {chunk}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    ResponseTemplate::new(200).set_body_raw(body, "text/event-stream")
}

fn reply(content: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(json!({ "choices": [{ "message": { "content": content } }] }))
}

/// Curator output as a sloppy real model might write it: reasoning, a code fence, an
/// unnormalized slug.
fn curator_reply(output: Value) -> ResponseTemplate {
    reply(&format!(
        "<think>Let me decide.</think>\n```json\n{output}\n```"
    ))
}

async fn next_proposal(rx: &mut Receiver<AppEvent>) -> Proposal {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await.unwrap() {
                AppEvent::ProposalCreated { proposal } => return proposal,
                AppEvent::JobFailed { message, .. } => panic!("curation failed: {message}"),
                AppEvent::TutorFailed { message, .. } => panic!("tutor failed: {message}"),
                _ => {}
            }
        }
    })
    .await
    .expect("timed out")
}

#[tokio::test]
async fn real_provider_loop_with_keychain_key_and_alias_dedupe() {
    let s = setup().await;
    let base_url = format!("{}/v1", s.server.uri());

    // Fresh install: both roles use the Demo provider.
    let settings = s.app.model_settings().await.unwrap();
    assert_eq!(settings.providers.len(), 1);
    assert_eq!(settings.providers[0].kind, ProviderKind::Demo);
    assert!(settings.all_local);

    // Add a provider with an API key. The key goes to the secret store, not the settings.
    let provider = s
        .app
        .save_provider(
            None,
            ProviderInput {
                name: "Fake OpenAI".into(),
                base_url: format!("{base_url}/"),
                api_key: Some("sk-secret".into()),
            },
        )
        .await
        .unwrap();
    assert!(provider.has_api_key);
    assert!(provider.is_local, "127.0.0.1 is local");
    assert_eq!(provider.base_url, base_url, "trailing slash trimmed");
    assert_eq!(
        s.secrets.get(&provider.id.to_string()).unwrap().as_deref(),
        Some("sk-secret")
    );
    let shown = serde_json::to_string(&s.app.model_settings().await.unwrap()).unwrap();
    assert!(
        !shown.contains("sk-secret"),
        "settings never contain the key"
    );

    // Probe: this server supports JSON schema and streaming.
    Mock::given(body_partial_json(json!({ "stream": true })))
        .and(body_string_contains("Reply with the single word"))
        .respond_with(sse("OK"))
        .with_priority(1)
        .mount(&s.server)
        .await;
    Mock::given(body_partial_json(
        json!({ "response_format": { "json_schema": { "name": "sum_answer" } } }),
    ))
    .respond_with(reply("{\"sum\":5}"))
    .with_priority(1)
    .mount(&s.server)
    .await;
    Mock::given(body_string_contains("Reply with the single word"))
        .respond_with(reply("OK"))
        .with_priority(2)
        .mount(&s.server)
        .await;
    Mock::given(body_string_contains("What is 2 + 3"))
        .respond_with(reply("five"))
        .with_priority(3)
        .mount(&s.server)
        .await;
    let report = s
        .app
        .test_connection(provider.id, "fake-model")
        .await
        .unwrap();
    assert!(report.reachable, "{:?}", report.notes);
    assert_eq!(report.capabilities.streaming, CapabilityStatus::Supported);
    assert_eq!(report.capabilities.json_schema, CapabilityStatus::Supported);
    assert_eq!(
        report.capabilities.tool_calls,
        CapabilityStatus::Unsupported
    );

    s.app
        .set_role(Role::Tutor, provider.id, "fake-model")
        .await
        .unwrap();
    s.app
        .set_role(Role::Curator, provider.id, "fake-model")
        .await
        .unwrap();
    let settings = s.app.model_settings().await.unwrap();
    assert!(settings.roles.iter().all(|r| r.provider_id == provider.id));
    assert_eq!(
        settings.roles[1].capabilities.json_schema,
        CapabilityStatus::Supported
    );

    // The provider cannot be removed while roles use it.
    assert!(s.app.delete_provider(provider.id).await.is_err());

    // Tutor: streamed answer. Curator: JSON schema request, sloppy JSON reply.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({ "stream": true })))
        .respond_with(sse("A VAE learns a distribution over latent codes."))
        .mount(&s.server)
        .await;
    Mock::given(body_partial_json(json!({ "response_format": { "json_schema": { "name": "curator_output" } } })))
        .and(body_string_contains("What is a VAE?"))
        .respond_with(curator_reply(json!({ "proposals": [{
            "summary": "Add VAE",
            "changes": [
                { "op": "upsert_concept", "slug": "Variational Autoencoder", "title": "Variational Autoencoder", "aliases": ["VAE"] },
                { "op": "revise_article", "concept": "Variational Autoencoder", "summary": "A generative model.", "bodyMd": "" },
                { "op": "add_observation", "concept": "variational-autoencoder", "kind": "question", "source": "observed", "note": "Asked what a VAE is." }
            ]
        }]})))
        .mount(&s.server)
        .await;

    let mut rx = s.app.subscribe();
    s.app.send_message(None, "What is a VAE?").await.unwrap();
    let first = next_proposal(&mut rx).await;
    s.app
        .approve_proposal(first.id, first.revision)
        .await
        .unwrap();
    let concepts = s.app.concepts().await.unwrap();
    assert_eq!(concepts.len(), 1);
    assert_eq!(
        concepts[0].slug, "variational-autoencoder",
        "slug normalized"
    );

    // The tutor got the key and the turn history.
    let tutor_call = s
        .server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .find(|r| {
            r.body_json::<Value>()
                .is_ok_and(|b| b["stream"] == true && b["model"] == "fake-model")
        })
        .unwrap();
    assert_eq!(tutor_call.headers["authorization"], "Bearer sk-secret");

    // Second exchange: the model calls it "VAE" with a new alias. It must reuse the concept.
    Mock::given(body_partial_json(json!({ "response_format": { "json_schema": { "name": "curator_output" } } })))
        .and(body_string_contains("encoder"))
        .respond_with(curator_reply(json!({ "proposals": [{
            "summary": "Expand VAE",
            "changes": [
                { "op": "upsert_concept", "slug": "vae", "title": "VAE", "aliases": ["變分自編碼器"] },
                { "op": "revise_article", "concept": "vae", "summary": "A generative model with an encoder and decoder.", "bodyMd": "" }
            ]
        }]})))
        .with_priority(1)
        .mount(&s.server)
        .await;
    s.app
        .send_message(None, "What does the encoder do?")
        .await
        .unwrap();
    let second = next_proposal(&mut rx).await;
    s.app
        .approve_proposal(second.id, second.revision)
        .await
        .unwrap();

    let concepts = s.app.concepts().await.unwrap();
    assert_eq!(concepts.len(), 1, "no duplicate concept");
    let page = s.app.concept_page(concepts[0].id).await.unwrap();
    assert_eq!(page.article.as_ref().unwrap().version, 2);
    assert_eq!(page.concept.aliases, ["VAE", "變分自編碼器"]);

    // Undo removes the alias along with the revision.
    let undone = s.app.undo_proposal(second.id).await.unwrap();
    assert_eq!(undone.status, ProposalStatus::Undone);
    let page = s.app.concept_page(concepts[0].id).await.unwrap();
    assert_eq!(page.concept.aliases, ["VAE"]);
    assert_eq!(page.article.unwrap().version, 1);
}

#[tokio::test]
async fn provider_errors_reach_the_user_without_fallback() {
    let s = setup().await;
    let provider = s
        .app
        .save_provider(
            None,
            ProviderInput {
                name: "Broken".into(),
                base_url: format!("{}/v1", s.server.uri()),
                api_key: None,
            },
        )
        .await
        .unwrap();
    assert!(!provider.has_api_key);
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(json!({ "error": { "message": "bad key" } })),
        )
        .mount(&s.server)
        .await;
    s.app.set_role(Role::Tutor, provider.id, "m").await.unwrap();

    let mut rx = s.app.subscribe();
    s.app.send_message(None, "What is a VAE?").await.unwrap();
    let message = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let AppEvent::TutorFailed { message, .. } = rx.recv().await.unwrap() {
                return message;
            }
        }
    })
    .await
    .unwrap();
    assert!(message.contains("API key was rejected"), "{message}");

    // Settings validation.
    let bad = ProviderInput {
        name: "x".into(),
        base_url: "ftp://example.com".into(),
        api_key: None,
    };
    assert!(s.app.save_provider(None, bad).await.is_err());
    let demo = s.app.model_settings().await.unwrap().providers[0].id;
    assert!(s.app.delete_provider(demo).await.is_err(), "Demo stays");

    // Removing the key clears it from the store; switching the role back frees the provider.
    s.app
        .save_provider(
            Some(provider.id),
            ProviderInput {
                name: "Broken".into(),
                base_url: format!("{}/v1", s.server.uri()),
                api_key: Some(String::new()),
            },
        )
        .await
        .unwrap();
    s.app.set_role(Role::Tutor, demo, "scripted").await.unwrap();
    s.app.delete_provider(provider.id).await.unwrap();
    assert_eq!(s.app.model_settings().await.unwrap().providers.len(), 1);
}
