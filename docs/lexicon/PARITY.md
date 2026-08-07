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
| Interop & sync | 2 | 6 | 4 | 11 |
| Library & browser | 0 | 7 | 9 | 0 |
| Smartlists | 0 | 0 | 2 | 0 |
| Analysis | 0 | 5 | 4 | 0 |
| Player, cues, generator | 0 | 4 | 9 | 0 |
| Files | 0 | 3 | 7 | 0 |
| Health | 0 | 3 | 2 | 0 |
| Recipes & editing | 1 | 2 | 4 | 0 |
| Streaming | 0 | 1 | 8 | 0 |
| History & backup | 1 | 0 | 2 | 2 |
| Extensibility | 0 | 0 | 0 | 3 |
| **Total** | **4** | **31** | **51** | **16** |

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
| All smartlists → playlists | **missing** | Option plumbed and **ignored** | 1 |
| Field Mappings | **missing** | No mapping concept at all | 4 |
| Excluded From Sync | **missing** | Name-prefix + tag convention; cheap | 1 |
| Beatshift correction on import/sync | **missing** | Correctness issue — we already write cues | 4 |
| Serato / Traktor / VirtualDJ / Engine / djay / Apple Music / M3U / USB / DIRECT2CDJ | deferred | 11 items | — |

## Library & browser — `02-library.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Virtualized track table | partial | Resizable, sortable, inline column search, multi-select | — |
| Search operators (`None`, `>`, `<`, ranges, `!`) | **missing** | Shared vocabulary with smartlist rules | 1 |
| Tag query language (`~`, `!`, comma) | **missing** | | 1 |
| Key-notation-aware search | **missing** | `4M` should match `Am` | 1 |
| Spreadsheet keyboard navigation | **missing** | | 2 |
| Inline per-row waveform preview | **missing** | | 2 |
| Compatible-key indicator | **missing** | | 2 |
| Track Timeline | **missing** | | 6 |
| Playlists tree | partial | No folder-drop, no M3U import, no drag-between, no create-from-selection | 6 |
| Favorite Playlists + hotkeys | **missing** | | 6 |
| Playlist Merge / Sort / Cross Reference / Prefix / Rewrite Order | **missing** | 5 tools; Rewrite Order is high value for CDJ export | 6 |
| Playlist Occurrence | partial | Only the N=0 case | 6 |
| Custom Tags | partial | Strong already; missing category colours, drag-reorder, OR/AND selection, MyTag import, per-tag hotkeys | 1, 5 |
| Manual multi-track editor | **missing** | `<multiple values>` semantics | 5 |
| Album art | **missing** | Absent from the product entirely | 4 |
| Archive | partial | Missing context-sensitive playlist rule, selection helper, delete-from-disk | 5 |
| Genre / Artist Cleanup | partial | Missing locking, pinned letters, alt-click filter, extra artist fields | 5 |

## Smartlists — `03-smartlists.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Smartlist rules engine | **missing** | Ad-hoc `localStorage` filters only | **1** |
| Smartlist Generator | **missing** | | **1** |

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
| Cue CRUD on the waveform | **missing** | Read-only today; `ChangeKind` has no cue delete | **2** |
| Loops / active loops | **missing** | | 2 |
| Quantize (incl. grid-move-carries-cues) | **missing** | | 2 |
| Cue templates | **missing** | | 2 |
| Beatgrid editing + half/double | **missing** | Parser exists; nothing writes | **2** |
| Beat jump | **missing** | | 2 |
| Hotkeys: rebinding, global, inline hints | partial | Fixed handlers in one hook | 2 |
| **Action registry** | **missing** | The substrate for hotkeys, palette, plugins — see `11-extensibility.md` | **2** |
| Action Center (`Cmd+Space`) | **missing** | | 2 |
| Find Popup (`Cmd+F`) | **missing** | | 2 |
| **Cue Point Generator** | **missing** | Nearest thing is beatgrid-arithmetic intro cues | **3** |
| Cue templates w/ anchors | **missing** | | 3 |
| Custom cue anchors | **missing** | Build first — pure matching, no ML | 3 |
| Emergency loop finder | **missing** | | 3 |

## Files — `06-files.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Watch folder | **missing** | | **4** |
| Incoming staging | partial | Exists; no watcher feeding it, no auto-advance, no hotkey | 4 |
| Auto move on done | **missing** | | 4 |
| Rename patterns (`%field%`, `{}` optional) | **missing** | | 4 |
| Special subfolder patterns | **missing** | Bitrate buckets, first tag, current year/month/decade | 4 |
| Quick move + favourite folders | **missing** | | 4 |
| Write Tags (ID3) | partial | `audio-tags` can write; no bulk flow, no per-field select, no mappings, no auto | 4 |
| Find Unused Files | **missing** | | 4 |
| Local Path Mappings | **missing** | `relocate` solves an adjacent problem | 4 |
| Automatic Actions settings group | **missing** | 5 automations | 4 |

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
