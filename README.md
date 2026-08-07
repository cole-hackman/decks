# decks

> Local-first AI DJ assistant for Rekordbox 7

**Status:** MVP implementation in progress. Core desktop workflows and local MCP tooling are implemented; real-library validation and `v0.1.0` release verification remain.

`decks` is a native desktop app that sits next to Rekordbox 7 and helps you maintain a clean, well-tagged, well-cued library. It reads your Rekordbox SQLCipher database directly, surfaces problems and opportunities across the library, lets a local Claude agent propose changes, and exports approved edits back to Rekordbox via XML (or — opt-in — applies them in-place under a write guard).

Everything runs on your machine. Your library never leaves your laptop except via enrichment APIs you explicitly enable, and those calls go through a local cache first.

---

## Why decks

Rekordbox's UI is great for performance but tedious for library hygiene at scale: normalizing artist/title formatting across thousands of tracks, finding tracks missing cues or playlist membership, identifying audio duplicates, relocating broken file paths, or staging bulk metadata edits. `decks` treats the library as a first-class object you can query, audit, and edit safely — with a Claude agent on the side that can see only what you've shown it and can only write through reviewable, reversible staged changes.

## Features

### Library access & browsing

- **SQLCipher reader.** Direct read-only access to Rekordbox 7's `master.db` (`crates/rekordbox-db`). Writes are gated behind an explicit, opt-in Sync flow under a `WriteGuard` that refuses to run while Rekordbox is open (WAL-lock probe) and takes a timestamped backup before the first write of a session (see [ADR-0010](docs/DECISIONS.md)).
- **Virtualized track table.** Filterable, sortable, resizable columns; multi-select; right-click context actions. Energy column hydrated from the local feature cache; Key values tinted with the Mixed In Key Camelot palette; inline Tag chips when bindings exist.
- **Track detail panel.** Metadata, cues, and a high-fidelity native Pioneer ANLZ color waveform (PWAV/PWV3/PWV4/PWV5) — not a re-decoded approximation.
- **Native audio preview.** Play/pause, seek, and interactive waveform scrubbing via `rodio`, with an end-of-track event so transport state stays consistent.
- **Analytics dashboard.** Genre / key / BPM distributions rendered with recharts.
- **Persistent filters.** Per-library filter state in `localStorage`, keyed by library path so multiple libraries don't collide.

### Workflow views

- **Inbox** — tracks missing metadata, cues, or playlist membership, with one-click audit.
- **Incoming** — ingest new files from the filesystem, fuzzy-match against the library, and stage adds. Backend CSV parsing with column-mapping UI for batch imports.
- **Track Matcher** — manual review surface for ambiguous matches (`crates/track-matcher`).
- **Smart Fixes** — title/artist normalization: case, encoded chars, garbage suffixes, URLs, promo tags, extracted remixer/artist (`crates/smart-fixes`).
- **Cleanup** — broken-path Relocate workflow with fuzzy filename + size matching (`crates/relocate`), plus audio-fingerprint duplicate detection (chromagram hash + Hamming grouping).
- **Duplicates** — library-wide duplicate scan combining exact title/artist, fuzzy title, and audio-fingerprint strategies in a single view; per-group "keep one" picker archives the rest.
- **Custom Tags** — manage user tags with picker modal, usage badges, and bulk apply (T-key shortcut).
- **Archive** — review and restore previously archived tracks.
- **Sync** — apply staged changes back to `master.db` with options for `cue_destination` (memory/hot/both), `keep_grids`, and `convert_keys` (Camelot / Open Key / original).

### Agent & change pipeline

- **In-app Claude chat panel.** Read-only tools by default; staged changes accumulate as a reviewable batch with inline diffs. Conversation history persists in a local SQLite (WAL) cache.
- **Staged-change lifecycle.** Every mutation flows through `Proposed → Accepted/Rejected → Exported/Applied`. Supported change types: track metadata, cues, playlists, track delete.
- **Bulk "Add intro cues" tool.** Reads the real ANLZ beat grid to stage perfect 1.1 downbeat memory cues + 4-bar loops.
- **Rekordbox XML export.** Default safe egress for accepted changes; round-trip tested (`crates/rekordbox-xml`).

### Local MCP server (provider-neutral)

`decks` ships a CLI that exposes its library tools over MCP so any MCP host — Claude Code, Gemini CLI, OpenAI Responses API, Cursor — can call them with your subscription instead of the desktop chat panel.

```sh
# stdio transport (Claude Code, Gemini CLI)
decks mcp

# local HTTP transport (OpenAI Responses API remote MCP)
decks mcp-http --bind 127.0.0.1:8765

# direct diagnostic invocation
decks tools call library_search --library /path/to/master.db --json '{"query":"UKG"}'
```

Available tools (read-only): `library_search`, `library_get_track`, `library_list_playlists`, `library_get_playlist`, `library_list_cues`, `health_orphan_scan`, `health_duplicate_scan`, `health_fuzzy_duplicate_scan`, `health_broken_link_scan`, `staging_list_changes`. XML export is intentionally omitted from MCP discovery until the shared tool service owns the export path.

