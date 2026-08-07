# Project: `decks` — a local-first AI DJ assistant for Rekordbox 7

You are Claude Code. You are the sole engineer on this project, working autonomously across many sessions. This document is your contract, your spec, and your memory. Read it in full at the start of every session before doing anything else.

-----

## 0. How to work

### Operating principles

1. **Plan before you build, build before you polish.** Every session starts by reading this file, reading `STATUS.md`, reading the last 3 entries in `JOURNAL.md`, then writing a short plan for the session in `JOURNAL.md` *before* writing code.
1. **Small, reviewable commits.** One logical change per commit. Conventional Commits format (`feat:`, `fix:`, `refactor:`, `chore:`, `docs:`, `test:`). Never commit broken builds to `main`.
1. **Tests are not optional.** Every Rust module gets unit tests. Every TS module gets vitest tests. Every IPC command gets an integration test. CI must stay green.
1. **No mocks in production paths.** If a feature isn’t real, it isn’t checked in. Stub UI is fine; stub logic that returns fake data is not — gate the UI behind a feature flag instead.
1. **Read-only access to the user’s Rekordbox DB is sacred.** No code path may open `master.db` with write flags. There is one exception: the explicit, opt-in “live patch” feature in Phase 6, which lives behind its own crate and a runtime flag. Until then, all writes go through XML export.
1. **Privacy-first.** No telemetry. No analytics. No remote logging. The user’s library never leaves their machine except via the enrichment APIs they explicitly enable, and those calls go through a local cache first.
1. **When stuck, stop and write.** If you can’t figure something out in 30 minutes, write a `DECISION.md` entry stating the problem, the options, and what you’d ask a senior engineer. Then pick the most reversible option and proceed.
1. **Update docs in the same commit as the code.** Drift is the enemy.

### Session workflow (do this every time)

```
1. cat CLAUDE_CODE_PROMPT.md            # this file
2. cat STATUS.md                        # current phase, next task
3. tail -n 200 JOURNAL.md               # what past-you was thinking
4. git status && git log --oneline -20  # where the tree is
5. cargo test && pnpm test              # confirm green
6. Write a session plan into JOURNAL.md (date, goals, ~3 bullets)
7. Work the plan. Update STATUS.md as tasks complete.
8. End-of-session entry in JOURNAL.md: what shipped, what's next, blockers.
9. git push
```

### Definition of done (per task)

A task is done when **all** of these are true:

- Code is written and reviewed by you against this spec.
- Unit tests cover the happy path and at least one error path.
- The feature is reachable from the UI (or, for backend tasks, from a real caller — never just tests).
- Docs in `/docs` updated.
- `STATUS.md` updated.
- Conventional commit pushed.
- `cargo test`, `cargo clippy -- -D warnings`, `pnpm test`, `pnpm typecheck`, `pnpm lint` all pass.

### Files you maintain

- `CLAUDE_CODE_PROMPT.md` — this file. Edit only to clarify ambiguities or record scope changes; treat changes as careful diffs, not rewrites.
- `STATUS.md` — single source of truth for phase, current task, recent completions, blockers. Always current.
- `JOURNAL.md` — append-only log of session plans, decisions, and end-of-session summaries.
- `DECISIONS.md` — ADR-lite. One entry per non-obvious technical choice.
- `docs/architecture.md`, `docs/data-model.md`, `docs/tools.md`, `docs/ui.md` — living reference docs.

-----

## 1. What we’re building

**`decks`** is a native desktop app (macOS first, Windows second, Linux best-effort) that gives a DJ a beautiful, fast, AI-assisted interface to their Rekordbox 7 library. It does everything `reklawdbox` does — collection audit, metadata backfill, genre classification, library health, set building, pool building, batch import — and adds:

