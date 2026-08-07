# Tool Inventory

> Authoritative list of agent tools. Each tool has: name, summary, parameters (typed), return type, idempotency, side effects, cost class.
>
> Cost classes: **free** (pure computation / local DB read), **network** (HTTP call to external service), **model-call** (LLM invocation).
>
> Living document — every new tool must be added here in the same commit it is implemented.

---

## Currently Implemented

The shared `agent-tools` service (`crates/agent-tools/src/types.rs::ToolRequest`) exposes a provider-neutral tool surface that powers the local MCP server (`decks mcp` / `decks mcp-http`), the diagnostic CLI (`decks tools call`), and the embedded Tauri chat panel.

Surfaces:

- **MCP** (`docs/MCP.md`) — host-safe underscore names; dotted aliases (`library.search`) also parsed by the CLI.
- **Tauri IPC** — additional desktop-only commands (analytics, list-tracks-with-cues, list-tracks-in-any-playlist) not advertised over MCP.

Implemented tools:

- `library.search`
- `library.get_track`
- `library.list_playlists`
- `library.get_playlist`
- `library.list_cues`
- `library.read_file_tags`
- `library.analyze_track`
- `library.scan_and_propose_missing`
- `library.bulk_add_intro_cues`
- `library.list_tracks_with_cues` (Tauri IPC only)
- `library.list_tracks_in_any_playlist` (Tauri IPC only)
- `library.analytics` (Tauri IPC only)
- `health.orphan_scan`
- `health.duplicate_scan`
- `health.fuzzy_duplicate_scan`
- `health.broken_link_scan`
- `relocate.scan`
- `relocate.apply`
- `staging.list_changes`
- `staging.stage_change` (agent-only; UI never calls this directly)
- `export_accepted_changes` (Tauri IPC only — not advertised over MCP per ADR-0003)

---

## Library tools

### `library.search`
Search tracks by text query across title, artist, album, genre, comment.

