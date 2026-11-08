//! Ask the curator model for proposals and parse its JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sunset_domain::{Concept, ProposedChange, Turn};
use sunset_providers::structured::extract_json;
use sunset_providers::{ChatMessage, ChatModel, ChatRequest, ProviderError, ResponseFormat};

use super::validate;
use crate::{CoreError, Result};

const CURATOR_PROMPT: &str = include_str!("../prompts/curator.md");

/// What the curator model must return. Its JSON schema is sent to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CuratorOutput {
    #[schemars(length(max = 3))]
    pub proposals: Vec<ProposalDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProposalDraft {
    /// One line describing the whole change, shown to the learner for review.
    pub summary: String,
    #[schemars(length(min = 1, max = 12))]
    pub changes: Vec<ProposedChange>,
}

/// Labels the mock curator also reads, so keep them stable.
pub const QUESTION_LABEL: &str = "Learner question:";
pub const ANSWER_LABEL: &str = "Tutor answer:";

/// How many existing concepts the curator sees, so it can reuse them instead of duplicating.
const MAX_EXISTING: usize = 300;

pub async fn propose(
    model: &dyn ChatModel,
    user_turn: &Turn,
    tutor_turn: &Turn,
    existing: &[Concept],
) -> Result<CuratorOutput> {
    let known: Vec<String> = existing
        .iter()
        .take(MAX_EXISTING)
        .map(|c| {
            if c.aliases.is_empty() {
                format!("- {} ({})", c.slug, c.title)
            } else {
                format!("- {} ({}; also: {})", c.slug, c.title, c.aliases.join(", "))
            }
        })
        .collect();
    let exchange = format!(
        "{QUESTION_LABEL}\n{}\n\n{ANSWER_LABEL}\n{}\n\nExisting concepts (slug, title):\n{}",
        user_turn.text,
        tutor_turn.text,
        if known.is_empty() {
            "(none)".to_owned()
        } else {
            known.join("\n")
        },
    );
    let schema = serde_json::to_value(schemars::schema_for!(CuratorOutput))
        .map_err(|e| CoreError::Invalid(e.to_string()))?;
    let mut request = ChatRequest {
        messages: vec![
            ChatMessage::system(CURATOR_PROMPT),
            ChatMessage::user(exchange),
        ],
        response_format: ResponseFormat::Json {
            name: "curator_output".into(),
            schema,
        },
        temperature: Some(0.2),
    };

    // Retry shape AND semantic validation errors. Silently dropping a well-formed proposal
    // leaves the user waiting forever when a small model references a nonexistent concept.
    let error = match model.complete(request.clone()).await {
        Ok(first) => match validated(&first.content, existing) {
            Ok(out) => return Ok(out),
            Err(error) => error,
        },
        // A simple probe can pass even when a server's grammar struggles with the full
        // tagged union. Retry once in prompt mode on the SAME model/provider, never a cloud
        // fallback. Transport/auth failures are not retried here.
        Err(ProviderError::BadResponse(error)) => error,
        Err(error) => return Err(error.into()),
    };
    request.response_format = ResponseFormat::Text;
    request.messages.push(ChatMessage::user(format!(
        "The previous structured-output attempt was invalid: {error}. Start over with ONE small proposal containing only an upsert_concept and a revise_article for the main concept. Use one brief paragraph for bodyMd; no equations, backslashes, diagrams, relations or copied transcript. Every referenced concept must exist or have an upsert_concept in this same proposal. Return only a COMPLETE JSON object, ending with the closing array and object brackets. Example shape (replace the content): {{\"proposals\":[{{\"summary\":\"Add concept\",\"changes\":[{{\"op\":\"upsert_concept\",\"slug\":\"main-concept\",\"title\":\"Main concept\",\"aliases\":[]}},{{\"op\":\"revise_article\",\"concept\":\"main-concept\",\"summary\":\"Short definition\",\"bodyMd\":\"One short explanatory paragraph.\"}}]}}]}}"
    )));
    let second = model.complete(request).await?.content;
    validated(&second, existing).map_err(|e| CoreError::Invalid(format!("curator output: {e}")))
}

fn validated(content: &str, existing: &[Concept]) -> Result<CuratorOutput, String> {
    let mut output = parse(content).map_err(|e| e.to_string())?;
    let names = validate::NameIndex::new(existing);
    for draft in &mut output.proposals {
        names.normalize(draft);
        validate::draft(draft, &names)?;
    }
    Ok(output)
}

/// Parse model output, tolerating reasoning blocks, code fences and prose around the JSON.
pub fn parse(content: &str) -> Result<CuratorOutput, serde_json::Error> {
    serde_json::from_str(extract_json(content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let out = parse("<think>hmm</think>```json\n{\"proposals\": []}\n```").unwrap();
        assert!(out.proposals.is_empty());
        assert!(parse("not json").is_err());
    }

    #[test]
    fn well_formed_but_invalid_proposals_are_not_silently_lost() {
        let invalid = r#"{"proposals":[{"summary":"Add relation","changes":[{"op":"add_relation","from":"vae","to":"missing","kind":"USES","label":null}]}]}"#;
        assert!(parse(invalid).is_ok());
        assert!(
            validated(invalid, &[])
                .unwrap_err()
                .contains("unknown concept")
        );
        assert!(validated(r#"{"proposals":[]}"#, &[]).is_ok());
    }

    #[tokio::test]
    async fn semantic_failure_gets_one_repair_on_the_same_model() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use sunset_domain::{SessionId, Speaker, TurnId};
        use sunset_providers::adapters::mock::MockChatModel;
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let model = MockChatModel::new("repair-test", move |request| {
            if count.fetch_add(1, Ordering::SeqCst) == 0 {
                assert!(matches!(
                    request.response_format,
                    ResponseFormat::Json { .. }
                ));
                r#"{"proposals":[{"summary":"Bad reference","changes":[{"op":"revise_article","concept":"missing","summary":"x","bodyMd":""}]}]}"#.into()
            } else {
                assert_eq!(request.response_format, ResponseFormat::Text);
                assert!(
                    request
                        .last_user_message()
                        .unwrap()
                        .contains("unknown concept")
                );
                r#"{"proposals":[{"summary":"Add VAE","changes":[{"op":"upsert_concept","slug":"vae","title":"VAE","aliases":[]},{"op":"revise_article","concept":"vae","summary":"A generative model","bodyMd":""}]}]}"#.into()
            }
        });
        let user = Turn {
            id: TurnId::new(),
            session_id: SessionId::new(),
            speaker: Speaker::User,
            text: "What is a VAE?".into(),
            created_at: chrono::Utc::now(),
        };
        let tutor = Turn {
            id: TurnId::new(),
            speaker: Speaker::Tutor,
            text: "A generative model.".into(),
            ..user.clone()
        };
        let output = propose(&model, &user, &tutor, &[]).await.unwrap();
        assert_eq!(output.proposals.len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
