//! Ask the curator model for proposals and parse its JSON.

use serde::{Deserialize, Serialize};
use sunset_domain::{ConceptSummary, ProposedChange, Turn};
use sunset_providers::{ChatMessage, ChatModel, ChatRequest, ResponseFormat};

use crate::{CoreError, Result};

const CURATOR_PROMPT: &str = include_str!("../prompts/curator.md");

/// What the curator model must return.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CuratorOutput {
    pub proposals: Vec<ProposalDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalDraft {
    pub summary: String,
    pub changes: Vec<ProposedChange>,
}

/// Labels the mock curator also reads, so keep them stable.
pub const QUESTION_LABEL: &str = "Learner question:";
pub const ANSWER_LABEL: &str = "Tutor answer:";

pub async fn propose(
    model: &dyn ChatModel,
    user_turn: &Turn,
    tutor_turn: &Turn,
    existing: &[ConceptSummary],
) -> Result<CuratorOutput> {
    let slugs: Vec<&str> = existing.iter().map(|c| c.slug.as_str()).collect();
    let exchange = format!(
        "{QUESTION_LABEL}\n{}\n\n{ANSWER_LABEL}\n{}\n\nExisting concept slugs: {}",
        user_turn.text,
        tutor_turn.text,
        if slugs.is_empty() {
            "(none)".to_owned()
        } else {
            slugs.join(", ")
        },
    );
    let mut request = ChatRequest {
        messages: vec![
            ChatMessage::system(CURATOR_PROMPT),
            ChatMessage::user(exchange),
        ],
        // Phase 1 replaces this loose schema with one derived from CuratorOutput.
        response_format: ResponseFormat::Json {
            name: "curator_output".into(),
            schema: serde_json::json!({ "type": "object", "required": ["proposals"] }),
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

/// Parse model output, tolerating a Markdown code fence around the JSON.
pub fn parse(content: &str) -> Result<CuratorOutput, serde_json::Error> {
    let trimmed = content.trim();
    let body = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(trimmed);
    serde_json::from_str(body.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let out = parse("```json\n{\"proposals\": []}\n```").unwrap();
        assert!(out.proposals.is_empty());
        assert!(parse("not json").is_err());
    }
}
