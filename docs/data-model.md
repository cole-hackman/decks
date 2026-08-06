# Data Model

> Living document — update in the same commit as the code it describes.

## Source of truth

The user's Rekordbox 7 `master.db` (SQLCipher-encrypted SQLite) is the primary source of truth for the library. We never write to it (until Phase 6 opt-in). Our `cache` SQLite database stores derived data (audio features, embeddings, change log, conversation history).

## Core types (`crates/rekordbox-db`)

### Track
- `id: String` — Rekordbox internal ID; Rekordbox 7 schemas can expose IDs as text
- `title: String`
- `artist: Option<String>`
- `album: Option<String>`
- `genre: Option<String>`
- `musical_key: Option<String>` — Camelot wheel notation when present
- `bpm: Option<f64>` — converted from DB integer × 100
- `duration_secs: Option<i64>`
- `folder_path: Option<String>`
- `analysis_data_path: Option<String>`
- `rating: Option<i64>` — Rekordbox rating value
- `comment: Option<String>`
- `file_type: Option<i64>`
- `sample_rate: Option<i64>`
- `bit_rate: Option<i64>`
- `release_year: Option<i64>`
- `dj_play_count: Option<i64>`

### Playlist
- `id: String`
- `name: String`
- `parent_id: Option<String>`
- `seq: Option<i64>`
- `kind: PlaylistKind` — `Playlist | Folder | SmartPlaylist | Unknown(i64)`

### PlaylistEntry
- `playlist_id: String`
- `content_id: String`
- `track_no: Option<i64>`

### HotCue / Memory Cue
- `id: String`
- `content_id: String`
- `in_msec: Option<i64>`
- `out_msec: Option<i64>`
- `kind: CueKind` — `MemoryCue | HotCue(u8)`
- `color: Option<i64>` — Rekordbox color ID; `-1` means unset
- `comment: Option<String>`

### BeatGrid
- `track_id: u64`
- `entries: Vec<BeatGridEntry>`

### BeatGridEntry
- `beat: u32`
- `position_ms: u64`
- `bpm: f64`

## Cache types (`crates/cache`)

The cache is a local SQLite WAL database whose schema is versioned via `PRAGMA user_version`. Migrations live in `crates/cache/src/migrations.rs`. Current schema version: **v7**.

### v1 — `audio_features`
Derived from audio analysis; cached per `(track_uri, analyzer_version)`.

```
audio_features (
    track_uri        TEXT NOT NULL,
    analyzer_version TEXT NOT NULL,
    bpm              REAL,
    musical_key      TEXT,
    energy           REAL,
    features_json    TEXT,
    created_at       INTEGER,
    PRIMARY KEY (track_uri, analyzer_version)
)
```

### v2 — `conversations`, `conversation_messages`
Persisted agent conversations. `conversation_messages.content_json` stores role-tagged content blocks plus tool inputs and results. Indexed by `(library_path, updated_at DESC)` for the conversation selector.

### v3 — `staged_changes`
Staged mutations. `kind` is a `ChangeKind` discriminant (e.g. `TrackMetadataEdit`, `TrackAddCue`, `PlaylistRename`, `PlaylistCreate`, `PlaylistAddTrack`, `PlaylistRemoveTrack`, `PlaylistDelete`). `status` cycles `Proposed → Accepted/Rejected → Exported`. Indexed by `(library_path, status, updated_at DESC)`.

### v4 — `audio_fingerprints`
Compact chromagram hash per track for Hamming-distance duplicate grouping.

```
audio_fingerprints (
    track_uri   TEXT PRIMARY KEY,
    chroma_hash BLOB NOT NULL,
    created_at  INTEGER
)
```

### v5 — `tag_categories`, `tags`, `track_tags`, `incoming_watermark`, `archived_tracks`, `common_text_blocklist`, `field_mappings`, `sync_runs`
Custom Tags, Incoming/Archive, and Smart Fixes / Sync config.

> `field_mappings` and `sync_runs` were created here but are **not read or written by any code**.
> `field_mappings` is also the wrong shape for real field mappings — its primary key is
> `(library_path, source_field)`, so it cannot express Lexicon's multi-source combining or
> per-DJ-app mappings. Epic 4 should replace it rather than build on it.

### v6 — `waveform_peaks`
Decoded peak data cached per track URI so waveforms survive restarts.

### v7 — `smartlists`
Rules-driven dynamic playlists (Epic 1).

```
smartlists (
    id               TEXT PRIMARY KEY,
    library_path     TEXT NOT NULL,
    name             TEXT NOT NULL,
    parent_folder_id TEXT,
    combinator       TEXT NOT NULL,   -- "all" | "any"
    clauses_json     TEXT NOT NULL,
    seq              INTEGER NOT NULL DEFAULT 0,
    created_at       INTEGER,
    updated_at       INTEGER
)
```

Rules are a JSON document rather than normalised clause/rule tables: evaluation is in-memory
(ADR-0013) so no query ever filters on an individual rule, and `staged_changes` already stores
JSON payloads in this database. A corrupt `clauses_json` degrades to an empty rule set, which
matches nothing.

`parent_folder_id` doubles as the Smartlist Generator's ledger — generated smartlists sit in the
reserved `Lexicon` folder, and moving one out is what makes the generator recreate it.

### v8 – v14

Added by Epics 4 and 5, each documented in place in `crates/cache/src/migrations.rs`:
`path_mappings` (v8), `quick_move_folders` (v9), `watch_folders` + `watch_dismissed` (v10),
`field_mapping_rules` (v11), `incoming_reviewed` (v12), `undo_runs` + `undo_entries` (v13),
`cleanup_locks` + `cleanup_pinned_letters` (v14).

### v15 — `mixable_templates`
Saved Mixable Tracks option sets (Epic 6).

```
mixable_templates (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    options    TEXT NOT NULL,     -- serialised scoring::MixableOptions
    created_at INTEGER NOT NULL
)
```

Rules are a JSON document for the same reason smartlists are: nothing queries an individual rule,
the set is always loaded whole, and a rule added later must not need a migration. A template whose
JSON no longer parses is skipped when listing rather than failing the list.

**Not keyed by `library_path`.** "My peak-time rules" is a statement about how someone mixes, not
about one database — the same reasoning as `path_mappings` and `cleanup_locks`.

`name` is unique because saving over a template by name is the intended workflow; a second
`Peak time` in the list is not what anyone doing that meant.

### Planned

- **Embedding** — 512-d CLAP vector; cached per `(track_id, model_version, chunking_config)`. Pending `crates/embeddings` (Phase 4).
- **TransitionPair** — accept/reject history feeding the learned ranker (Phase 5).
