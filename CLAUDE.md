# CLAUDE.md

Guidance for Claude Code (and other coding agents) working in this repository.

## What this project is

**Project Sunset** is a voice-first personal knowledge tutor. The user learns by talking (or typing)
with an AI teacher. While they talk, the app proposes updates to a personal wiki and concept graph:
articles, typed relationships, citations, and a separate private learning overlay. The user can
approve, reject, edit or undo every change.

## Status

**Phase 0 (skeleton) in progress.** The workspace, crate layout and a Tauri window exist. Still to
do in Phase 0: SQLite schema, generated bindings, `SunsetClient`, mock provider and the mock
proposal loop (`docs/architecture.md` §8).

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
- **The product name and bundle id are temporary.** They are written only in `app-identity.json`
  ("Sunset", `dev.sennett.sunset`). Never hard-code either one anywhere else:
  - Rust gets them through the injected `sunset_domain::AppIdentity` (keychain service, vault
    folder name, etc. are derived from it).
  - TypeScript imports them via `apps/desktop/src/identity.ts`.
  - Files that must contain a literal copy (e.g. `tauri.conf.json`) are targets in
    `scripts/identity.mjs`. Add new ones there, then run `pnpm identity:sync`.
  - The `sunset-*` crate/package names and `Sunset*` type names are internal code names and are
    fine to use. See `docs/architecture.md` §2.7.

## Commands

Run from the repo root.

| Command | What it does |
|---|---|
| `pnpm install` | Install JS dependencies |
| `pnpm dev` | Run the desktop app (Vite + Tauri, hot reload) |
| `pnpm build` | Build the macOS `.app` / `.dmg` |
| `pnpm check` | Identity check, typecheck, `cargo fmt --check`, clippy, tests. **Run before every commit;** there is no automatic CI |
| `pnpm typecheck` | TypeScript only |
| `pnpm identity:sync` | Copy `app-identity.json` into files that need a literal copy |
| `cargo test --workspace` | Rust tests only |

Rust version is pinned in `rust-toolchain.toml`; rustup installs it automatically.

## GitHub

Remote: `https://github.com/sennett-lau/project-sunset` (public). Use the `sennett-lau` account for all
GitHub actions.
