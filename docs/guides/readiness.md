# Development readiness — 2026-11-09

Text learning and reviewed voice input are ready for exploratory testing. The complete voice-first
product in `docs/initial/initial.md` is **not finished**. Lexical retrieval, portability and pending
proposal editing are implemented. TTS/realtime voice, semantic retrieval and source ingestion remain.

## Verified behavior

- `pnpm check` passed: 36 UI tests and 57 Rust tests, type checking, formatting, generated
  bindings and clippy. Post-change Rust tests cover the demo loop,
  real HTTP adapter against fixtures, capability probing, validation, approval/auto-save,
  rejection/undo, restart, alias deduplication, retrieval, mirror recovery and public export.
  New checks cover editing/approval races, immutable proposal history, audio resampling/limits,
  fixed speech destinations, cancellation, stale transcription results and draft retention.
  Saved-article revision tests cover Auto-mode review, stale writes, restart, export and undo.
- The opt-in live test passed against the existing **Qwen3.5 4B** llama.cpp server on sennett-pc:
  capability probes → streamed tutor → real curator proposal → approval → reopen library → undo.
  Both English and Chinese/Cantonese-prompt runs passed. The latter also checked lexical search,
  the actual public-export files, mirror status and search/export removal after undo.
  It used a temporary library and synthetic questions. A successful run delivered first text in
  approximately 110 ms and a proposal in 5.6 seconds. This is one observation, not a benchmark.
- A real-model failure exposed grammar property ordering: alphabetic properties encouraged
  repeated visual changes without creating their concept. The adapter now preserves schema
  ordering and puts operation discriminants first. Semantic errors trigger one repair attempt
  on the same model and surface an error if still invalid.
- Leading reasoning blocks are hidden, including split stream chunks. Empty, interrupted and
  output-limited responses fail instead of being stored as complete answers.
- Sunset's new speech adapter transcribed synthetic Oratype English and Cantonese WAV fixtures
  through the existing **Qwen3-ASR** endpoint. Synthetic VAE questions also passed ASR → tutor →
  curator → approval → retrieval/export → restart → undo, in English and Cantonese. The test can
  require an expected transcription phrase; a successful plumbing run alone is not a quality check.
- Native microphone capture, two-minute limit, cancellation and reviewed transcript insertion are
  implemented. Tests never opened the physical microphone. Permission prompts, input-device capture
  and native visual layout still need device verification.
- At inspection, the RTX 3080 Ti had 12,288 MiB total, 8,309 MiB used and 3,776 MiB free.
  No models were installed and no Oratype services were stopped or reconfigured.
- `pnpm build` produced the macOS `.app` and Apple Silicon `.dmg`.
- The native development executable built and launched with an isolated test bundle identifier.
  Native visual inspection was unavailable because the computer-use tool failed with
  `Sky Computer Use native pipe startup failed`. Automated UI checks do not replace visual QA.

## Original idea audit

| Requirement | Current state |
|---|---|
| FR-01: live spoken learning | Native recording → STT → reviewed draft implemented; physical capture unverified; TTS, interruption and hosted voice missing |
| FR-02: text and conversation history | Text streaming implemented; history/resume controls included in this increment |
| FR-03: teacher-style answers | Real model verified; teaching accuracy and quality still depend on the selected model |
| FR-04: articles and concept links | Proposal-based creation/revision and typed relations implemented |
| FR-05: linked wiki and graph | Markdown wiki, backlinks and focused two-hop graph implemented |
| FR-06: separate personal annotations | Private observations shown separately and excluded from public export |
| FR-07: auto-save or review first | Both implemented |
| FR-08: quiet capture | Proposal badge and non-modal review implemented; active delivery queue deferred |
| FR-09: reject, edit and undo | Pending proposals can be edited with version history, rejected or approved/undone; saved articles can be revised through reviewed, version-checked proposals |
| FR-10: conversation provenance | Evidence turn IDs persisted; navigable citation/provenance UI remains incomplete |
| FR-11: onboarding and editable context | Basic first-run/demo guidance; editable learner profile/preferences still missing |
| FR-12: English and Cantonese | Synthetic ASR-to-knowledge loops passed in both languages; Cantonese style/accuracy varies with the tested 4B model; no physical spoken-conversation benchmark |
| FR-13: optional next topics | Model may suggest them in prose; structured next-topic actions not implemented |
| FR-14: research papers and citations | Not implemented |
| FR-15: webpage sources | Not implemented |
| FR-16: inline diagrams | Typed concept-flow rendering implemented; broader visual generation/rendering incomplete |
| FR-17: interactive charts/animations | Not implemented |
| FR-18: knowledge/history search | Approved article and alias lexical search; no embeddings, source search or full transcript search |
| FR-19: public export | Fresh Markdown snapshot excluding private overlay/transcripts implemented |
| FR-20: import/fork/reconciliation | Not implemented |
| FR-21: flexible topic collections | Freeform conversations work; explicit collections not implemented |

