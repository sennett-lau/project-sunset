//! One adapter for every server that speaks the OpenAI chat-completions API: Ollama, LM Studio,
//! llama.cpp, vLLM, MLX-LM, OpenRouter, OpenAI and most self-hosted servers.
//!
//! Structured output follows the probed capabilities (docs/architecture.md §4.4): native JSON
//! schema, else a forced tool call, else instructions in the prompt. Callers still validate.

use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{Value, json};

use crate::reasoning::AnswerFilter;
use crate::sse::data_events;
use crate::{
    Capability, ChatCapabilities, ChatDelta, ChatModel, ChatRequest, ChatResponse, ChatRole,
    ChatStream, ProviderError, ResponseFormat, Result,
};

/// A shared HTTP client. Reuse one per app; it pools connections.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(300))
        .user_agent("project-sunset")
        .build()
        .expect("HTTP client")
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatConfig {
    /// Up to and including `/v1`, e.g. `http://localhost:11434/v1`.
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub capabilities: ChatCapabilities,
}

pub struct OpenAiCompatModel {
    client: reqwest::Client,
    config: OpenAiCompatConfig,
    id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonStrategy {
    Schema,
    Tool,
    Prompt,
}

impl OpenAiCompatModel {
    pub fn new(client: reqwest::Client, config: OpenAiCompatConfig) -> Self {
        let id = format!("{}#{}", config.base_url, config.model);
        Self { client, config, id }
    }

    fn json_strategy(&self) -> JsonStrategy {
        let caps = &self.config.capabilities;
        if caps.json_schema == Capability::Supported {
            JsonStrategy::Schema
        } else if caps.tool_calls == Capability::Supported {
            JsonStrategy::Tool
        } else {
            JsonStrategy::Prompt
        }
    }

    fn body(&self, request: &ChatRequest, stream: bool) -> (Value, JsonStrategy) {
        let mut messages: Vec<Value> = request
            .messages
            .iter()
            .map(|m| json!({ "role": role(m.role), "content": m.content }))
            .collect();
        let mut body = json!({ "model": self.config.model, "stream": stream });
        // Bound a runaway generation. A length-limited result is rejected below rather than
        // being saved as a complete answer/proposal.
        body["max_tokens"] = json!(if matches!(request.response_format, ResponseFormat::Text) {
            2048
        } else {
            4096
        });
        if let Some(t) = request.temperature {
            body["temperature"] = json!(t);
        }
        let mut strategy = JsonStrategy::Prompt;
        if let ResponseFormat::Json { name, schema } = &request.response_format {
            let schema = discriminator_first(schema);
            strategy = self.json_strategy();
            match strategy {
                JsonStrategy::Schema => {
                    body["response_format"] = json!({
                        "type": "json_schema",
                        "json_schema": { "name": name, "schema": schema, "strict": false },
                    });
                }
                JsonStrategy::Tool => {
                    body["tools"] = json!([{
                        "type": "function",
                        "function": {
                            "name": name,
                            "description": "Return the result as the arguments of this function.",
                            "parameters": schema,
                        },
                    }]);
                    body["tool_choice"] =
                        json!({ "type": "function", "function": { "name": name } });
                }
                JsonStrategy::Prompt => {
                    let instruction = format!(
                        "Reply with one JSON object that matches this JSON Schema, and nothing else:\n{schema}"
                    );
                    messages.push(json!({ "role": "system", "content": instruction }));
                }
            }
        }
        body["messages"] = Value::Array(messages);
        (body, strategy)
    }

    async fn post(&self, body: &Value) -> Result<reqwest::Response> {
        let mut req = self
            .client
            .post(endpoint(&self.config.base_url, "chat/completions"))
            .json(body);
        if let Some(key) = &self.config.api_key {
            req = req.bearer_auth(key);
        }
        check(req.send().await?).await
    }
}

#[async_trait]
impl ChatModel for OpenAiCompatModel {
    fn model_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &ChatCapabilities {
        &self.config.capabilities
    }

