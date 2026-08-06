# Lexicon Parity Matrix

Every Lexicon capability, `decks`'s current status, and the epic that owns closing the gap.

**Status key** — `done` (parity or better) · `partial` (exists but materially incomplete) ·
`missing` · `deferred` (out of scope for this initiative)

**Scope** — the decision is *Rekordbox-deep first*. Non-Rekordbox app adapters are `deferred`
regardless of how much of Lexicon they represent.

---

## Summary

| Domain | done | partial | missing | deferred |
|---|---:|---:|---:|---:|
| Interop & sync | 4 | 7 | 1 | 11 |
| Library & browser | 0 | 9 | 7 | 0 |
| Smartlists | 2 | 1 | 0 | 0 |
| Analysis | 0 | 5 | 4 | 0 |
| Player, cues, generator | 7 | 7 | 2 | 0 |
| Files | 5 | 6 | 0 | 0 |
| Health | 0 | 3 | 2 | 0 |
| Recipes & editing | 1 | 2 | 4 | 0 |
| Streaming | 0 | 1 | 8 | 0 |
| History & backup | 1 | 0 | 2 | 2 |
| Extensibility | 0 | 0 | 0 | 3 |
| **Total** | **22** | **41** | **24** | **16** |

The shape of the work: `decks` has broad shallow coverage of library *hygiene* and almost nothing
of library *editing*, *automation*, or *set preparation*.

---

## Interop & sync — `01-interop.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Rekordbox 6/7 direct DB read | **done** | SQLCipher reader, ANLZ parser | — |
| Rekordbox XML emit | **done** | Round-trip tested | — |
| Rekordbox direct DB write | partial | `WriteGuard` + applier; stricter than Lexicon (refuses while RB is open) | — |
| Full / Playlist / Modified sync | partial | Modes exist; no per-app modified watermark, no Full-Sync delete | 6 |
| Cue Destination | partial | Sets `Kind` on new cues only; no hidden-memory-cue round-trip | 2 |
| Don't Touch My Grids | partial | Only skips BPM edits — no grid writes exist yet to skip | 2 |
| Key conversion | partial | Camelot + Open Key done; no leading-zero option | 6 |
| Colors → nearest | **missing** | Option plumbed through `SyncOptions` and **ignored** | 6 |
| All smartlists → playlists | **done** | Materialises via `PlaylistCreate` + `PlaylistAddTrack`, staged before the change set is collected | 1 |
| Field Mappings | partial | Engine + ID3 profile done, v5 dead table dropped; no per-DJ-app profiles, not applied during sync | 4 |
| Excluded From Sync | **done** | Name-prefix (case-insensitive) and custom-tag conventions, both honoured during materialisation | 1 |
| Beatshift correction on import/sync | **missing** | Correctness issue — we already write cues | 4 |
| Serato / Traktor / VirtualDJ / Engine / djay / Apple Music / M3U / USB / DIRECT2CDJ | deferred | 11 items | — |

## Library & browser — `02-library.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Virtualized track table | partial | Resizable, sortable, inline column search, multi-select | — |
| Search operators (`None`, `>`, `<`, ranges, `!`) | **partial** | Implemented in the smartlist rule engine; the track-browser search box does not yet share the vocabulary | 1 |
| Tag query language (`~`, `!`, comma) in the search box | **missing** | The rule engine expresses the same logic via `has_all`/`has_any`/`has_none`; the browser search syntax is separate | 5 |
| Key-notation-aware search | **partial** | `canonical_key` handles Camelot / Open Key / musical spellings in smartlist rules; not yet wired into the browser search box | 1 |
| Spreadsheet keyboard navigation | **missing** | | 2 |
| Inline per-row waveform preview | **missing** | | 2 |
| Compatible-key indicator | **missing** | | 2 |
| Track Timeline | **missing** | | 6 |
| Playlists tree | partial | No folder-drop, no M3U import, no drag-between, no create-from-selection | 6 |
| Favorite Playlists + hotkeys | **missing** | | 6 |
| Playlist Merge / Sort / Cross Reference / Prefix / Rewrite Order | **missing** | 5 tools; Rewrite Order is high value for CDJ export | 6 |
| Playlist Occurrence | partial | Only the N=0 case | 6 |
| Custom Tags | partial | Strong already; missing category colours, drag-reorder, OR/AND selection on the Tags page, MyTag import, per-tag hotkeys | 5 |
| Manual multi-track editor | **missing** | `<multiple values>` semantics | 5 |
| Album art | **missing** | Absent from the product entirely | 4 |
| Archive | partial | Missing context-sensitive playlist rule, selection helper, delete-from-disk | 5 |
| Genre / Artist Cleanup | partial | Missing locking, pinned letters, alt-click filter, extra artist fields | 5 |