1. A real **GUI** with library browser, waveform preview, drag-and-drop set timeline, and inline diff review of staged changes.
1. **Audio embedding similarity search** (CLAP, local) — “tracks that sound like this,” and natural-language queries like “dark hypnotic dub-techno around 130”.
1. **Local-first model routing** — Claude/GPT in the cloud for hard reasoning, Ollama-hosted local models for cheap calls.
1. **A learned transition ranker** that personalizes to the user’s accept/reject history.
1. **A user-extensible plugin tool system** so users (or the agent) can register Python/JS scripts as agent tools without a release.
1. **Cross-platform**.

The architecture is: **Tauri 2 desktop shell, Rust core, TypeScript/React frontend, agent orchestrator in TS, all local-first.** All proposed library changes stage in memory and only land in Rekordbox via XML import (Phase 6 adds optional direct-DB patching behind a flag).

The original `reklawdbox` (MIT-licensed) is the reference implementation for the read-only DB layer, XML format, audio analysis pipeline, enrichment, scoring, and SOPs. We **vendor or fork** the relevant Rust modules from it rather than rewrite. Attribution preserved.

-----

## 2. Reference: reklawdbox

Repo: `https://github.com/ryan-voitiskis/reklawdbox` (MIT)

What we keep, mostly as-is:

|reklawdbox source                                                                      |We use it as                                                 |
|---------------------------------------------------------------------------------------|-------------------------------------------------------------|
|`src/db.rs`                                                                            |`crates/rekordbox-db` — SQLCipher reader, schema, queries    |
|`src/xml.rs`                                                                           |`crates/rekordbox-xml` — Rekordbox XML import format         |
|`src/audio.rs`, `src/audio_profile.rs`, `stratum-dsp/`                                 |`crates/audio-analysis` — BPM, key, features                 |
|`src/tags.rs`                                                                          |`crates/audio-tags` — read/write ID3, MP4, FLAC tags         |
|`src/discogs.rs`, `src/beatport.rs`, `src/bandcamp.rs`, `src/musicbrainz.rs`, `broker/`|`crates/enrichment` + Cloudflare Worker broker               |
|`src/classify.rs`, `src/genre.rs`                                                      |`crates/classify` — genre decision tree                      |
|`src/changes.rs`                                                                       |`crates/changes` — staged ChangeManager                      |
|`src/store.rs`                                                                         |`crates/cache` — local SQLite cache                          |
|`src/tools/scoring.rs`, `src/tools/sequencing_handlers.rs`                             |`crates/scoring` — transition scoring + beam-search sequencer|
|`docs/rekordbox-internals.md`, `docs/rekordbox-gotchas.md`                             |`docs/rekordbox-internals.md` (vendored, kept current)       |
|`site/src/partials/sops/*.mdx`                                                         |`docs/sops/*.mdx` — embedded into binary like upstream       |

What we replace:

|reklawdbox piece             |Our replacement                                                               |
|-----------------------------|------------------------------------------------------------------------------|
|`src/tools/mod.rs` MCP server|Tauri IPC commands + a slim MCP shim for external clients                     |
|Help-handler / chat-only UX  |Native React UI                                                               |
|`src/cli/`                   |Same subcommands, but as a separate `decks-cli` binary calling the same crates|

What we add (new crates):

- `crates/embeddings` — CLAP audio + text embeddings, ONNX runtime, vector search via sqlite-vec.
- `crates/agent` — model-agnostic agent loop with tool dispatch, streaming, and replay (TS, in `apps/desktop/src/agent`).
- `crates/ranker` — learned pairwise transition ranker (gradient-boosted; train offline, infer online).
- `crates/plugins` — sandboxed user plugin host.

-----

## 3. Repo layout