    async fn complete(&self, request: ChatRequest) -> Result<ChatResponse> {
        let is_text = matches!(request.response_format, ResponseFormat::Text);
        let (body, strategy) = self.body(&request, false);
        let response: Value = self.post(&body).await?.json().await?;
        check_finish_reason(&response["choices"][0]["finish_reason"])?;
        let message = &response["choices"][0]["message"];
        if strategy == JsonStrategy::Tool
            && let Some(args) = message["tool_calls"][0]["function"]["arguments"].as_str()
        {
            return Ok(ChatResponse {
                content: args.to_owned(),
                via_tool_call: true,
            });
        }
        match message["content"].as_str() {
            Some(content) => {
                let content = if is_text {
                    let mut filter = AnswerFilter::default();
                    let mut answer = filter.push(content);
                    answer.push_str(&filter.finish());
                    answer
                } else {
                    content.to_owned()
                };
                if content.trim().is_empty() {
                    return Err(ProviderError::BadResponse(
                        "the model returned no answer text".into(),
                    ));
                }
                Ok(ChatResponse {
                    content,
                    via_tool_call: false,
                })
            }
            None => Err(ProviderError::BadResponse(format!(
                "no message content in {}",
                truncate(&response.to_string(), 300)
            ))),
        }
    }

    async fn stream(&self, request: ChatRequest) -> Result<ChatStream> {
        if self.config.capabilities.streaming == Capability::Unsupported {
            let text = self.complete(request).await?.content;
            return Ok(futures::stream::once(async { Ok(ChatDelta { text }) }).boxed());
        }
        let (body, _) = self.body(&request, true);
        let response = self.post(&body).await?;
        let events = data_events(response.bytes_stream()).boxed();
        let deltas = futures::stream::unfold(
            (events, AnswerFilter::default(), false, false, false),
            |(mut events, mut filter, mut finished, mut got_text, mut ended)| async move {
                if ended {
                    return None;
                }
                let output: Result<String> = loop {
                    match events.next().await {
                        Some(Ok(data)) if data.trim() != "[DONE]" => {
                            let event: Value = match serde_json::from_str(&data) {
                                Ok(v) => v,
                                Err(e) => break Err(ProviderError::BadResponse(e.to_string())),
                            };
                            if let Some(err) = event.get("error") {
                                break Err(ProviderError::BadResponse(err.to_string()));
                            }
                            let reason = &event["choices"][0]["finish_reason"];
                            if let Err(e) = check_finish_reason(reason) {
                                break Err(e);
                            }
                            finished |= reason.as_str() == Some("stop");
                            if let Some(text) = event["choices"][0]["delta"]["content"].as_str() {
                                let text = filter.push(text);
                                if !text.is_empty() {
                                    got_text |= !text.trim().is_empty();
                                    break Ok(text);
                                }
                            }
                        }
                        Some(Err(e)) => break Err(e),
                        terminal => {
                            ended = true;
                            finished |= terminal.is_some(); // Explicit [DONE].
                            if !finished {
                                break Err(ProviderError::BadResponse(
                                    "stream ended before the model completed its answer".into(),
                                ));
                            }
                            let tail = filter.finish();
                            got_text |= !tail.trim().is_empty();
                            if !got_text {
                                break Err(ProviderError::BadResponse(
                                    "the model returned no answer text".into(),
                                ));
                            }
                            if tail.is_empty() {
                                return None;
                            }
                            break Ok(tail);
                        }
                    }
                };
                ended |= output.is_err();
                Some((
                    output.map(|text| ChatDelta { text }),
                    (events, filter, finished, got_text, ended),
                ))
            },
        );
        Ok(deltas.boxed())
    }
}

fn check_finish_reason(reason: &Value) -> Result<()> {
    match reason.as_str() {
        Some("length") => Err(ProviderError::BadResponse("the answer exceeded the model's output limit; try a shorter question or a larger context window".into())),
        Some("content_filter") => Err(ProviderError::BadResponse("the provider filtered this answer".into())),
        _ => Ok(()),
    }
}

/// Grammar-based servers generate properties in schema order. Emit the discriminant before
/// variant-specific fields so the model can choose the operation before its arguments.
fn discriminator_first(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let value = if key == "properties" {
                        if let Some(properties) = value.as_object() {
                            let mut ordered = serde_json::Map::new();
                            for tag in ["op", "kind"] {
                                if let Some(value) = properties.get(tag) {
                                    ordered.insert(tag.to_owned(), discriminator_first(value));
                                }
                            }
                            for (key, value) in properties {
                                if !ordered.contains_key(key) {
                                    ordered.insert(key.clone(), discriminator_first(value));
                                }
                            }
                            Value::Object(ordered)
                        } else {
                            discriminator_first(value)
                        }
                    } else {
                        discriminator_first(value)
                    };
                    (key.clone(), value)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(discriminator_first).collect()),
        _ => value.clone(),
    }
}

