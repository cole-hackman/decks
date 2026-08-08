# Lexicon Parity Matrix

Every Lexicon capability, `decks`'s current status, and the epic that owns closing the gap.

**Status key** — `done` (parity or better) · `partial` (exists but materially incomplete) ·
`missing` · `deferred` (out of scope for this initiative)

**Scope** — the decision is *Rekordbox-deep first*. Non-Rekordbox app adapters are `deferred`
regardless of how much of Lexicon they represent.

---

## Summary

| Domain | done | partial | missing | blocked | deferred |
|---|---:|---:|---:|---:|---:|
| Interop & sync | 7 | 4 | 1 | 0 | 11 |
| Library & browser | 15 | 2 | 1 | 0 | 0 |
| Smartlists | 2 | 1 | 0 | 0 | 0 |
| Analysis | 2 | 4 | 3 | 2 | 0 |
| Player, cues, generator | 10 | 6 | 0 | 0 | 0 |
| Files | 7 | 4 | 0 | 0 | 0 |
| Health | 2 | 1 | 1 | 0 | 0 |
| Recipes & editing | 7 | 1 | 1 | 0 | 0 |
| Streaming | 1 | 1 | 7 | 0 | 0 |
| History & backup | 3 | 0 | 0 | 0 | 2 |
| Extensibility | 0 | 0 | 0 | 0 | 3 |
| **Total** | **56** | **24** | **14** | **2** | **16** |

The shape of the work: library *hygiene*, *editing* and *set preparation* are broadly covered.
What is thin is *automation* — nothing runs unprompted except auto-analyse — and *enrichment*,
where `crates/enrichment` is still a stub and album art does not exist at all.

**How to read these numbers.** They are self-reported against a matrix written from Lexicon's
manual, not from Lexicon itself. Three specific limits apply:

- The denominator may be short. `lexicondj.com/features` has never been readable from this
  environment (HTTP 403 — see [`GAPS.md`](GAPS.md) gap 1), and it enumerates named features the
  manual does not.
- Nothing here has been checked against a running Lexicon or a real Rekordbox library. Every
  fixture in the repository is synthetic; real ones are gitignored.
- `done` means "parity or better *on the behaviour the manual describes*". Several `done` rows
  carry a divergence in their Notes — read the row, not the count.

`blocked` means the row cannot be built as specified, for a reason outside the codebase — not that
it is merely unbuilt. Two rows sit there: the Camelot/Open Key posture is a licensing decision
rather than code, and Danceability / Popularity / Happiness depend on a Spotify endpoint that was
withdrawn in 2024 (ADR-0012), with Popularity uncomputable locally under any circumstances.
`Colors → nearest` left this column when `Track` gained a colour field and is now `done`.

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
| Key conversion | **done** | Camelot + Open Key, both directions, plus the leading-zero Sync option. Notation posture still open | 6 |
| Colors → nearest | **done** | `Track` carries colour; `TrackMetadataEdit` writes `ColorID` against Rekordbox's fixed eight-colour palette. Off, an inexact colour is **left unchanged** and the skip is reported; on, it maps to the nearest and each mapping is named. Never creates a `djmdColor` row — a ninth colour renders on no CDJ | 4 |
| All smartlists → playlists | **done** | Materialises via `PlaylistCreate` + `PlaylistAddTrack`, staged before the change set is collected | 1 |
| Field Mappings | **done** | Engine, ID3 profile **and** a Rekordbox profile applied to the library. Per-category tag sources and the Colour source populated. Library mappings are **previewed and staged**, not written directly — a mapping rewrites Comment or Genre library-wide, and that goes through review like every other bulk edit. Profiles for non-Rekordbox apps stay `deferred` with the rest of those adapters | 4 |
| Excluded From Sync | **done** | Name-prefix (case-insensitive) and custom-tag conventions, both honoured during materialisation | 1 |
| Beatshift correction on import/sync | **missing** | Correctness issue — we already write cues | 4 |
| Serato / Traktor / VirtualDJ / Engine / djay / Apple Music / M3U / USB / DIRECT2CDJ | deferred | 11 items | — |

