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

Type the model id (for Ollama, e.g. `qwen3:8b`; the list fills in when the server supports
`GET /v1/models`), then **Use this model**.

## 3. Test connection

**Test connection** sends four small requests and records what the model supports:

1. a basic request;
2. streaming;
3. native JSON-schema output;
4. tool calls.

Until a model is tested, the app uses the safest path: it streams if it can and puts the JSON
schema in the prompt for the curator. After testing, the curator uses native JSON schema when
supported, otherwise a forced tool call, otherwise the prompt. All output is still validated;
invalid proposals are dropped, not saved.

## Privacy

- If a role's model fails, you see the error. The app never retries with a different provider.
- The header shows **local** only when every role uses a local provider.

## Example: a model on the Windows PC over Tailscale

Run an OpenAI-compatible server there (Ollama with `OLLAMA_HOST=0.0.0.0`, or vLLM), then add a
provider with base URL `http://100.90.208.26:11434/v1` (Ollama) or `http://100.90.208.26:8000/v1`
(vLLM). It is labelled local because it is on your tailnet.
