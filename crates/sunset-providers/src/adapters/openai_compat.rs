//! One adapter for every server that speaks the OpenAI chat-completions API: Ollama, LM Studio,
//! llama.cpp, vLLM, MLX-LM, OpenRouter, OpenAI and most self-hosted servers.
//!
//! Structured output follows the probed capabilities (docs/architecture.md §4.4): native JSON
//! schema, else a forced tool call, else instructions in the prompt. Callers still validate.

use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{Value, json};

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
        if let Some(t) = request.temperature {
            body["temperature"] = json!(t);
        }
        let mut strategy = JsonStrategy::Prompt;
        if let ResponseFormat::Json { name, schema } = &request.response_format {
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
        let (body, strategy) = self.body(&request, false);
        let response: Value = self.post(&body).await?.json().await?;
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
            Some(content) => Ok(ChatResponse {
                content: content.to_owned(),
                via_tool_call: false,
            }),
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
        let deltas = data_events(response.bytes_stream())
            .take_while(|data| {
                let done = matches!(data, Ok(d) if d.trim() == "[DONE]");
                async move { !done }
            })
            .filter_map(|data| async move {
                let data = match data {
                    Ok(d) => d,
                    Err(e) => return Some(Err(e)),
                };
                let event: Value = match serde_json::from_str(&data) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(ProviderError::BadResponse(e.to_string()))),
                };
                if let Some(err) = event.get("error") {
                    return Some(Err(ProviderError::BadResponse(err.to_string())));
                }
                event["choices"][0]["delta"]["content"]
                    .as_str()
                    .filter(|t| !t.is_empty())
                    .map(|t| Ok(ChatDelta { text: t.to_owned() }))
            });
        Ok(deltas.boxed())
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