See [docs/MCP.md](docs/MCP.md) for full setup including Claude Code, Gemini CLI, and OpenAI Responses API.

### Privacy & safety guarantees

- **No telemetry.** No analytics. No remote logging.
- **`master.db` is read-only by default.** Writes only happen through the explicit Sync flow, behind a `WriteGuard`.
- **Refuses to write while Rekordbox is running.** WAL lock is probed before any mutation.
- **Per-session backup.** First write of a session creates a timestamped copy of `master.db` next to the original.
- **Anthropic API key stored in the OS keychain.** Never written to disk in plaintext or committed config.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  React / TypeScript frontend  (apps/desktop/src)        │
│  Vite · Tailwind · Radix UI · Zustand · TanStack Query  │
├─────────────────────────────────────────────────────────┤
│  Tauri IPC layer  (apps/desktop/src-tauri)              │
│  Typed commands + events bridging frontend ↔ Rust core  │
├─────────────────────────────────────────────────────────┤
│  Rust core  (crates/*)                                  │
│  rekordbox-db · rekordbox-xml · audio-analysis          │
│  audio-tags · stratum-dsp · relocate · smart-fixes      │
│  track-matcher · agent-tools · changes · cache          │
│  classify · scoring · enrichment · embeddings · ranker  │
│  plugins · decks-core (facade)                          │
└─────────────────────────────────────────────────────────┘
```

- **Tauri 2** desktop shell — system WebView, no Chromium ([ADR-0001](docs/DECISIONS.md)).
- **Rust workspace** with one crate per bounded concern.
- **TypeScript/React** frontend, fully typed IPC.
- **Local SQLite (WAL) cache** for audio features, waveform peaks, conversation history, and fingerprints (`crates/cache`).
- **Shared tool service** (`crates/agent-tools`) — the same Rust functions back the in-app chat, the MCP server, and the `decks tools call` CLI.

See [docs/architecture.md](docs/architecture.md) for the full design, [docs/data-model.md](docs/data-model.md) for schema details, and [docs/DECISIONS.md](docs/DECISIONS.md) for the ADR log.

## Repository layout

```
apps/
  desktop/        Tauri 2 + React frontend & src-tauri backend
  cli/            `decks` CLI (mcp, mcp-http, tools call)
crates/
  rekordbox-db    SQLCipher reader + ANLZ beat-grid parser
  rekordbox-xml   Rekordbox XML parse/emit (round-trip tested)
  audio-analysis  Decoding, peak extraction, fingerprinting
  audio-tags      ID3 / tag reading
  cache           SQLite WAL cache (features, waveforms, conversation)
  changes         Staged-change lifecycle + applier + key formats
  relocate        Broken-path matching (fuzzy filename + size)
  smart-fixes     Title/artist normalization
  track-matcher   Fuzzy library matching for incoming tracks
  agent-tools     Shared tool service (chat + MCP + CLI)
  decks-core      Facade re-export crate
  stratum-dsp · classify · scoring · enrichment ·
  embeddings · ranker · prodjlink · plugins
docs/             Architecture, ADRs, MCP, manual test plan, journal
scripts/          dev.sh, release.sh, real-library-smoke.sh, …
fixtures/         Synthetic test libraries (real fixtures gitignored)
```

## Development

```sh
# Prerequisites: Rust stable, Node 20+, pnpm 9+
./scripts/dev.sh
```

Common workflows:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm test                     # vitest
pnpm typecheck && pnpm lint
pnpm e2e                      # Playwright
pnpm --filter desktop tauri build   # produce a packaged .app / DMG
./scripts/real-library-smoke.sh     # data-layer smoke against a real master.db
```

See [docs/MANUAL_TEST_PLAN.md](docs/MANUAL_TEST_PLAN.md) for the human-in-the-loop verification checklist, and [docs/STATUS.md](docs/STATUS.md) for current implementation state.

## Attribution

Core Rust logic vendored and adapted from [reklawdbox](https://github.com/ryan-voitiskis/reklawdbox) (MIT, Ryan Voitiskis). See [NOTICE](NOTICE).

## Roadmap

`decks` is working toward feature parity with [Lexicon DJ](https://www.lexicondj.com) within
Rekordbox. See [docs/ROADMAP.md](docs/ROADMAP.md) for the epic queue,
[docs/lexicon/](docs/lexicon/) for the reference specification, and
[docs/lexicon/PARITY.md](docs/lexicon/PARITY.md) for the current feature matrix.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).

Relicensed from MIT in 2026 so the project can use copyleft analysis libraries
(libKeyFinder, aubio, audiowaveform) that have no permissive equivalent. The
vendored reklawdbox code was MIT, which is GPL-compatible; attribution is
preserved in [NOTICE](NOTICE). See ADR-0011 in [docs/DECISIONS.md](docs/DECISIONS.md).
