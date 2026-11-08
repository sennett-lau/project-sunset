# Connecting models

The app reaches every model over HTTP. Nothing is bundled. Until you connect one, both roles use
the built-in **Demo** provider, which is scripted and only knows "What is a VAE?".

Open **Models** in the header.

## 1. Add a provider

A provider is one OpenAI-compatible endpoint: a base URL that usually ends in `/v1`, plus an
optional API key.

| Server | Base URL | API key |
|---|---|---|
| Ollama on this Mac | `http://localhost:11434/v1` | none |
| LM Studio on this Mac | `http://localhost:1234/v1` | none |
| llama.cpp `llama-server` | `http://localhost:8080/v1` | none, unless started with `--api-key` |
| vLLM / your own GPU box | `http://<host>:8000/v1` | the `--api-key` you set, if any |
| OpenAI | `https://api.openai.com/v1` | required |
| OpenRouter | `https://openrouter.ai/api/v1` | required |

- API keys go to the macOS Keychain (login keychain, service = the app's bundle id). They are
  never written to the database, logs or config, and the app never shows them again.
- A development build is unsigned and its signature changes on every rebuild, so macOS may ask
  "sunset-desktop wants to use your confidential information" the first time a rebuilt app reads
  a key. Choose **Always Allow**.
- Providers on `localhost`, private addresses (192.168.x, 10.x), Tailscale addresses (100.64–127.x,
  `*.ts.net`) or single-word LAN hostnames are labelled **local**. Everything else is **cloud**.

## 2. Pick a model for each role

| Role | Job | Needs |
|---|---|---|
| Tutor | Answers you, streamed | The best model you can run or afford |
| Curator | Turns each exchange into proposed wiki updates, in the background | Reliable JSON output |
| Speech recognition (optional) | Transcribes a recording into a draft | An audio model accepting WAV at `/audio/transcriptions` |

Type the model id (for Ollama, e.g. `qwen3:8b`; the list fills in when the server supports
`GET /v1/models`), then **Use this model**.

## 3. Test connection

For Tutor and Curator, **Test connection** sends four small requests and records what the model supports:

1. a basic request;
2. streaming;
3. native JSON-schema output;
4. tool calls.

Until a model is tested, the app uses the safest path: it streams if it can and puts the JSON
schema in the prompt for the curator. After testing, the curator uses native JSON schema when
supported, otherwise a forced tool call, otherwise the prompt. All output is still validated;
invalid output gets one corrective retry on the same model. If repair fails, a visible capture
error is shown and nothing invalid is saved. Simple probes do not guarantee reliable curation.

## Privacy

- If a role's model fails, you see the error. The app never retries with a different provider.
- The header shows **local** only when every role uses a local provider.

## Example: a model on the Windows PC over Tailscale

The existing Oratype llama.cpp text server can also serve Sunset. Use the PC's Tailscale
address with port `8181` and `/v1`, model `qwen3.5-4b-abliterated`, with no key if the
server is configured without one. Set both Tutor and Curator to this model and test the
connection. The ASR server on port `8180` is a speech service, not a tutor model.

Sunset does not start, stop or install these services. Only connect over your private network.

## Voice input

Add the PC's speech endpoint using port `8180` and `/v1`. Under **Speech recognition**, select
that provider and model `qwen3-asr`, then **Use for speech**. Speech has no Demo assignment and
does not use the chat capability probe. **Disable speech** cancels recording/transcription and
removes the optional assignment; you can then remove its provider if no other role uses it.

Choose **Record a question**, allow microphone access when macOS asks, speak, then stop.
The recording is sent to the selected speech provider and its text is appended to your draft.
Review and correct it before pressing **Send**. The tutor currently replies in text.

Recordings last at most two minutes. **Cancel** stops capture or discards an in-flight transcription;
it cannot retract audio that has already reached the provider. Audio is held in memory and is not
saved in the library. Changing the selected provider does not redirect an active recording.
If access is denied, enable this app under System Settings → Privacy & Security → Microphone.

Microphone capture/permissions still require a physical-device test. Automated checks cover audio
processing and UI lifecycle; live service checks use synthetic recordings, not your microphone.

## Repeat the live text-loop check

This opt-in integration test probes capabilities, streams a real answer, validates a real
proposal, approves it, reopens the temporary library and undoes the change. It does not modify
your regular library or provider settings.

```sh
SUNSET_TEST_BASE_URL=http://<tailscale-host>:8181/v1 \
SUNSET_TEST_MODEL=qwen3.5-4b-abliterated \
cargo test -p sunset-core --test live_model -- --ignored --nocapture
```

To include speech recognition, also set `SUNSET_TEST_STT_URL`, `SUNSET_TEST_STT_MODEL` and
`SUNSET_TEST_WAV` to a synthetic/public WAV fixture. Set `SUNSET_TEST_TRANSCRIPT_CONTAINS` to an
expected phrase to reject an incorrect fixture/transcript before starting the tutor. The lower-level
`cargo test -p sunset-providers --test live_speech -- --ignored --nocapture` checks transcription alone.