| Field | Value |
|-------|-------|
| Parameters | `query: string`, `limit?: number` (default 50) |
| Returns | `Track[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `library.get_track`
Fetch full track details by ID.

| Field | Value |
|-------|-------|
| Parameters | `id: string` |
| Returns | `Track` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `library.list_playlists`
List all playlists and folders.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `Playlist[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `library.get_playlist`
Fetch one playlist/folder and its ordered track entries.

| Field | Value |
|-------|-------|
| Parameters | `id: string` |
| Returns | `{ playlist: Playlist, tracks: Track[] }` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `library.list_cues`
List hot cues for a track.

| Field | Value |
|-------|-------|
| Parameters | `track_id: string` |
| Returns | `HotCue[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `library.read_file_tags`
Read embedded ID3/MP4/FLAC tags (lofty-based) for a track whose `folder_path` resolves on disk. Useful for surfacing drift between Rekordbox metadata and the file itself.

| Field | Value |
|-------|-------|
| Parameters | `track_id: string` |
| Returns | `{ title?, artist?, album?, genre?, bpm?, key?, comment?, year?, duration? }` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `library.analyze_track`
Symphonia decode + stratum-dsp analysis (BPM, key) with Camelot conversion. Results cached in `audio_features`.

| Field | Value |
|-------|-------|
| Parameters | `track_id: string`, `force?: boolean` |
| Returns | `{ bpm: number, key: string, confidence: number, … }` |
| Idempotent | yes (cached) |
| Side effects | writes to cache DB |
| Cost | free (CPU; seconds per track) |

### `library.scan_and_propose_missing`
Identify tracks with missing metadata (artist, BPM, key, etc.) and stage suggested corrections derived from `library.analyze_track` or file tags.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | summary of staged proposals |
| Idempotent | no (stages changes) |
| Side effects | writes proposed `Change` rows to cache DB |
| Cost | free (CPU; scales with library) |

### `library.bulk_add_intro_cues`
Reads the real ANLZ beat grid for each track, finds the first `beat_number == 1` downbeat, computes a 4-bar loop length from local BPM, and stages a memory cue + memory loop pair.

| Field | Value |
|-------|-------|
| Parameters | `track_ids: string[]` |
| Returns | summary of staged cues |
| Idempotent | no (stages changes) |
| Side effects | writes proposed `Change` rows to cache DB |
| Cost | free |

### `library.list_tracks_with_cues` _(Tauri IPC only)_
List track IDs that have at least one cue. Used by the structured filter system.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `string[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `library.list_tracks_in_any_playlist` _(Tauri IPC only)_
List track IDs that appear in at least one non-smart playlist. Used by the "not in any playlist" filter and Inbox view.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `string[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `library.analytics` _(Tauri IPC only)_
Aggregate genre, key, and BPM distributions (computed in SQLite). Drives `AnalyticsView`.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `{ total_tracks, by_genre, by_key, bpm_histogram }` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

---

## Health tools

### `health.orphan_scan`
Find tracks whose audio files are missing from disk.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `Track[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `health.duplicate_scan`
Find exact-match duplicate candidates (same title/artist/duration).

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `DuplicateGroup[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `health.fuzzy_duplicate_scan`
Find near-duplicate candidates via fuzzy string matching on normalized title + artist.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `DuplicateGroup[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `health.broken_link_scan`
Find tracks with broken metadata (missing/zero BPM, missing key, etc.). Returns categorized buckets, not a flat array.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `{ missing_bpm: Track[], missing_key: Track[], … }` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

---

## Relocate tools

### `relocate.scan`
Walk one or more root directories, index audio files, and propose folder-path corrections for missing tracks via exact filename + size match, falling back to Levenshtein-distance fuzzy match.

| Field | Value |
|-------|-------|
| Parameters | `roots: string[]` |
| Returns | `RelocateCandidate[]` per orphan track |
| Idempotent | yes |
| Side effects | none |
| Cost | free (CPU; scales with disk) |

### `relocate.apply`
Stage `TrackMetadataEdit` changes (field=`folder_path`) for a set of accepted relocate candidates. The old value is the original folder path so the diff reads as a relocation, not new metadata.

| Field | Value |
|-------|-------|
| Parameters | `candidates: RelocateCandidate[]` |
| Returns | summary of staged changes |
| Idempotent | no (stages changes) |
| Side effects | writes proposed `Change` rows to cache DB |
| Cost | free |

---

## Staging tools

The agent can propose and list staged changes. Accept/reject/export actions are user-driven through UI/IPC, not autonomous agent tools. XML export is intentionally not advertised over MCP until the export path moves into the shared tool service (ADR-0003).

### `staging.stage_change`
Propose a change for user review. Does not apply anything to Rekordbox.

| Field | Value |
|-------|-------|
| Parameters | `kind: ChangeKind`, `target_id?: string`, `field?: string`, `old_value?: unknown`, `new_value?: unknown`, `reason?: string`, `confidence?: number` |
| Returns | `Change` |
| Idempotent | no |
| Side effects | writes proposed change to cache DB only |
| Cost | free |

### `staging.list_changes`
List all staged changes.

| Field | Value |
|-------|-------|
| Parameters | — |
| Returns | `Change[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `staging.accept_change` _(Tauri IPC only)_
Accept a staged change (marks it ready for export).

| Field | Value |
|-------|-------|
| Parameters | `change_id: string` |
| Returns | `Change` |
| Idempotent | yes |
| Side effects | mutates change state |
| Cost | free |

### `staging.reject_change` _(Tauri IPC only)_
Reject a staged change.

| Field | Value |
|-------|-------|
| Parameters | `change_id: string` |
| Returns | `Change` |
| Idempotent | yes |
| Side effects | mutates change state |
| Cost | free |

### `export_accepted_changes` _(Tauri IPC only)_
Generate a Rekordbox-importable XML file with all accepted changes overlaid on the live library. Parses the generated XML before marking changes exported.

| Field | Value |
|-------|-------|
| Parameters | `output_path: string` |
| Returns | `ExportResult` |
| Idempotent | no (writes file) |
| Side effects | writes XML file to disk; marks accepted changes exported |
| Cost | free |

### `smartlist_list`
List saved smartlists (rules-driven dynamic playlists) for a library.

| Field | Value |
|-------|-------|
| Parameters | `library_path: string` |
| Returns | `Smartlist[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `smartlist_evaluate`
Evaluate a saved smartlist and return the tracks that currently match its rules. Archived tracks
are excluded unless a rule explicitly selects them.

| Field | Value |
|-------|-------|
| Parameters | `library_path: string`, `id: string` |
| Returns | `Track[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `undo_list`
List Sync runs recorded for a library, newest first, with how many changes of each can be reversed
and how many cannot.

| Field | Value |
|-------|-------|
| Parameters | `library_path: string` |
| Returns | `UndoRun[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `undo_entries`
Show what one Sync run did, and the reason for each change that cannot be undone. A blocked entry
carries a sentence explaining why (ADR-0008) rather than being omitted.

| Field | Value |
|-------|-------|
| Parameters | `run_id: string` |
| Returns | `UndoEntry[]` |
| Idempotent | yes |
| Side effects | none |
| Cost | free |

### `undo_run`
Stage the inverse of everything a Sync run applied. The inverses land as **proposed** changes and
still go through review and Sync — this never writes to `master.db`. A run can only be undone once;
a second call errors rather than staging a duplicate pile.

| Field | Value |
|-------|-------|
| Parameters | `library_path: string`, `run_id: string` |
| Returns | `StagedUndo` — `{ staged: string[], blocked: [description, reason][] }` |
| Idempotent | no (stages changes; refuses on a second call) |
| Side effects | writes rows to the cache DB's `staged_changes`; marks the run undone |
| Cost | free |

---

## Phase 3+ — Not yet implemented

The tools below are part of the long-range roadmap (`CLAUDE_CODE_PROMPT.md` §5). They are **specs, not contracts** — the parameter and return shapes will be revisited when implemented. Do not assume the agent has access to them.

### Audio tools

#### `audio.get_waveform` _(planned)_
Return downsampled peak data for rendering. The native Pioneer ANLZ color waveform (PWAV/PWV3/PWV4/PWV5) is already available via the `get_anlz_waveform` Tauri command; this future tool would expose symphonia-decoded peaks for tracks without ANLZ data.

### Enrichment tools _(Phase 2 carryover; not implemented)_

- `enrichment.discogs_lookup`
- `enrichment.mb_lookup`
- `enrichment.beatport_lookup`
- `enrichment.bandcamp_lookup`

### Classify tools _(Phase 2 carryover; not implemented)_

- `classify.genre_classify`
- `classify.genre_audit`

### Sets tools _(Phase 3)_

- `sets.score_transition`
- `sets.sequence_set`
- `sets.plan_chapters`

### Pools tools _(Phase 3)_

- `pools.find_pool`
- `pools.expand_pool`

### Embeddings tools _(Phase 4)_

- `embeddings.find_similar`
- `embeddings.text_to_tracks`
- `embeddings.cluster_library`

### Plugin tools _(Phase 5)_

- `plugins.register_plugin`
- `plugins.list_plugins`
- `plugins.call_plugin`