The future web/server tier, synchronization, sharing, MCP/external tools and hosted voice are
also still deferred. The core/client separation remains intact for those additions.

## Start testing

1. Run `pnpm dev`. Your normal library is preserved; tests never delete it.
2. Try the offline VAE demo, approve its proposal, explore Wiki/Graph and undo the change.
3. In **Models**, connect the existing PC text server on port 8181, choose
   `qwen3.5-4b-abliterated`, test it and assign Tutor and Curator. See [Connecting models](providers.md).
4. Ask about a concept, edit its pending proposal and save the edit, then approve the useful updates.
   Check edit history. Start another conversation and ask about it
   again. Try searching by title, an alias and a phrase from the article.
5. Reopen an earlier conversation. Restart the app and check that approved knowledge persists.
6. In **Library**, check mirror status and export public Markdown. Use the **export snapshot**
   for sharing; the full vault contains a private `.private_overlay` folder.
7. In Wiki, choose **Edit article**, change the summary or Markdown and preview it. Choose
   **Review article edit**, then approve it under Proposed updates. Check Open and Undo.
8. Add the speech endpoint on port 8180 under **Models → Speech recognition**, model `qwen3-asr`.
   Record a question, stop, correct the transcript in the draft and Send. Test microphone permission,
   Cancel, retry after a service error, and recording near the two-minute limit. Audio is not saved.

The mirror is one-way. Edits to Markdown do not import into SQLite; they are preserved as
conflicts rather than overwritten. Public export uses current SQLite content, not edited mirror
files. Exported snapshots are independent copies and are not changed by a later undo.

## Remaining limits relevant to testing

- Retrieval is literal lexical matching, not semantic similarity. Common words can retrieve
  weakly relevant pages; long unsegmented Chinese phrases can miss shorter related terms.
- Context uses character budgets rather than model-specific token limits. Very long chats or
  a small context window can still cause provider rejection. The curator sees at most 300 concepts.
- The tested 4B model produced overly long explanations and formal Chinese rather than reliably
  colloquial Cantonese, and some example details were questionable. It can also return malformed
  proposals; repair is bounded and failures remain visible. Review proposed content: a passing
  plumbing test does not establish teaching or language quality.
  Updated tutor instructions encourage concise Cantonese and clarification of ambiguous inputs;
  a later sample was more conversational but still exceeded the requested length and mixed scripts.
- Proposal editing covers text and selected visual labels; full visual structure editing is not exposed.
  Unsaved proposal drafts survive inspector navigation in memory; save them before quitting.
- Saved-article edits create pending proposals even in Auto mode. Submission and approval check the
  original article revision to prevent overwriting newer work. Approval updates exports; undo restores
  the previous article. Manual edits retain their article source without inventing conversation turns.
- An unsigned development binary may prompt for Keychain access when using stored API keys.
- No live cloud model, hosted voice, TTS, microphone permissions or release signing/notarization
  was validated in this increment.

Development changes are recorded in focused local commits. No pushes or pull requests were created.
