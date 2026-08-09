# decks

Local-first AI library assistant for Rekordbox DJs — audit, clean, and
re-cue a music library with a Claude agent that can only act through
reviewable staged changes.

**Status:** MVP in progress. Core desktop workflows and the local MCP
tooling work; real-library validation and a first tagged release (v0.1.0)
remain.

<!-- SCREENSHOT: the differentiator shot — the chat panel proposing changes
with the staged-changes diff view open next to the track table (Proposed →
Accepted lifecycle visible). Second choice: track detail with the native
ANLZ color waveform. Keep under 5MB. -->

## The problem

Rekordbox is built for performing, not for library hygiene. Normalizing
artist and title formatting across thousands of tracks, finding tracks with
no cues or no playlist, spotting audio duplicates, and relocating broken
file paths are all one-track-at-a-time chores in its UI. The paid benchmark
for this job is Lexicon DJ; decks is working toward Lexicon parity within
Rekordbox (the parity matrix in `docs/lexicon/PARITY.md` drives the
roadmap) as a local-first desktop app with an agent attached — one that can
see only what you've shown it and can only write through staged,
reversible changes.

<!-- TODO: verify — one line on provenance (built against your own library?
what chore triggered it?). See README-QUESTIONS.md. -->

## How it works

- A Tauri 2 desktop app: React/TypeScript frontend over a Rust workspace
  (~20 crates, one per bounded concern), with typed IPC between them.
- `crates/rekordbox-db` opens Rekordbox 7's SQLCipher `master.db` directly,
  read-only, and parses the native ANLZ files (beat grids, color
  waveforms). Audio features, waveform peaks, fingerprints, and chat
  history live in a local SQLite (WAL) cache.
- Every mutation — from the agent, Smart Fixes, Cleanup, or the Duplicates
  view — is a staged change: `Proposed → Accepted/Rejected →
  Exported/Applied`, previewable as a diff. The default egress is
  round-trip-tested Rekordbox XML export; an opt-in Sync flow writes
  directly to `master.db` under a `WriteGuard`.
- The same Rust tool service backs three surfaces at once: the in-app
  Claude chat panel, an MCP server (`decks mcp` / `decks mcp-http`) usable
  from any MCP host on your existing subscription, and a `decks tools call`
  CLI.
- Everything runs on your machine. No telemetry; enrichment APIs
  (MusicBrainz, opt-in Discogs) only fire if enabled, through a local
  cache. The Anthropic key lives in the OS keychain.

## Running it

Requires Rust stable, Node 20+, pnpm 9+, and a Rekordbox 7 library
(`master.db`) to point at.

    git clone https://github.com/cole-hackman/decks
    cd decks
    ./scripts/dev.sh

Tests run against synthetic fixture libraries in `fixtures/` — no real
library needed:

    cargo test --workspace
    pnpm test && pnpm typecheck && pnpm lint
    pnpm e2e                            # Playwright
    ./scripts/real-library-smoke.sh     # data-layer smoke vs a real master.db copy

## Scope and non-goals

**In scope:** Rekordbox 7, deep — browsing, auditing, smart fixes,
duplicates, relocation, cues, smartlists, staged edits back to the
library.

**Not in scope (deliberately, for now):**

- Other DJ software (Serato, Traktor, Engine DJ, VirtualDJ) and USB/CDJ
  export — deferred until Rekordbox parity is done.
- Cloud anything: no sync, no backup service, no mobile app.
- Silent fixes. Playlist duplicate entries are surfaced, not auto-removed;
  every change is staged for review.

## Tradeoffs

**Staged changes everywhere, direct writes almost nowhere.** The founding
invariant was "never mutate `master.db`" — all edits stage into a
reviewable batch and leave via XML export the user imports manually. That
bought trust (an agent can propose bulk edits to a library you care about
with zero blast radius) and cost friction: for "rename one genre across
200 tracks," quit-Rekordbox-and-import is a heavy loop. ADR-0010 formally
relaxed the invariant for one path: an opt-in Sync that writes in-place,
gated by a `WriteGuard` that probes Rekordbox's WAL lock, takes a
timestamped backup on first write, and commits atomically — reducing the
worst case from "library destroyed" to "copy the `.bak` back."

**Tauri 2 over Electron.** The core logic is Rust (SQLCipher access, audio
analysis, DSP), so Tauri lets the frontend call it directly instead of
through an FFI or subprocess boundary, and ships a ~5–10 MB binary using
30–80 MB of memory — which matters when Rekordbox and a DAW are open next
to it. The cost: the system WebView differs per OS (Safari, WebView2,
WebKitGTK), so rendering must be verified on each platform instead of once
in a bundled Chromium.

## Known limitations and failure modes

- It has not been validated against a real library at scale yet — that's
  the line between here and v0.1.0. Synthetic fixtures cover the test
  suites; real Rekordbox databases have column variants and ANLZ quirks
  the fixtures don't.
- The whole foundation is a reverse-engineered format: SQLCipher schema
  and ANLZ binary parsing (vendored and adapted from MIT-licensed
  reklawdbox). A Rekordbox update can break reads — and the Sync write
  path — overnight.
- An agent session once shipped a non-compiling workspace while its notes
  claimed "verified clean." ADR-0009 is the scar: agent-built work is
  untrusted until the full verification suite passes, and "done" claims
  without captured command output don't count.
- CI runs the Rust suite (fmt, clippy, tests) on macOS and Windows plus
  frontend typecheck/lint/vitest — but the Playwright e2e suite and the
  real-library smoke are local-only gates, and Linux isn't in the matrix.
- No telemetry cuts both ways: nothing phones home, and nothing tells me
  what broke on someone else's machine.

## What I'd do next

1. Real-library validation and the v0.1.0 release — the smoke harness
   exists; it needs to be run in anger against large libraries.
2. Work down the Lexicon parity matrix (`docs/lexicon/PARITY.md`) — one
   epic per branch, already queued in `docs/ROADMAP.md`.
3. Put Linux in the CI matrix and get e2e running in CI, so the
   three-WebView cost of the Tauri decision is actually paid down.

## Stack

Tauri 2 · Rust · React · TypeScript · Vite · Tailwind · SQLite/SQLCipher ·
GPL-3.0-or-later
