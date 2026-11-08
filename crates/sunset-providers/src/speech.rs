//! Speech recognition through the same user-configured HTTP endpoints as other models.
//! Audio stays in memory; this adapter never persists recordings or retries another provider.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use crate::{ProviderError, Result};

pub const MAX_AUDIO_BYTES: usize = 24 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 128 * 1024;

#[async_trait]
pub trait SpeechRecognizer: Send + Sync {
    fn model_id(&self) -> &str;
    async fn transcribe(&self, wav: Vec<u8>) -> Result<String>;
}

/// A separate speech model assignment; chat capabilities are not assumed to imply audio support.
pub struct OpenAiSpeechConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

pub struct OpenAiSpeechRecognizer {
    client: reqwest::Client,
    config: OpenAiSpeechConfig,
    id: String,
}

impl OpenAiSpeechRecognizer {
    pub fn new(client: reqwest::Client, config: OpenAiSpeechConfig) -> Self {
        let id = format!("{}#{}", config.base_url, config.model);
        Self { client, config, id }
    }
}

#[async_trait]
impl SpeechRecognizer for OpenAiSpeechRecognizer {
    fn model_id(&self) -> &str {
        &self.id
    }

    async fn transcribe(&self, wav: Vec<u8>) -> Result<String> {
        if wav.len() > MAX_AUDIO_BYTES {
            return Err(ProviderError::BadResponse("recording is too large".into()));
        }
        if wav.len() <= 44 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
            return Err(ProviderError::BadResponse(
                "expected a nonempty WAV recording".into(),
            ));
        }
        let file = Part::bytes(wav)
            .file_name("recording.wav")
            .mime_str("audio/wav")?;
        let form = Form::new()
            .text("model", self.config.model.clone())
            .text("response_format", "json")
            .part("file", file);
        let mut request = self
            .client
            .post(format!(
                "{}/audio/transcriptions",
                self.config.base_url.trim_end_matches('/')
            ))
            .timeout(Duration::from_secs(120))
            .multipart(form);
        if let Some(key) = &self.config.api_key {
            request = request.bearer_auth(key);
        }
        let mut response = request.send().await?;
        if !response.status().is_success() {
            // Error bodies may echo recordings, transcripts or credentials. Keep them out of UI/logs.
            return Err(ProviderError::Status {
                status: response.status().as_u16(),
                message: "speech transcription failed".into(),
            });
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(ProviderError::BadResponse(
                    "transcription response is too large".into(),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        #[derive(Deserialize)]
        struct Transcript {
            text: String,
        }
        let transcript: Transcript = serde_json::from_slice(&body).map_err(|_| {
            ProviderError::BadResponse("expected a transcription JSON object with text".into())
        })?;
        let text = normalize_transcript(&transcript.text);
        if text.is_empty() {
            return Err(ProviderError::BadResponse(
                "no speech was recognized; try recording again".into(),
            ));
        }
        Ok(text.to_owned())
    }
}

/// llama.cpp's Qwen3-ASR response includes a language prefix. Strip only its leading header,
/// never marker-looking words embedded in the actual dictation.
fn normalize_transcript(text: &str) -> &str {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("language ")
        && let Some((language, transcript)) = rest.split_once("<asr_text>")
        && !language.is_empty()
        && language.len() <= 48
        && language
            .chars()
            .all(|c| c.is_alphabetic() || c == ' ' || c == '-')
    {
        return transcript.trim();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn wav() -> Vec<u8> {
        let mut bytes = vec![0; 46];
        bytes[..4].copy_from_slice(b"RIFF");
        bytes[8..12].copy_from_slice(b"WAVE");
        bytes
    }

    fn recognizer(server: &MockServer) -> OpenAiSpeechRecognizer {
        OpenAiSpeechRecognizer::new(
            crate::adapters::openai_compat::http_client(),
            OpenAiSpeechConfig {
                base_url: format!("{}/v1/", server.uri()),
                api_key: Some("test-only-key".into()),
                model: "qwen3-asr".into(),
            },
        )
    }

    #[tokio::test]
    async fn uploads_wav_to_selected_model_and_cleans_qwen_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/transcriptions"))
            .and(header("authorization", "Bearer test-only-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "text": "language Cantonese<asr_text>聽日下晝四點開會。"
            })))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            recognizer(&server).transcribe(wav()).await.unwrap(),
            "聽日下晝四點開會。"
        );
        let requests = server.received_requests().await.unwrap();
        let body = String::from_utf8_lossy(&requests[0].body);
        assert!(body.contains("name=\"model\"\r\n\r\nqwen3-asr"));
        assert!(body.contains("filename=\"recording.wav\""));
        assert!(body.contains("Content-Type: audio/wav"));
        assert!(body.contains("name=\"response_format\"\r\n\r\njson"));
    }

    #[tokio::test]
    async fn rejects_invalid_audio_without_uploading_it() {
        let server = MockServer::start().await;
        for bytes in [vec![], vec![1; 100], vec![0; MAX_AUDIO_BYTES + 1]] {
            assert!(recognizer(&server).transcribe(bytes).await.is_err());
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_empty_malformed_and_unbounded_responses() {
        for body in [
            "{}".to_owned(),
            "{\"text\":\" \"}".into(),
            "{\"text\":7}".into(),
            "{\"text\":\"language None<asr_text>\"}".into(),
            "x".repeat(MAX_RESPONSE_BYTES + 1),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_string(body))
                .mount(&server)
                .await;
            assert!(matches!(
                recognizer(&server).transcribe(wav()).await,
                Err(ProviderError::BadResponse(_))
            ));
        }
    }

    #[tokio::test]
    async fn errors_do_not_echo_server_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_string("private transcript"))
            .mount(&server)
            .await;
        let error = recognizer(&server)
            .transcribe(wav())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("401"));
        assert!(!error.contains("private transcript"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[test]
    fn preserves_transcript_marker_literals_and_regular_json_text() {
        assert_eq!(normalize_transcript(" hello "), "hello");
        assert_eq!(
            normalize_transcript("Say language English<asr_text> hello"),
            "Say language English<asr_text> hello"
        );
        assert_eq!(
            normalize_transcript("language English<asr_text>Say <asr_text>"),
            "Say <asr_text>"
        );
    }
}
