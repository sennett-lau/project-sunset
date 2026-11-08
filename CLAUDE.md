# CLAUDE.md

Guidance for Claude Code (and other coding agents) working in this repository.

## What this project is

**Project Sunset** is a voice-first personal knowledge tutor. The user learns by talking (or typing)
with an AI teacher. While they talk, the app proposes updates to a personal wiki and concept graph:
articles, typed relationships, citations, and a separate private learning overlay. The user can
approve, reject, edit or undo every change.

## Status

Pre-code. No application code exists yet. The next step is **Phase 0** in `docs/architecture.md`.

## Read these first

| Doc | What it is | Authority |
|---|---|---|
| `docs/initial/initial.md` | Original product blueprint (Part I) + technical research (Part II). Long (~1,950 lines); read sections as needed. | Source of truth for **product behavior** |
| `docs/architecture.md` | Desktop-first, local-first technical direction (proposed). | Source of truth for **technology choices**; overrides the initial spec's web/PostgreSQL/FastAPI stack |

## Architecture rules

- **Local-first macOS app** built with **Tauri v2**: Rust backend + React/TypeScript UI. No server
  in v1.
- **Business logic lives in `crates/sunset-core`**, which must not depend on Tauri, axum or any UI.
  `apps/desktop/src-tauri` is a thin adapter. A future `crates/sunset-server` wraps the same core.
- **The UI talks only to the `SunsetClient` TypeScript interface.** Components never call Tauri
  `invoke`/`listen` directly, so a web client can implement the same interface later.
- **Bring your own model; bundle no models.** All AI (LLM, embeddings, STT, TTS, voice, document
  parsing) is reached over HTTP through traits in `crates/sunset-providers`. Do not add model
  weights, ONNX runtimes, or in-process inference.
- **Roles, not one model:** `tutor`, `curator`, `classifier`, `embedder` (later `stt`, `tts`,
  `voice`) are configured separately.
- **Probe capabilities; never assume them.** "OpenAI-compatible" does not imply tool calling or JSON
  schema support. Validate all structured model output with `serde`.
- **The AI never writes the database.** It emits typed proposals; the core validates, applies the
  save policy and commits revisions. Undo is a compensating commit; `commits` is append-only.
- **SQLite (`sqlx`) is the source of truth.** No PostgreSQL, no graph database. The Markdown vault is
  a one-way mirror. Private learning observations never go into shareable articles or exports.
- **IDs are UUIDv7.**
- **Secrets go in the macOS Keychain** (`keyring` crate). Never in SQLite, config files, logs, the
  repo, or the webview.
- **No silent fallback** from a local provider to a cloud provider.
- **The fast path never waits on the slow path:** the tutor reply must not block on curation or
  background jobs.

## Conventions

- Rust: workspace at repo root, crates under `crates/`. `cargo fmt` and `cargo clippy -- -D warnings`
  must pass.
- Frontend: pnpm workspace, TypeScript strict. Use pnpm, not npm or yarn. Do not disable pnpm's
  global virtual store.
- TypeScript types for Rust data are generated into `packages/bindings/`. Never hand-edit them.
- Product terms follow `docs/initial/initial.md` Appendix A (concept, article, proposal, revision,
  personal overlay, source anchor, ...). Table names follow Part II §7.2.
- When a technical decision changes, update `docs/architecture.md` in the same change.

## Commands

None yet. Add build, dev, test and lint commands here when Phase 0 creates them.

## GitHub

Remote: `https://github.com/sennett-lau/project-sunset` (public). Use the `sennett-lau` account for all
GitHub actions.