## Smartlists — `03-smartlists.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Smartlist rules engine | **done** | `crates/smartlists`: two-level rule model, in-memory evaluator, 30s recompute throttle, cache v7 | 1 |
| Smartlist Generator | **done** | By field / tag category / decade / BPM range / play count; idempotent via the `Lexicon` folder | 1 |
| Smartlists in the playlist tree | **partial** | Ships as a dedicated view. Rekordbox playlists come from `master.db` and smartlists from the cache DB, so merging the two trees is a larger refactor — deferred | 6 |

## Analysis — `04-analysis.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| BPM & beatgrid analysis | partial | `stratum-dsp`; no BPM range, no accuracy classes | 2, 3 |
| BPM changepoints | **missing** | Lexicon has no dynamic analysis either — only manual changepoints | 2 |
| Waveform | partial | Reads native Pioneer ANLZ (arguably better); no custom colours, no bulk pre-gen | 2 |
| Key detection | partial | Chroma-based; single algorithm | 6 |
| **Camelot → Open Key posture** | **decision needed** | Lexicon avoids Camelot for licensing; we use it everywhere incl. a "Mixed In Key" palette | see GAPS |
| Energy | partial | Cached + displayed; no defined scale, no deliberate fill | 4 |
| Danceability / Popularity / Happiness | **missing** | | 4 |
| Auto-analyze on add | **missing** | | 4 |
| Mixable Tracks | partial | **`scoring::score_transition` + `suggest_next_tracks` exist with no UI caller** | 6 |
| Beatshift Fixer | **missing** | | 4 |

## Player, cues, generator — `05-cues-player.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Playback + waveform scrub | partial | `rodio`; no queue, no autoplay | 2 |
| Play queue | **missing** | | 2 |
| Cue CRUD | **done** | Set/play/delete/move/colour via `CueEditor`, all through staged changes. New `ChangeKind::TrackDeleteCue`. Placement is on the cue list + slot grid, not yet drag-on-waveform | 2 |
| Loops | partial | Loop length in beats via `OutMsec`. **Active loops need a `djmdCue` column we do not model** — deferred | 2 |
| Quantize (incl. grid-move-carries-cues) | **done** | 1/2/4/16/64-beat snapping; a grid nudge moves only cues already on the grid | 2 |
| Cue templates | **missing** | | 2 |
| Beatgrid editing | partial | Grid nudge stages the cue moves that follow it. Writing the grid itself back to ANLZ, and half/double BPM, still missing | 2 |
| Beat jump | **done** | ±4/±16 beats along the real ANLZ grid, clamped at both ends | 2 |
| Hotkeys: rebinding, global, inline hints | partial | Rebinding + persistence + conflict detection exist in the registry; no settings UI yet, and no system-wide hotkeys | 2 |
| **Action registry** | **done** | `lib/actions.ts` — bindings, rebinding, conflict detection, search. App globals migrated onto it | 2 |
| Action Center (`Cmd+Space`) | **done** | Palette over the registry with fuzzy search and arrow navigation | 2 |
| Find Popup (`Cmd+F`) | **missing** | | 2 |
| **Cue Point Generator** | partial | Template engine + custom cue anchors ship; **structural detection (drop/breakdown/fade-out) is not implemented** | 3 |
| Cue templates w/ anchors | **done** | Offsets in beats relative to anchors, name/colour/enabled/order, keep-cue-position, overflow trimming, Rekordbox duplicate-memory-cue guard | 3 |
| Custom cue anchors | **done** | Name+colour / name-only / colour-only matching, exactly Lexicon's rules | 3 |
| Emergency loop | partial | A template entry can carry a loop length in beats; **finding a good loop spot** needs the detection work | 3 |