```
decks/
├── CLAUDE_CODE_PROMPT.md
├── STATUS.md
├── JOURNAL.md
├── DECISIONS.md
├── README.md
├── LICENSE                       # GPL-3.0-or-later, attribution to reklawdbox
├── NOTICE                        # third-party attributions
├── Cargo.toml                    # workspace
├── pnpm-workspace.yaml
├── package.json
├── rust-toolchain.toml           # pin stable
├── .github/workflows/            # ci.yml, release.yml
│
├── crates/
│   ├── rekordbox-db/             # read-only SQLCipher access
│   ├── rekordbox-xml/            # XML import format read/write
│   ├── audio-analysis/           # stratum-dsp + Essentia bridge
│   ├── audio-tags/
│   ├── enrichment/               # Discogs/MB/Beatport/Bandcamp clients
│   ├── classify/                 # genre decision tree
│   ├── scoring/                  # transition scoring, beam search
│   ├── changes/                  # staged ChangeManager
│   ├── cache/                    # SQLite cache (WAL)
│   ├── embeddings/               # CLAP, sqlite-vec
│   ├── prodjlink/                # Pioneer PRO DJ LINK listener
│   ├── ranker/                   # learned transition ranker
│   ├── plugins/                  # user plugin host
│   └── decks-core/               # facade: re-exports + high-level workflows
│
├── apps/
│   ├── desktop/                  # Tauri app (Rust shell + React frontend)
│   │   ├── src-tauri/
│   │   └── src/                  # React, TS
│   └── cli/                      # decks-cli binary
│
├── broker/                       # Cloudflare Worker for Discogs OAuth (vendored)
│
├── docs/
│   ├── architecture.md
│   ├── data-model.md
│   ├── tools.md
│   ├── ui.md
│   ├── rekordbox-internals.md
│   ├── sops/                     # workflow SOPs (mdx)
│   └── adr/                      # architecture decision records
│
├── scripts/
│   ├── dev.sh                    # bring up dev environment
│   ├── seed-test-library.sh      # build a fixture library for tests
│   └── release.sh
│
└── fixtures/
    ├── tiny-library/             # small synthetic master.db + 10 tracks
    └── audio/                    # CC0/public-domain audio for tests
```

-----

## 4. Tech stack — locked

These are decisions, not options. Don’t second-guess them without writing a `DECISIONS.md` ADR first.

