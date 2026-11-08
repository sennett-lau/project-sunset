//! Ask the curator model for proposals and parse its JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sunset_domain::{Concept, ProposedChange, Turn};
use sunset_providers::structured::extract_json;
use sunset_providers::{ChatMessage, ChatModel, ChatRequest, ResponseFormat};

use crate::{CoreError, Result};

const CURATOR_PROMPT: &str = include_str!("../prompts/curator.md");

/// What the curator model must return. Its JSON schema is sent to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CuratorOutput {
    pub proposals: Vec<ProposalDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProposalDraft {
    /// One line describing the whole change, shown to the learner for review.
    pub summary: String,
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

    // One retry with the parse error, then give up (docs/architecture.md §4.4).
    let first = model.complete(request.clone()).await?.content;
    match parse(&first) {
        Ok(out) => Ok(out),
        Err(error) => {
            request.messages.push(ChatMessage::assistant(first));
            request.messages.push(ChatMessage::user(format!(
                "That was not valid JSON for the required shape: {error}. Reply with corrected JSON only."
            )));
            let second = model.complete(request).await?.content;
            parse(&second).map_err(|e| CoreError::Invalid(format!("curator output: {e}")))
        }
    }
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
}