## Files — `06-files.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Watch folder | **done** | Debounced scan rather than a native watcher; settle rule; dismissals | **4** |
| Incoming staging | partial | Watch queue, Selected done with auto-advance + D hotkey; no delete-from-disk | 4 |
| Auto move on done | partial | Move & Rename runs on demand; nothing triggers it, no watch folder | 4 |
| Rename patterns (`%field%`, `{}` optional) | **done** | `crates/file-organizer::pattern`; nesting rejected, renders trimmed | 4 |
| Special subfolder patterns | **done** | Bitrate buckets, first tag, current year/month/decade, plus release decade | 4 |
| Quick move + favourite folders | partial | Remembered folders, favourites, hotkeys 1–9, Send to entry; no picker popup | 4 |
| Write Tags (ID3) | partial | Bulk flow + per-field selection done; no field mappings, no auto-write | 4 |
| Find Unused Files | **done** | Extension filter, DJ-folder skips, path export, deletion record | 4 |
| Local Path Mappings | **done** | Longest-prefix, component-wise, cross-platform separators | 4 |
| Automatic Actions settings group | partial | Group + auto-analyse work; other four disabled with reasons | 4 |

## Health — `07-health.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Find Duplicates | partial | 3 strategies + keep-one archive already. Missing duration bounds, interruptible scan, preselection, bulk Prefer, review step, **playlist re-pointing**, manual merge | 5 |
| Find Lost Tracks / Relocate | partial | Fuzzy filename+size. Missing prefix rewriting, all-tracks mode, extension change, merge-with-existing, pre-change backup | 5 |
| Find Broken Tracks | **missing** | We check existence, never decodability | 5 |
| Find Tags & Album Art | **missing** | `crates/enrichment` is a 10-line stub | 4 |

## Recipes & editing — `10-recipes.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Smart Fixes (10 fixed cleanups) | **done** | 11 fixes with preview/apply diffs — ahead of Lexicon here | — |
| Common-text blocklist UI | partial | IPC exists, **no UI consumes it** | 5 |
| Recipes: casing / field / text / number / tag / other | **missing** | ~30 parameterized ops | 5 |
| Recipes: cue point (11 ops) | **missing** | Depends on the Epic 2 cue model | 5 |
| Recipes: beatgrid (3 ops) | **missing** | Depends on Epic 2 | 5 |
| Import Tags From CSV | partial | CSV parses, but only to match — never writes fields | 5 |
| Undo History | **missing** | We gate before applying; no recourse after | 5 |

## Streaming — `08-streaming.md`

All **missing** except Track Matcher (partial: no `.m3u8`, no separator choice, no playlist
creation, no onward search). Beatport / Beatsource / Tidal / SoundCloud sources, Beatport catalog +
cart + purchase-replacement, Charts, Store Links, Track Discovery, Send To, Transfer Streaming To
Local, Share/export (CSV/M3U/HTML/PDF). **Epic 7**, except Share/export → **Epic 6**.

## History & backup — `09-history-backup.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| DJ app backup before write | **done** | `WriteGuard` — stricter than Lexicon | — |
| History / sessions | **missing** | Snapshot semantics + deleted-set ledger | 6 |
| Database backup / restore (ZIP) | **missing** | Our cache DB holds tags, archive, staged changes — currently unbackupable | 5 |
| Cloud DB backup · Cloud Storage | deferred | Requires accounts + hosting; conflicts with local-first, no-telemetry | — |

## Extensibility — `11-extensibility.md`

Plugins, Local API, Stream Deck — all **deferred**. Keep new capability behind
`agent-tools::ToolRequest` so a future plugin host is a consumer, not a rewrite.

---

## Where `decks` is already ahead

Worth stating, since the matrix is otherwise a list of deficits:

- **Staged-change pipeline.** Every mutation is `Proposed → Accepted/Rejected → Exported/Applied`
  with inline diffs. Lexicon applies immediately and offers 60-minute undo.
- **Write safety.** Refuses to touch `master.db` while Rekordbox holds the WAL lock, plus a
  per-session timestamped backup (ADR-0010).
- **Native Pioneer waveforms.** Real PWAV/PWV3/PWV4/PWV5 rendering rather than a re-decoded
  approximation.
- **Smart Fixes previews.** Per-proposal deselectable diffs before applying.
- **Agent + MCP surface.** `crates/agent-tools` backs the in-app chat, an MCP server (stdio + HTTP)
  and a CLI from one implementation. Lexicon has a JS plugin API; this is a different and, for our
  purposes, more useful axis.
- **No telemetry, no account, fully local.**
