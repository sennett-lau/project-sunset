//! Opt-in test using an explicitly selected synthetic/public WAV fixture and ASR endpoint.
//! Never opens the microphone or writes audio to disk.
use sunset_providers::adapters::openai_compat::http_client;
use sunset_providers::speech::{OpenAiSpeechConfig, OpenAiSpeechRecognizer, SpeechRecognizer};

#[tokio::test]
#[ignore = "requires an explicitly selected speech endpoint and WAV fixture"]
async fn live_transcription() {
    let config = OpenAiSpeechConfig {
        base_url: std::env::var("SUNSET_TEST_STT_URL").expect("SUNSET_TEST_STT_URL required"),
        model: std::env::var("SUNSET_TEST_STT_MODEL").expect("SUNSET_TEST_STT_MODEL required"),
        api_key: None,
    };
    let wav =
        std::fs::read(std::env::var("SUNSET_TEST_WAV").expect("SUNSET_TEST_WAV required")).unwrap();
    let transcript = OpenAiSpeechRecognizer::new(http_client(), config)
        .transcribe(wav)
        .await
        .unwrap();
    assert!(!transcript.contains("<asr_text>"));
    if let Ok(expected) = std::env::var("SUNSET_TEST_TRANSCRIPT_CONTAINS") {
        assert!(
            transcript.contains(&expected),
            "transcript did not contain the expected phrase"
        );
    }
    println!("Transcript: {transcript}");
}