/// Model ids the server offers (`GET /models`). Not every server implements it.
pub async fn list_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>> {
    let mut req = client.get(endpoint(base_url, "models"));
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }
    let body: Value = check(req.send().await?).await?.json().await?;
    let mut ids: Vec<String> = body["data"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|m| m["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    Ok(ids)
}

fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{path}", base_url.trim_end_matches('/'))
}

fn role(r: ChatRole) -> &'static str {
    match r {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

/// Turn a non-2xx response into an error carrying the server's message.
async fn check(response: reqwest::Response) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let text = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| {
            v["error"]["message"]
                .as_str()
                .or_else(|| v["error"].as_str())
                .map(str::to_owned)
        })
        .unwrap_or(text);
    Err(ProviderError::Status {
        status: status.as_u16(),
        message: truncate(&message, 300).to_owned(),
    })
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChatMessage;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(server: &MockServer, caps: ChatCapabilities) -> OpenAiCompatModel {
        OpenAiCompatModel::new(
            http_client(),
            OpenAiCompatConfig {
                base_url: format!("{}/v1", server.uri()),
                api_key: Some("sk-test".into()),
                model: "test-model".into(),
                capabilities: caps,
            },
        )
    }

    fn json_request() -> ChatRequest {
        ChatRequest {
            messages: vec![ChatMessage::user("2+3?")],
            response_format: ResponseFormat::Json {
                name: "sum".into(),
                schema: json!({ "type": "object" }),
            },
            temperature: None,
        }
    }

    fn reply(content: &str) -> ResponseTemplate {
        ResponseTemplate::new(200)
            .set_body_json(json!({ "choices": [{ "message": { "content": content } }] }))
    }

    #[test]
    fn tagged_union_grammars_choose_the_operation_before_its_fields() {
        let schema = json!({"oneOf": [{"properties": {
            "concept": {"type": "string"},
            "op": {"const": "add_visual"},
            "payload": {"properties": {"edges": {}, "kind": {"const": "concept_flow"}}}
        }}]});
        let ordered = discriminator_first(&schema);
        let properties = ordered["oneOf"][0]["properties"].as_object().unwrap();
        assert_eq!(properties.keys().next().unwrap(), "op");
        assert_eq!(
            properties["payload"]["properties"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap(),
            "kind"
        );
        assert_eq!(ordered, schema, "ordering must not change schema meaning");
    }

    #[tokio::test]
    async fn streams_sse_deltas_with_auth() {
        let server = MockServer::start().await;
        let sse = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n\
                   data: [DONE]\n\n";
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk-test"))
            .and(body_partial_json(
                json!({ "stream": true, "model": "test-model" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
            .mount(&server)
            .await;
        let text: Vec<String> = model(&server, ChatCapabilities::default())
            .stream(ChatRequest {
                messages: vec![ChatMessage::user("hi")],
                ..Default::default()
            })
            .await
            .unwrap()
            .map(|d| d.unwrap().text)
            .collect()
            .await;
        assert_eq!(text.concat(), "Hello");
    }

    #[tokio::test]
    async fn reasoning_is_hidden_and_truncated_streams_fail() {
        for (parts, terminal, expected) in [
            (
                vec!["<thi", "nk>private", "</think>", "答案"],
                "data: [DONE]\n\n",
                Some("答案"),
            ),
            (vec!["partial answer"], "", None),
            (vec!["<think>reasoning only"], "data: [DONE]\n\n", None),
            (vec![], "data: [DONE]\n\n", None),
            (
                vec!["partial answer"],
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
                None,
            ),
            (
                vec!["complete answer"],
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                Some("complete answer"),
            ),
        ] {
            let server = MockServer::start().await;
            let mut body = String::new();
            for text in parts {
                body.push_str(&format!(
                    "data: {}\n\n",
                    json!({"choices": [{"delta": {"content": text}}]})
                ));
            }
            body.push_str(terminal);
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
                .mount(&server)
                .await;
            let chunks: Vec<_> = model(&server, ChatCapabilities::default())
                .stream(ChatRequest::default())
                .await
                .unwrap()
                .collect()
                .await;
            if let Some(expected) = expected {
                assert_eq!(
                    chunks
                        .into_iter()
                        .map(|d| d.unwrap().text)
                        .collect::<String>(),
                    expected
                );
            } else {
                assert!(
                    chunks.iter().any(Result::is_err),
                    "must reject incomplete/empty output: {chunks:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn nonstreaming_hides_reasoning_and_rejects_empty_answers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(reply("<think>private</think>Hello"))
            .mount(&server)
            .await;
        let caps = ChatCapabilities {
            streaming: Capability::Unsupported,
            ..Default::default()
        };
        let chunks: Vec<_> = model(&server, caps)
            .stream(ChatRequest::default())
            .await
            .unwrap()
            .collect()
            .await;
        assert_eq!(chunks[0].as_ref().unwrap().text, "Hello");
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(reply(""))
            .mount(&server)
            .await;
        assert!(
            model(&server, ChatCapabilities::default())
                .complete(ChatRequest::default())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn uses_json_schema_when_supported() {
        let server = MockServer::start().await;
        Mock::given(body_partial_json(
            json!({ "response_format": { "type": "json_schema" } }),
        ))
        .respond_with(reply("{\"sum\":5}"))
        .mount(&server)
        .await;
        let caps = ChatCapabilities {
            json_schema: Capability::Supported,
            ..Default::default()
        };
        let out = model(&server, caps).complete(json_request()).await.unwrap();
        assert_eq!(out.content, "{\"sum\":5}");
    }

    #[tokio::test]
    async fn falls_back_to_a_forced_tool_call() {
        let server = MockServer::start().await;
        Mock::given(body_partial_json(
            json!({ "tool_choice": { "function": { "name": "sum" } } }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "content": null, "tool_calls": [
                { "function": { "name": "sum", "arguments": "{\"sum\":5}" } }
            ] } }]
        })))
        .mount(&server)
        .await;
        let caps = ChatCapabilities {
            json_schema: Capability::Unsupported,
            tool_calls: Capability::Supported,
            ..Default::default()
        };
        let out = model(&server, caps).complete(json_request()).await.unwrap();
        assert!(out.via_tool_call);
        assert_eq!(out.content, "{\"sum\":5}");
    }

    #[tokio::test]
    async fn untested_models_get_the_schema_in_the_prompt() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(reply("ok"))
            .mount(&server)
            .await;
        model(&server, ChatCapabilities::default())
            .complete(json_request())
            .await
            .unwrap();
        let sent: Value = server.received_requests().await.unwrap()[0]
            .body_json()
            .unwrap();
        assert!(sent.get("response_format").is_none() && sent.get("tools").is_none());
        let last = sent["messages"].as_array().unwrap().last().unwrap().clone();
        assert!(last["content"].as_str().unwrap().contains("JSON Schema"));
    }

    #[tokio::test]
    async fn reports_http_errors_with_the_server_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(json!({ "error": { "message": "Incorrect API key" } })),
            )
            .mount(&server)
            .await;
        let err = model(&server, ChatCapabilities::default())
            .complete(json_request())
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Status { status: 401, .. }));
        assert!(err.to_string().contains("API key was rejected"));
        assert!(err.to_string().contains("Incorrect API key"));
    }

    #[tokio::test]
    async fn lists_models() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{ "id": "qwen3:8b" }, { "id": "llama3.2" }]
            })))
            .mount(&server)
            .await;
        let ids = list_models(&http_client(), &format!("{}/v1/", server.uri()), None)
            .await
            .unwrap();
        assert_eq!(ids, ["llama3.2", "qwen3:8b"]);
    }
}
