# Project Sunset

A voice-first personal knowledge tutor.

You learn by talking (or typing) with an AI teacher. As the conversation goes on, the app proposes
updates to your own wiki and concept graph: short articles, links between concepts, citations, and
private notes about how you learn. You stay in control: approve, reject, edit, or undo anything.

Think of it as a personal Wikipedia that grows from your conversations.

> **Status:** Phase 1. Connect any OpenAI-compatible model (Ollama, LM Studio, vLLM, OpenAI,
> OpenRouter, ...) under **Models**; see [`docs/guides/providers.md`](docs/guides/providers.md).
> Without one, a built-in demo model answers "What is a VAE?" offline.
>
> "Sunset" is a working name. The product name and bundle id live in
> [`app-identity.json`](app-identity.json) and will change before the first release.

## Goals

- **Learning comes first.** The tutor answers right away. Note-taking happens in the background.
- **You own your knowledge.** Everything is stored locally in SQLite and mirrored to plain Markdown
  files that work with Obsidian and Git.
- **Bring your own model.** Connect any OpenAI-compatible endpoint (Ollama, LM Studio, llama.cpp,
  vLLM, a self-hosted GPU server) or a cloud API. No models are bundled.
- **General knowledge and personal notes stay separate**, so articles can be shared without leaking
  your learning history.
- **Cantonese and English** are the first supported languages, including mixed technical speech.

## Planned architecture

- **macOS desktop app** built with [Tauri v2](https://v2.tauri.app/): a Rust core and a
  React + TypeScript interface.
- **Local-first:** no server needed. A server and web client may come later, reusing the same
  Rust core.
- **Text first, voice next.** The full learning loop is built with typed input before voice is added.

```text
app-identity.json     product name + bundle id (temporary), the only place they are written
crates/
  sunset-domain/      plain data types shared by all crates
  sunset-core/        business logic: proposals, revisions, jobs, events
  sunset-store/       SQLite storage, search, Markdown vault mirror
  sunset-providers/   AI model adapters (OpenAI-compatible, Anthropic, ...)
  sunset-server/      later: HTTP/WebSocket server for a web client
apps/
  desktop/            Tauri app (Rust shell + React entry point)
packages/
  ui/                 shared React UI
  client/             SunsetClient interface: the only way the UI reaches a backend
  bindings/           TypeScript types and commands generated from Rust (pnpm gen:bindings)
```

Details: [`docs/architecture.md`](docs/architecture.md).

## Documents

- [`docs/initial/initial.md`](docs/initial/initial.md): the original product blueprint and
  technical research.
- [`docs/architecture.md`](docs/architecture.md): the current technical direction (proposed).

## Development

Requirements:

- macOS with Xcode Command Line Tools (`xcode-select --install`)
- [rustup](https://rustup.rs/). The Rust version is pinned in `rust-toolchain.toml` and installed
  automatically.
- Node.js 24+ and pnpm 10+

```sh
pnpm install
pnpm dev      # run the desktop app with hot reload
pnpm check    # run before every commit: identity check, typecheck, fmt, clippy, tests
pnpm build    # build the .app and .dmg
```

## License

To be decided. The project is intended to be open source.