## Library & browser — `02-library.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Virtualized track table | partial | Resizable, sortable, inline column search, multi-select, inline cell editing. Label / Mix / Remixer / Colour / Added columns added with the `Track` widening | — |
| Search operators (`None`, `>`, `<`, ranges, `!`) | **done** | `smartlists::search` parses the box into rules the same evaluator runs — one implementation, not two | 1 |
| Tag query language (`~`, `!`, comma) in the search box | **done** | `~a,b` requires all, `tag:a,b` any, `!` negates — parsed to `has_all` / `has_any` / `has_none` | 5 |
| Key-notation-aware search | **done** | `key:4A` finds `Abm`: the box parses to a key rule, and the evaluator does the notation work | 1 |
| Spreadsheet keyboard navigation | **done** | Cell cursor, arrows/jumps/Home/End/page keys, shift-extend from an anchor, Tab within the row, inline edit that stages rather than writes. Clamps, never wraps | 6 |
| Inline per-row waveform preview | **done** | `Wave` column from the ANLZ preview, downsampled to 40 bars in Rust, batched per visible page and cached. Absence renders as nothing, not a flat line | 6 |
| Compatible-key indicator | **done** | A dot on keys that mix out of the selected track, following the global Key Mixing Mode. Positive mark only | 2 |
| Sidepanel (second track browser) | **done** | Resizable, toggled from the header or `Cmd/Ctrl+\\`; keeps its own selection so it is a second view rather than a mirror | 6 |
| Track Timeline | **done** | BPM / Energy / Rating / Key, coloured by key or BPM change; hidden past 200 tracks; also on history sets. Danceability / Popularity / Happiness not modelled | 6 |
| Playlists tree | partial | M3U import and create-from-selection done. Still no folder-drop and no drag-between | 6 |
| Favorite Playlists + hotkeys | **done** | Star up to 9; bar above the browser, 1–9 opens and Shift+1–9 files the selection. Drag-and-drop target not done — no drag source in the table yet | 6 |
| Playlist Merge / Sort / Cross Reference / Prefix / Rewrite Order | **done** | All five, in a Playlist Tools view. Sort needed a new `PlaylistReorder` change kind. Rewrite Order sorts on a field picked in the tool rather than the browser's transient column sort — documented divergence | 6 |
| Playlist Occurrence | **done** | Any N, in Playlist Tools. Counts distinct playlists, and ships the whole distribution so N does not have to be guessed | 6 |
| Custom Tags | **done** | OR-within/AND-across selection, MyTag and hashtag import, category colours, drag **and keyboard** reorder (`reorder_tags`), per-tag number hotkeys (global, so assigning a taken one steals it), and Field-Mapper export both for all tags and per category | 5 |
| Manual multi-track editor | **done** | `<multiple values>` as a placeholder; untouched fields never written. Album art out of scope | 5 |
| Album art | **missing** | Absent from the product entirely | 4 |
| Archive | **done** | Context-sensitive playlist rule, selection helper, staged cleanup. Delete-from-disk is now a separate button, never a side effect of cleanup | 5 |
| Genre / Artist Cleanup | **done** | Locking, pinned letters, alt-click filter, sort modes. Remixer is now modelled; composer and original-artist still are not | 5 |

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
| Danceability / Popularity / Happiness | **blocked** | Lexicon sources all three from Spotify's `audio-features` endpoint, deprecated 2024-11-27 and 403 for applications registered since (ADR-0012). Danceability is approximable from onset density; **Popularity is a catalog metric that cannot be computed locally at all**. Previously mislabelled `missing`, which implied it was merely unbuilt | 4 |
| Auto-analyze on add | **missing** | | 4 |
| Mixable Tracks | **done** | Panel reachable from the track context menu and the header; 11 of 13 rules, `Use as next track`, saveable templates. `Match colour` and `Recently added` ship now that `Track` carries the fields. The 2 that remain (Popularity/Danceability/Happiness) are blocked upstream, not unbuilt | 4, 6 |
| Key Mixing Mode | **done** | Global setting; Harmonically Compatible and Fuzzy, shared with the compatible-key set the panel shows | 6 |
| Beatshift Fixer | **missing** | | 4 |

## Player, cues, generator — `05-cues-player.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Playback + waveform scrub | partial | `rodio`; no queue, no autoplay | 2 |
| Play queue | **done** | Add to queue / Play next, reorder, shuffle-upcoming-only, clear-but-keep-playing, autoplay on `playback-ended`. Per-session and in-memory by design | 6 |
| Cue CRUD | **done** | Set/play/delete/move/colour via `CueEditor`, all through staged changes. New `ChangeKind::TrackDeleteCue`. Placement is on the cue list + slot grid, not yet drag-on-waveform | 2 |
| Loops | partial | Loop length in beats via `OutMsec`. **Active loops need a `djmdCue` column we do not model** — deferred | 2 |
| Quantize (incl. grid-move-carries-cues) | **done** | 1/2/4/16/64-beat snapping; a grid nudge moves only cues already on the grid | 2 |
| Cue templates | **done** | Ship as *cue presets* — `CueTemplate` was taken by the generator. Immutable, promoted from a cue, applied as staged `CueMetadataEdit`s, hotkeys 1–8 with gap-closing on delete | 6 |
| Beatgrid editing | partial | Grid nudge stages the cue moves that follow it. Writing the grid itself back to ANLZ, and half/double BPM, still missing | 2 |
| Beat jump | **done** | ±4/±16 beats along the real ANLZ grid, clamped at both ends | 2 |
| Hotkeys: rebinding, global, inline hints | partial | Rebinding + persistence + conflict detection exist in the registry; no settings UI yet, and no system-wide hotkeys | 2 |
| **Action registry** | **done** | `lib/actions.ts` — bindings, rebinding, conflict detection, search. App globals migrated onto it | 2 |
| Action Center (`Cmd+Space`) | **done** | Palette over the registry with fuzzy search and arrow navigation | 2 |
| Find Popup (`Cmd+F`) | **done** | Playlists + smartlists + tracks in one box, per-section caps, tiered (not fuzzy) ranking, per-result queue / add-selection actions | 6 |
| **Cue Point Generator** | partial | Template engine + custom cue anchors ship; **structural detection (drop/breakdown/fade-out) is not implemented** | 3 |
| Cue templates w/ anchors | **done** | Offsets in beats relative to anchors, name/colour/enabled/order, keep-cue-position, overflow trimming, Rekordbox duplicate-memory-cue guard | 3 |
| Custom cue anchors | **done** | Name+colour / name-only / colour-only matching, exactly Lexicon's rules | 3 |
| Emergency loop | partial | A template entry can carry a loop length in beats; **finding a good loop spot** needs the detection work | 3 |

