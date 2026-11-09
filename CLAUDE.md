# CLAUDE.md

Guidance for Claude Code (and other coding agents) working in this repository.

## What this project is

**Project Sunset** is a voice-first personal knowledge tutor. The user learns by talking (or typing)
with an AI teacher. While they talk, the app proposes updates to a personal wiki and concept graph:
articles, typed relationships, citations, and a separate private learning overlay. The user can
approve, reject, edit or undo every change.

## Status

**Text learning, proposal editing, Phase 2 lexical retrieval + portability, and native voice input implemented**
(`docs/architecture.md` §8.2–8.3). An
OpenAI-compatible endpoint (Ollama, LM Studio, vLLM, OpenAI, OpenRouter, ...) can serve the
`tutor` and `curator` roles; set them up under **Models** (`docs/guides/providers.md`). Until then
the built-in Demo provider answers "What is a VAE?" offline. An optional `stt` model transcribes
native recordings into a reviewed draft. Embeddings, TTS and hosted/realtime voice remain.
See `docs/guides/readiness.md` for the explicit feature audit and physical-device test gaps.

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
- **Roles, not one model:** `tutor`, `curator`, optional `stt` (later `classifier`, `embedder`, `tts`,
  `voice`) are configured separately.
- **Probe capabilities; never assume them.** "OpenAI-compatible" does not imply tool calling or JSON
  schema support. Validate all structured model output with `serde`.
- **The AI never writes the database.** It emits typed proposals; the core validates, applies the
  save policy and commits revisions. Undo is a compensating commit; `commits` is append-only.
- **SQLite (`sqlx`) is the source of truth.** No PostgreSQL, no graph database. The Markdown vault is
  a one-way mirror. Private learning observations never go into shareable articles or exports.
- **IDs are UUIDv7.**
- **Secrets go in the macOS Keychain** (`SecretStore` in `sunset-providers`, backed by
  `keyring-core` + `apple-native-keyring-store`). Never in SQLite, config files, logs, the repo,
  or sent back to the webview (the settings form may send a key in, never read one out).
- **No silent fallback** from a local provider to a cloud provider.
- **The fast path never waits on the slow path:** the tutor reply must not block on curation or
  background jobs.

- **Structured-output property order matters.** Keep `serde_json/preserve_order` enabled and
  emit union discriminants (`op`, `kind`) before arguments. The llama.cpp live-model check
  exposed repetitive or semantically invalid output with alphabetically ordered properties.
- The vault is derived output, with private notes under `.private_overlay/`. Use the public
  export command to share; never copy the complete vault as a public export. Preserve edited
  and unmanaged mirror files and surface conflicts. Export snapshots always use a new folder.
- Pending proposal edits append immutable snapshots. Approve/reject must carry the revision
  the user reviewed; stale actions fail. Session and evidence turn IDs cannot be edited.
  Manual saved-article edits always await approval, even in Auto mode; their immutable base
  revision must still be current at submission and approval. Never invent conversation evidence.
- Microphone capture starts only on the user's Record action, on a dedicated CPAL worker.
  Keep audio bounded and in memory, pin the speech destination before capture, and discard
  recording/transcription on cancel. Recognized text goes to a draft, never automatically to chat.

## Conventions

- Commit incrementally as each coherent piece is implemented and checked. Keep commits small,
  focused and dependency-ordered; avoid accumulating a whole development phase in one commit.
  Verify the repo's author/committer identity before committing. Local commits do not authorize
  pushing or creating a PR; obtain explicit permission for each publication scope.
- Rust: workspace at repo root, crates under `crates/`. `cargo fmt` and `cargo clippy -- -D warnings`
  must pass.
- Frontend: pnpm workspace, TypeScript strict. Use pnpm, not npm or yarn. Do not disable pnpm's
  global virtual store.
- TypeScript types for Rust data are generated into `packages/bindings/`. Never hand-edit them.
  After changing a Rust type or command that crosses IPC, run `pnpm gen:bindings`; `cargo test`
  fails while the bindings are stale.
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
| `pnpm check` | Identity check, typecheck, UI tests, `cargo fmt --check`, clippy, Rust tests. **Run before every commit;** there is no automatic CI |
| `pnpm typecheck` | TypeScript only |
| `pnpm test` | UI tests only (Vitest) |
| `pnpm gen:bindings` | Regenerate `packages/bindings` from the Rust commands and types |
| `pnpm identity:sync` | Copy `app-identity.json` into files that need a literal copy |
| `cargo test --workspace` | Rust tests only. `crates/sunset-core/tests/phase0_loop.rs` and `phase1_real_provider.rs` are the end-to-end loop tests |

Rust version is pinned in `rust-toolchain.toml`; rustup installs it automatically.

App data in development: `~/Library/Application Support/dev.sennett.sunset/library.sqlite`. Delete
the folder to start from an empty library. API keys are in the login keychain under service
`dev.sennett.sunset`.

The webview does not paint while the window is hidden or covered, so a screenshot of a background
window is blank even when the app works.

## GitHub

Remote: `https://github.com/sennett-lau/project-sunset` (public). Use the `sennett-lau` account for all
GitHub actions.

Opt-in real-model test: set `SUNSET_TEST_BASE_URL` and `SUNSET_TEST_MODEL`, then run
`cargo test -p sunset-core --test live_model -- --ignored --nocapture`. It uses an isolated
temporary library, never the user's data.
