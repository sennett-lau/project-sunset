//! "Test connection": small requests that find out what a model really supports
//! (docs/architecture.md §4.4). An "OpenAI-compatible" URL promises nothing.

use futures::StreamExt;
use serde_json::{Value, json};

use crate::adapters::openai_compat::{OpenAiCompatConfig, OpenAiCompatModel};
use crate::structured::extract_json;
use crate::{
    Capability, ChatCapabilities, ChatMessage, ChatModel, ChatRequest, ProviderError,
    ResponseFormat,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ProbeOutcome {
    pub reachable: bool,
    pub capabilities: ChatCapabilities,
    pub notes: Vec<String>,
}

pub async fn probe_openai_compat(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
) -> ProbeOutcome {
    let with = |capabilities: ChatCapabilities| {
        OpenAiCompatModel::new(
            client.clone(),
            OpenAiCompatConfig {
                base_url: base_url.to_owned(),
                api_key: api_key.map(str::to_owned),
                model: model.to_owned(),
                capabilities,
            },
        )
    };
    let none = ChatCapabilities {
        streaming: Capability::Unsupported,
        tool_calls: Capability::Unsupported,
        json_schema: Capability::Unsupported,
        context_window: None,
    };
    let mut caps = ChatCapabilities::default();
    let mut notes = Vec::new();

    // 1. Basic request.
    let hello = ChatRequest {
        messages: vec![ChatMessage::user("Reply with the single word: OK")],
        ..Default::default()
    };
    if let Err(e) = with(none.clone()).complete(hello.clone()).await {
        notes.push(format!("Basic request failed: {e}"));
        return ProbeOutcome {
            reachable: false,
            capabilities: caps,
            notes,
        };
    }
    notes.push("Basic request: OK".into());

    // 2. Streaming.
    let streaming = ChatCapabilities {
        streaming: Capability::Supported,
        ..none.clone()
    };
    caps.streaming = match with(streaming).stream(hello).await {
        Ok(mut stream) => {
            let mut got_text = false;
            let mut failure = None;
            while let Some(delta) = stream.next().await {
                match delta {
                    Ok(_) => got_text = true,
                    Err(e) => {
                        failure = Some(e.to_string());
                        break;
                    }
                }
            }
            verdict(
                &mut notes,
                "Streaming",
                got_text && failure.is_none(),
                failure,
            )
        }
        Err(e) => verdict(&mut notes, "Streaming", false, Some(e.to_string())),
    };

    // 3. Native JSON schema, then 4. tool calls, both on a question with a checkable answer.
    let sum = ChatRequest {
        messages: vec![ChatMessage::user("What is 2 + 3? Answer in JSON.")],
        response_format: ResponseFormat::Json {
            name: "sum_answer".into(),
            schema: json!({
                "type": "object",
                "properties": { "sum": { "type": "integer" } },
                "required": ["sum"],
            }),
        },
        temperature: Some(0.0),
    };
    let schema_caps = ChatCapabilities {
        json_schema: Capability::Supported,
        ..none.clone()
    };
    caps.json_schema = match with(schema_caps).complete(sum.clone()).await {
        Ok(r) => verdict(&mut notes, "JSON schema output", is_five(&r.content), None),
        Err(e) => verdict(&mut notes, "JSON schema output", false, Some(rejection(&e))),
    };
    let tool_caps = ChatCapabilities {
        tool_calls: Capability::Supported,
        ..none
    };
    caps.tool_calls = match with(tool_caps).complete(sum).await {
        Ok(r) => verdict(
            &mut notes,
            "Tool calls",
            r.via_tool_call && is_five(&r.content),
            None,
        ),
        Err(e) => verdict(&mut notes, "Tool calls", false, Some(rejection(&e))),
    };

    ProbeOutcome {
        reachable: true,
        capabilities: caps,
        notes,
    }
}

fn is_five(content: &str) -> bool {
    serde_json::from_str::<Value>(extract_json(content)).is_ok_and(|v| v["sum"].as_i64() == Some(5))
}

fn rejection(e: &ProviderError) -> String {
    match e {
        ProviderError::Status { status, .. } => format!("rejected (HTTP {status})"),
        other => other.to_string(),
    }
}

fn verdict(notes: &mut Vec<String>, what: &str, ok: bool, why: Option<String>) -> Capability {
    let detail = why.unwrap_or_else(|| {
        if ok {
            "OK".into()
        } else {
            "not followed".into()
        }
    });
    notes.push(format!("{what}: {detail}"));
    if ok {
        Capability::Supported
    } else {
        Capability::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::openai_compat::http_client;
    use wiremock::matchers::{body_partial_json, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn content(text: &str) -> ResponseTemplate {
        ResponseTemplate::new(200)
            .set_body_json(json!({ "choices": [{ "message": { "content": text } }] }))
    }

    /// A server with streaming and JSON schema, which ignores tools and answers in prose.
    #[tokio::test]
    async fn finds_what_the_server_supports() {
        let server = MockServer::start().await;
        Mock::given(body_partial_json(json!({ "stream": true })))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\ndata: [DONE]\n\n",
                "text/event-stream",
            ))
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(body_partial_json(
            json!({ "response_format": { "type": "json_schema" } }),
        ))
        .respond_with(content("{\"sum\": 5}"))
        .with_priority(1)
        .mount(&server)
        .await;
        Mock::given(method("POST"))
            .respond_with(content("OK, the sum is 5."))
            .with_priority(5)
            .mount(&server)
            .await;

        let out =
            probe_openai_compat(&http_client(), &format!("{}/v1", server.uri()), None, "m").await;
        assert!(out.reachable);
        assert_eq!(out.capabilities.streaming, Capability::Supported);
        assert_eq!(out.capabilities.json_schema, Capability::Supported);
        assert_eq!(out.capabilities.tool_calls, Capability::Unsupported);
    }

    #[tokio::test]
    async fn unreachable_server() {
        let out = probe_openai_compat(&http_client(), "http://127.0.0.1:9/v1", None, "m").await;
        assert!(!out.reachable);
        assert!(out.notes[0].starts_with("Basic request failed"));
    }
}
