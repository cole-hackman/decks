# Architecture

> Living document — update in the same commit as the code it describes.

## Overview

`decks` is a native desktop application with a three-layer architecture:

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
│  audio-tags · stratum-dsp · relocate · smartlists        │
│  agent-tools (shared MCP/CLI/Tauri tool service)        │
│  changes · cache · classify · scoring · enrichment      │
│  embeddings · ranker · plugins                          │
│  decks-core (facade)                                    │
└─────────────────────────────────────────────────────────┘

In addition to the Tauri desktop app, the workspace ships a `decks` CLI
binary (`apps/cli`) providing three subcommands:

- `decks mcp` — local stdio MCP server (Claude Code, Gemini CLI, …)
- `decks mcp-http --bind <addr>` — local HTTP MCP transport for OpenAI
  Responses API remote-MCP development
- `decks tools call <tool> --library <path> --json <args>` — direct
  diagnostic invocation of any shared tool
```

## Key design principles

- **Local-first**: all data stays on the user's machine; enrichment API calls go through a local cache first.
- **Read-only DB**: `master.db` is opened with read-only flags everywhere except the Phase 6 opt-in live-patch mode.
- **Staged changes**: all proposed library mutations accumulate in the `changes` crate's `ChangeManager` and are only applied via XML export (or Phase 6 direct patch).
- **Privacy-first**: no telemetry, no analytics, no remote logging.

## Crate dependency graph

```
decks-core   ──▶ rekordbox-db, rekordbox-xml, cache, changes, scoring,
                 classify
agent-tools  ──▶ decks-core, changes, cache, audio-tags, audio-analysis,
                 relocate
audio-analysis ─▶ stratum-dsp, audio-tags
apps/cli     ──▶ decks-core, agent-tools
src-tauri    ──▶ decks-core, agent-tools, changes, cache
```

`smartlists` owns the rule model and evaluator for dynamic playlists. It depends on
`rekordbox-db` (for `Track`) and `changes` (for key-notation conversion and the staged-change
types it emits when materialising a smartlist into a Rekordbox playlist). Evaluation is pure and
in-memory over an already-loaded track list — see ADR-0013.

`agent-tools` is the provider-neutral tool service shared by the local
MCP server, the diagnostic CLI, and the Tauri chat panel — it owns the
`ToolRequest` enum and dispatches against the underlying crates so
behaviour stays aligned across surfaces (see ADR-0003).

Phase 4+ crates (`embeddings`, `prodjlink`, `ranker`, `plugins`) are pulled in by `decks-core` when their features are enabled.

## Data flow (Phase 1)

```
master.db (read-only)
    │  rekordbox-db
    ▼
Track / Playlist / Cue structs
    │  Tauri IPC
    ▼
TanStack Query cache (frontend)
    │  React components
    ▼
Library browser UI
```