- **Rust:** stable, edition 2021. Workspace.
- **Tauri:** v2.x.
- **Frontend:** React 18, TypeScript, Vite, Tailwind, Radix UI primitives, Zustand for state, TanStack Query for async.
- **Audio in browser:** WebAudio + WaveSurfer.js for waveform UI; native preview via Tauri (rodio).
- **Audio embeddings:** [LAION CLAP](https://github.com/LAION-AI/CLAP) via ONNX Runtime in Rust (`ort` crate). Model: `music_audioset_epoch_15_esc_90.14.pt` exported to ONNX. 512-dim shared text+audio space.
- **Vector search:** [sqlite-vec](https://github.com/asg017/sqlite-vec) extension loaded into our cache DB.
- **DB encryption:** SQLCipher via `rusqlite` with the `bundled-sqlcipher` feature. Key derivation per reklawdbox/pyrekordbox.
- **PRO DJ LINK:** port `prolink-connect`’s wire format to Rust; UDP listener on ports 50000/50001/50002.
- **Agent SDKs:** `@anthropic-ai/sdk` (primary), `openai` (compat), `ollama` (local). Stream tool calls.
- **Ranker training:** Python `lightgbm` in a sidecar script, called from the CLI; inference in Rust via `lightgbm3` or just reading the model JSON.
- **Plugin host:** Deno (sandboxed JS/TS) for the default; allow-list IPC with the Rust core. Optional Python via subprocess for power users.
- **Package manager:** pnpm.
- **Linters:** `cargo clippy -- -D warnings`, `cargo fmt`, `eslint`, `prettier`, `typescript --noEmit`.
- **Tests:** `cargo test`, `vitest`, `playwright` for end-to-end UI.

-----

## 5. Phases & milestones

You ship in phases. Each phase ends with a tag (`v0.1.0`, `v0.2.0`, …) and a short release note in `docs/releases/`. **Do not start phase N+1 until phase N is tagged and the demo path works on a clean install.**

### Phase 1 — foundations & read-only library (target: v0.1.0)

Goal: a Tauri app that opens the user’s Rekordbox library and shows it.

- [ ] Repo scaffold per §3.
- [ ] CI: lint, test, build matrix (macOS, Windows).
- [ ] `crates/rekordbox-db`: SQLCipher key derivation; open `master.db` read-only; query tracks, playlists, hot cues, beat grid; integration test against `fixtures/tiny-library/`.
- [ ] `crates/rekordbox-xml`: parse and emit Rekordbox XML; round-trip property tests.
- [ ] `crates/cache`: SQLite WAL store; schema versioning; load sqlite-vec extension.
- [ ] `apps/desktop`: Tauri 2 scaffold, React + Vite + Tailwind. First-run wizard locates `master.db` and validates.
- [ ] Library browser UI: virtualized track table (TanStack Table + virtualizer), filterable, sortable.
- [ ] Track detail panel: tags, cues, waveform (read ANLZ; render via WaveSurfer).
- [ ] Audio preview: spacebar to play/pause selected track, scrub on waveform.
- [ ] Settings: theme, library path, model API keys (stored via OS keychain through `keyring` crate).
- [ ] **Demo:** open the app, see your library, click a track, hear it.

### Phase 2 — agent loop & MCP-equivalent toolset (target: v0.2.0)

Goal: the agent can do everything reklawdbox does, but from a chat panel inside the app.

- [ ] `crates/changes`: staged ChangeManager; diff types for tag edits, playlist mutations, etc.
- [ ] `crates/audio-tags`, `crates/audio-analysis`, `crates/enrichment`, `crates/classify`, `crates/scoring`: vendor from reklawdbox; bring tests over; add coverage where it’s thin.
- [ ] Tauri IPC commands mirroring reklawdbox’s MCP tool surface (~50 tools). Source of truth: `docs/tools.md`. Each tool is a typed Rust function plus a typed TS wrapper.
- [ ] `apps/desktop/src/agent`: agent orchestrator. Streams tool calls. Renders each tool call inline (track lists as clickable lists, diffs as diff views, charts as charts). Conversation persisted to `~/.local/share/decks/conversations.db`.
- [ ] Model picker: Claude (default), OpenAI, Ollama. Stored per-conversation.
- [ ] Workflow SOPs: vendor reklawdbox’s SOPs as MDX, render them as guided checklists in the UI; the user can launch a workflow and the agent follows it.
- [ ] Inline diff view: every staged change shows old → new; user accepts/rejects per-row or in bulk.
- [ ] Export: produce Rekordbox-importable XML; show a “click here, then File → Import Collection in Rekordbox” instruction.
- [ ] MCP shim binary `decks-mcp`: exposes the same tools over MCP stdio for users who still want Claude Desktop / Claude Code on the side.
- [ ] **Demo:** ask the agent “audit my collection for naming violations,” watch it work, review the diff, export, reimport in Rekordbox.

### Phase 3 — visual set builder (target: v0.3.0)

Goal: drag-and-drop set construction with the agent as a copilot.

- [ ] Set timeline UI: horizontal track of clips, each clip showing waveform thumbnail, BPM, key, energy curve.
- [ ] Drag tracks from library into the timeline; reorder; remove.
- [ ] Side panel “next track” suggestions ranked by transition score; each suggestion shows the score breakdown (key compat, energy delta, timbre similarity, BPM delta) and a one-sentence reason from the agent.
- [ ] Click “preview transition” to hear the last 30s of A crossfaded into the first 30s of B.
- [ ] Save/load sets; export set as Rekordbox playlist via XML.
- [ ] Energy curve target: user draws an energy arc; the agent fills in tracks that match.
- [ ] **Demo:** build a 90-minute set in 10 minutes, end-to-end.

### Phase 4 — embeddings & similarity (target: v0.4.0)

Goal: real audio similarity, no genre tags required.

- [ ] `crates/embeddings`: load CLAP ONNX model; chunk audio (10s windows, mean-pooled); produce 512-d vector per track. Cache per (track-uri, model-version, chunking-config).
- [ ] Background indexer: traverses the library, embeds new/changed tracks, stores vectors in cache DB via sqlite-vec. Pause/resume; throttled.
- [ ] Tools: `find_similar(track_id, n)`, `text_to_tracks(query, n)`, `cluster_library(k)`. Surface in UI as a “more like this” button on every track and a “natural-language search” input.
- [ ] Pool builder upgrade: pools can be seeded by similarity, not just by metadata filters.
- [ ] **Demo:** “find me 30 dark, hypnotic, dub-influenced techno tracks around 130 BPM” → real results, no genre tag dependency.

### Phase 5 — learned ranker, plugins, polish (target: v1.0.0)

- [ ] `crates/ranker`: feature extraction from track-pair (BPM ratio, key Camelot distance, energy delta, embedding cosine, time-of-day, recent-skip count, etc.). Pairwise LightGBM (LambdaRank). Training script reads accept/reject log; inference in Rust.
- [ ] Ranker A/B: user can compare hand-tuned scoring vs. learned ranker side-by-side.
- [ ] `crates/plugins`: Deno-sandboxed plugin host. Plugin manifest declares allowed tool calls. Plugin tools appear in the agent’s tool list with a clear “user plugin” tag.
- [ ] Onboarding pass: first-run wizard polished, tooltips, empty states, error recovery.
- [ ] Performance: library views must stay smooth at 50k tracks. Profile and fix.
- [ ] Optional “live patch” mode: explicit, opt-in, behind a feature flag, off by default. Allows patching `master.db` while Rekordbox is closed. Disabled until extensive testing on disposable libraries proves safe. Even when on, every patch is preceded by a backup.
- [ ] Cross-platform: Windows fully supported. Linux best-effort (ships, but flagged experimental).
- [ ] **v1.0.0 demo:** a user installs `decks`, opens their library, builds a set with the agent, plays it on real CDJs with live suggestions, and at no point feels like they’re talking to a chatbot.

### Beyond 1.0 (don’t start, but design for)

- Mobile companion app for set review on the go.
- Collaborative sets (multi-DJ).
- iCloud / Dropbox sync of the changes log so the same library can be edited from two machines.
- Streaming-platform integrations (Beatport DJ, SoundCloud Go+).

-----

## 6. Tool inventory

Authoritative list lives in `docs/tools.md`. Each tool has: name, summary, parameters (typed), return type, idempotency, side effects, cost class (free / network / model-call). Mirror reklawdbox’s tools 1-for-1 in Phase 2; new tools added in later phases must be appended to this list with the same rigor.

Tool categories (initial):

- **Library** — search, get-track, list-playlists, list-cues.
- **Audio** — analyze, get-waveform, get-features.
- **Enrichment** — discogs-lookup, mb-lookup, beatport-lookup, bandcamp-lookup.
- **Classify** — genre-classify, genre-audit.
- **Health** — orphan-scan, duplicate-scan, broken-link-scan.
- **Sets** — score-transition, sequence-set, plan-chapters.
- **Pools** — find-pool, expand-pool.
- **Staging** — list-changes, accept-change, reject-change, export-xml.
- **Embeddings (Phase 4)** — find-similar, text-to-tracks, cluster.
- **Plugins (Phase 5)** — register-plugin, list-plugins, call-plugin.

-----

## 7. UI principles

- **Three-pane default**: library (left) / focus (center) / agent chat (right). Each collapsible. Remember layout per workspace.
- **Keyboard-first**: spacebar = play/pause, J/K/L = scrub, Cmd+K = command palette, Cmd+/ = open chat, arrows for selection.
- **Diffs everywhere**: any agent-proposed change appears as a diff. Red/green, accept/reject, in bulk or per-row. The user must never feel like they don’t know what the agent did.
- **Don’t hide the agent’s reasoning, but don’t drown the user in it either.** Each tool call is collapsible. Default expanded for the most recent, collapsed for older ones.
- **Latency budgets**: any UI interaction < 100 ms. Library queries < 250 ms. Tool calls show a streaming indicator within 200 ms.
- **No emoji in UI chrome.** Tabler icons only.
- **Light + dark from day one.** No “we’ll add dark mode later.”
- **Empty states are designed, not stubbed.** Every empty list has a useful next action.
- **Errors are friendly.** Show what happened, what to do, and a “copy details” button. No raw stack traces in the user’s face.

-----

## 8. Testing strategy

- **Unit tests** for every Rust module and TS module. Aim for ≥80% line coverage on `crates/*` and `apps/desktop/src/agent`.
- **Property tests** (proptest in Rust, fast-check in TS) for: XML round-trip, scoring monotonicity, tag normalization, key/Camelot conversions.
- **Integration tests** against `fixtures/tiny-library/` — a small synthetic master.db with 10 tracks of CC0 audio. This fixture is checked in and is the canonical test corpus.
- **End-to-end tests** with Playwright on a fixture library: launch app, open library, run a workflow, accept changes, export XML, validate output.
- **Snapshot tests** for the agent’s tool-call rendering (each tool kind has a snapshot of its UI).
- **Manual test plan** in `docs/manual-test-plan.md`, run before every release tag.
- **No flaky tests.** A flaky test is a broken test; quarantine it in a `flaky/` dir and fix within the same week or delete.

-----

## 9. Security & safety

- API keys live in the OS keychain (`keyring` crate). Never in plaintext config.
- The Rekordbox DB is opened `READ_ONLY` until Phase 6’s opt-in flag. Add a unit test that fails the build if any code path opens it otherwise.
- Plugins run in a Deno sandbox with no network and no FS access by default. Explicit allow-list per plugin, surfaced in UI.
- All enrichment HTTP requests go through a single client with timeouts, retries, and rate-limiting per host.
- Logs never contain track audio paths beyond their basename in user-visible logs (full paths only in debug logs gated behind a flag).
- Crash reports are local-only; the user can opt to copy them to the clipboard to share manually.

-----

## 10. Branching & releases

- `main` is always shippable.
- Feature work on `feat/<short-name>`; merge via squash after CI green.
- Release: bump `Cargo.toml` workspace version, tag `v0.X.Y`, push tag. CI builds notarized macOS dmg, signed Windows msi, and a Linux AppImage. GitHub Release auto-populated from changelog.
- Changelog generated from Conventional Commits since last tag; you write a human-readable preamble per release.

-----

## 11. When you don’t know what to do

In order:

1. Re-read the relevant section of this file.
1. Check the corresponding `docs/*.md`.
1. Check reklawdbox’s source for prior art.
1. Write a `DECISIONS.md` entry: problem, options, chosen path, why.
1. Pick the most reversible option.
1. Ship behind a feature flag if you’re unsure.

If you find yourself wanting to skip tests, take shortcuts, write a “TODO: clean up later” comment, or commit a `console.log` — stop. Those are tells. Take five minutes, write the journal entry, and do it properly. There is no rush. There is only one chance to build the foundation right.

-----

## 12. Bootstrap (only run once, at the very first session)

If `STATUS.md` does not exist, you are in session 1. Do this:

1. Initialize the repo: `git init`, add the LICENSE (GPL-3.0-or-later since ADR-0011; MIT at the time this was written), NOTICE attributing reklawdbox.
1. Create the directory tree from §3 (empty crates/apps with placeholder Cargo.tomls).
1. Set up `Cargo.toml` workspace, `pnpm-workspace.yaml`, `package.json` root.
1. Set up `.github/workflows/ci.yml` running fmt, clippy, test on macOS + Windows.
1. Set up `rust-toolchain.toml` pinning stable.
1. Create `STATUS.md` with phase = 1, current task = “scaffold rekordbox-db crate”.
1. Create `JOURNAL.md` with session 1 entry.
1. Create `DECISIONS.md` with ADR-0001 = “Tauri v2 over Electron” (one paragraph).
1. Create empty `docs/architecture.md`, `docs/data-model.md`, `docs/tools.md`, `docs/ui.md`.
1. First commit: `chore: bootstrap repo`.
1. Push.
1. End session.

After session 1, the rest of phase 1 unfolds task-by-task per §5.

-----

End of contract. Now read `STATUS.md`.