## Files — `06-files.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Watch folder | **done** | Debounced scan rather than a native watcher; settle rule; dismissals | **4** |
| Incoming staging | **done** | Watch queue, Selected done with auto-advance + D hotkey, archive, and delete-from-disk as the third triage outcome | 4 |
| Auto move on done | partial | Move & Rename runs on demand; nothing triggers it, no watch folder | 4 |
| Rename patterns (`%field%`, `{}` optional) | **done** | `crates/file-organizer::pattern`; nesting rejected, renders trimmed | 4 |
| Special subfolder patterns | **done** | Bitrate buckets, first tag, current year/month/decade, plus release decade | 4 |
| Quick move + favourite folders | partial | Remembered folders, favourites, hotkeys 1–9, Send to entry; no picker popup | 4 |
| Write Tags (ID3) | partial | Bulk flow + per-field selection done; no field mappings, no auto-write | 4 |
| Find Unused Files | **done** | Extension filter, DJ-folder skips, path export, deletion record | 4 |
| Local Path Mappings | **done** | Longest-prefix, component-wise, cross-platform separators | 4 |
| Delete from disk | **done** | Quarantine + manifest rather than `unlink`; seven refusals, one overridable; fail-closed on unconfigured music folders; `purge` is the separate irreversible step | 6 |
| Automatic Actions settings group | partial | Group + auto-analyse work; other four disabled with reasons | 4 |

## Health — `07-health.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Find Duplicates | **done** | 3 strategies, duration bounds, preselection, bulk Prefer, review step, playlist re-pointing. Interruptible scan and manual merge outstanding | 5 |
| Find Lost Tracks / Relocate | partial | Fuzzy match + prefix rewriting, all-tracks mode, extension change; backup rides on `WriteGuard`. Merge-with-existing and the 5-min re-check cadence outstanding | 5 |
| Find Broken Tracks | **done** | Real decode check, two depths, per-playlist report. Deleting from disk deliberately not offered | 5 |
| Find Tags & Album Art | **missing** | `crates/enrichment` is a 10-line stub | 4 |

## Recipes & editing — `10-recipes.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Smart Fixes (10 fixed cleanups) | **done** | 11 fixes with preview/apply diffs — ahead of Lexicon here | — |
| Common-text blocklist UI | **done** | Settings → Remove Common Text, with the manual's two presets | 5 |
| Recipes: casing / field / text / number | **done** | 18 ops in `crates/recipes`, preview-then-stage | 5 |
| Recipes: tag | **done** | Import from text (idempotent), add/remove/replace/clear | 5 |
| Recipes: other (3 ops) | **done** | Mark as Incoming, Remove from All Playlists, Import Date from Filesystem | 5 |
| Recipes: cue point (11 ops) | partial | 9 of 11 in `crates/recipes::cues`; Change Active Loops and Half/Double BPM need an unmodelled column and ANLZ writes | 5 |
| Recipes: beatgrid (3 ops) | **missing** | Depends on Epic 2 | 5 |
| Import Tags From CSV | **done** | `csv_import`: Location or Artist+Title matching, per-row report, stages edits | 5 |
| Undo History | **done** | Sync runs record inverses; undo re-stages them for review. Kept for 50 runs rather than 60 minutes | 5 |

## Streaming — `08-streaming.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| Share / export (CSV, M3U, HTML/PDF, quick copy) | **done** | `crates/share`, in Playlist Tools. CSV formula injection defused; M3U reports the pathless tracks it could not carry; HTML self-contained, PDF via the browser. Header drag-to-reorder not done — the picker orders by tick order | 6 |
| Track Matcher | partial | No `.m3u8`, no separator choice, no playlist creation, no onward search | 7 |

Everything else is **missing** and belongs to **Epic 7**: Beatport / Beatsource / Tidal /
SoundCloud sources, Beatport catalog + cart + purchase-replacement, Charts, Store Links, Track
Discovery, Send To, Transfer Streaming To Local.

## History & backup — `09-history-backup.md`

| Feature | Status | Notes | Epic |
|---|---|---|---|
| DJ app backup before write | **done** | `WriteGuard` — stricter than Lexicon | — |
| History / sessions | **done** | Snapshot tables (migration **v17**), idempotent import, deleted-set ledger, rating + location, save-as-playlist with a labelled re-match | 6 |
| Database backup / restore (ZIP) | **done** | JSON rather than ZIP: inspectable and schema-tolerant. Analysis caches excluded; nothing auto-deleted | 5 |
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
