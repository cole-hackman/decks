# Status

## 2026-08-06 — Epic 5 (part 1): the Recipes engine

New `crates/recipes` — the *other* bulk-editing system. `crates/smart-fixes` is ten fixed,
zero-parameter cleanups; a recipe takes parameters, and the user assembles the one they need. The
casing, field, text and number categories are done: **18 operations**, every one a pure function of
(recipe, fields) → fields, with no database, no filesystem and no notion of what a track is. That
keeps the whole vocabulary testable in isolation and lets one engine back the preview, the apply
pass and, later, an agent tool.

**The casing recipes exist because of one parameter.** `fix_casing` hardcodes its
article/preposition list; a recipe takes the user's own words-to-ignore. A library full of `EDM`,
`NYC` and `DJ` needs those protected, and no hardcoded list will ever contain them.

Three rules the spec leaves open, decided and tested:

- A recipe whose source is empty **reports why** — `SourceEmpty`, `NoMatch`, `NotANumber`,
  `Misconfigured`. "340 of 400 changed" needs an explanation attached; silence reads as a bug.
- `Merge Fields` with one half missing yields the other half, not a stray separator.
- `Extract Text` with no match leaves the target **untouched**. Writing an empty string would blank
  a good remixer field, which is worse than not running at all.

Smaller decisions worth recording: `AdjustNumber` keeps an integer looking like an integer (bumping
a track number from 3 must give `4`, not `4`); casing preserves original spacing rather than
round-tripping through `split_whitespace`; and `RemoveBetween` collapses the gap it leaves, since
`"Track  Live"` with a double space is exactly what a cleanup recipe should not produce.

The field vocabulary offered by the UI is deliberately the intersection of what `decks` models and
what the applier's allowlist will actually write — a test enforces it. Offering a field that cannot
be persisted would produce a preview full of changes that silently vanish at sync time.

`RecipesPanel` (sidebar → **Recipes**) builds an ordered list, previews every change as a
deselectable before/after row, and stages what survives. Recipes serialise, so one built today can
be saved and replayed on next month's downloads — the point of the feature.

**The tag recipes** ship alongside, in `crates/recipes::tags`. They are modelled as a *delta* to
the track's tag set rather than a new value — which is what the cache's add/remove accessors want,
and what lets a preview say "adds 3, removes 1" instead of showing two lists.

`Import Tags from Text` is the one the spec singles out, and it is idempotent in two senses: a tag
the track already has is not re-added, and nothing existing is ever removed, so a hand-added tag
survives a re-run. Matching is case-insensitive, because a library holding both `#techno` and
`#Techno` is exactly the mess the feature exists to clean up. A tag runs from the marker to the
next whitespace, matching how the convention is written (`#PeakTime`, not `#Peak time`).

Two more rules the spec leaves open: replacing a tag with one the track already has is a removal
*only*, or it ends up holding it twice; and replacing with an empty tag is refused rather than
silently becoming a delete.

Tag recipes apply directly rather than staging — tags live in the local cache, so there is no sync
step to carry them. A tag name with no existing tag is created in the first category, and the result
reports which were invented.

**The three "other" recipes** close the category. Each reaches into a different subsystem — Incoming
state, playlists, the filesystem — so they run one at a time rather than joining the ordered recipe
list, and the UI states what each does *before* it runs. `Remove from All Playlists` leaving
smartlists alone is exactly the sort of thing that otherwise reads as the recipe having missed some.

`Import Date from Filesystem` takes the file's **modification** time, not creation time: creation
time is not portable (Linux has no reliable `birthtime`), and a file copied between drives keeps its
mtime while its ctime becomes the copy date — worse than useless as a release year. `Mark as
Incoming` is the exact inverse of `Selected done`, clearing the per-track reviewed flag added in
migration v12.

**Not done:** the cue (11) and beatgrid (3) recipes, which operate on cue lists rather than text and
need the quantize arithmetic from `crates/rekordbox-db`.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 390, typecheck, lint, `pnpm e2e` 20 — all green.

## Blockers — verified, not assumed (2026-08-06)

The three Epic 4 items still open cannot be built *in this environment*, for reasons that are about
the environment rather than the design:

| Item | Blocker | Evidence |
|---|---|---|
| Enrichment (Find Tags, album art) | Outbound egress to metadata APIs is blocked. | `curl https://musicbrainz.org/ws/2/...` returns `CONNECT tunnel failed, response 403`. Writing response parsers against invented JSON shapes would be untestable code in a production path. |
| Energy / Danceability | No audio to calibrate against. `fixtures/audio/` holds only `.gitkeep`; real fixtures are gitignored. | An uncalibrated 1–10 scale would be a number we could not defend, which is exactly what ADR-0008 exists to prevent. |
| Beatshift Fixer | Both of the above, plus a re-encoding dependency. | Detecting beatshift needs real encoder-padded MP3s; fixing it needs a re-encoder. |

None of these is deferred by preference. Each needs either network access or a real (gitignored)
fixture library, both of which are available on a developer machine and not here. The design work
for all three is recorded in `docs/lexicon/` and `docs/ROADMAP.md`.

## 2026-08-06 — Epic 4 (part 2): Field Mappings

`crates/changes::field_mappings` — the projection engine for fields the target does not have.
Energy, Danceability and Custom Tags have no Rekordbox column and no standard ID3 frame; a mapping
writes them somewhere that does. It lives in `changes` rather than beside Write Tags because the
same `Energy → Comment` rule must produce the same string whether it lands in `master.db` or in an
ID3 frame — one implementation, two call sites.

Spec semantics: source → target, overwrite replaces while off appends, several sources on one
target combine with `, `, custom tags write hashtag form, and a colour source writes the colour
*name* since a text target cannot use a hex value.

Three rules the spec leaves open, decided and tested:

- A track with no value for a source contributes **nothing** — not `Energy` with no number after
  it, and not a blanked target.
- Numbers are zero-padded to two digits, so a text target sorts them correctly. Same reason Key
  Conversion has a leading-zero option.
- Where several mappings share a target, the **first** decides overwrite-vs-append. Mixing the two
  on one target is a configuration mistake; first-wins is predictable and matches reading order.

**Cache migration v11 drops the dead `field_mappings` table from v5.** Nothing ever read or wrote
it, and its `(library_path, source_field)` primary key allowed exactly one target per source —
which cannot express combining, the feature's most useful half. The replacement is scoped by
*profile* rather than library path, because mappings are configured per destination.

Write Tags honours them, with two guards: mappings only fill targets the per-field selection did
**not** claim (quietly replacing a field the user explicitly ticked would be a nasty surprise), and
a mapping onto a field audio files do not have produces a warning rather than silently vanishing.

`FieldMappingsSection` in Settings configures the ID3 profile. Per-DJ-app profiles and applying
mappings during sync are outstanding; the schema is ready for both.

**Incoming `Selected done`** ships alongside (cache migration **v12**). The manual is right that
auto-advance is the detail that makes triage fast: marking the selection reviewed immediately
selects the next track, so an inbox clears with one repeated `D` instead of a click-and-reach cycle
per track. Two details that had to be got right — the next track is chosen from the list as it stood
*before* removal, so it is the one that visually followed what the user was looking at; and
advancing is skipped entirely when marking failed, because advancing past a track that is still in
the inbox would lose it.

The existing `incoming_watermark` could not express this: it answers "what arrived since I last
cleared", which is all-or-nothing. Per-track review state is a separate table, filtered out
alongside archived tracks.

**Send to → Move files…** is now on the track context menu, scoped to the current multi-selection
when the right-clicked track is part of it, so "send these twelve" works as well as "send this one".
It carries a `Moves on disk` hint — it is the only context-menu entry that touches the filesystem,
and that should not be a surprise.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 354, typecheck, lint, `pnpm e2e` 17 — all green.

## 2026-08-06 — Epic 4 (part 1): Move & Rename

New `crates/file-organizer`, three pure layers with no filesystem access anywhere in the crate:

**`pattern`** — the `%field%` template language. `%field%` interpolates, literal text passes
through, and `{ … }` marks an optional segment that is emitted only when *every* field inside it
has a value. That last construct is the whole point: `%artist% - %title% (%key%)` on a keyless
track leaves `Daft Punk - Get Lucky ()`, while `{(%key%)}` leaves nothing behind. Renders are
trimmed, because a dropped segment nearly always strands the space that separated it. Optional
segments deliberately do not nest — one level covers every documented use, and rejecting nesting
gives a clear error rather than a surprise. Every worked example from the manual is a test.

**`subfolder`** — up to three nested levels, each independently optional, plus the computed
patterns: bitrate as `320+`/`320-` buckets rather than raw numbers, first tag, current year, current
month zero-padded, current decade as a range. **An empty field drops its level, not the move** — a
track with no genre still lands in the target folder, one level shallower. Anything else orphans
files. A missing bitrate drops the level rather than defaulting into `320-` and mislabelling a
lossless file. Field values are sanitised per component, so `Drum & Bass / Jungle` is one folder
and cannot invent a second level.

**`plan`** — combines the two into destination paths, taking an existence oracle as an argument so
collisions, no-op moves and empty renders are all unit-testable. Collisions suffix ` (2)`, both
against the filesystem and against destinations claimed earlier in the same batch: two tracks can
legitimately render to the same name, and overwriting one with the other destroys audio. A render
containing nothing but punctuation falls back to the existing filename, so an untagged track never
becomes `-.mp3`.

**New `ChangeKind::TrackRelocate`.** Moving files is a filesystem operation; telling Rekordbox
where they went is a staged change like everything else. The applier writes `FolderPath`, and
writes `FileNameL`/`FileNameS` **only if the database has them** — detected per-database with
`PRAGMA table_info` rather than assumed, since `decks` does not model those columns.

This also fixes a live bug: `RelocateBanner` staged path updates as
`TrackMetadataEdit { field: "folder_path" }`, which is not a `djmdContent` column and was rejected
by the applier's allowlist — so relocations staged from the UI never actually applied. It now
stages `TrackRelocate`. The frontend `ChangeKind` union was also missing `TrackDelete`,
`TrackAddCue` and `TrackDeleteCue`; it now mirrors the Rust enum.

Four Tauri commands in `src-tauri/src/organizer.rs`. `pattern_fields` returns the manual's full
28-field vocabulary with a `supported` flag, so the editor can say "decks cannot fill remixer yet"
instead of quietly rendering blanks. `preview_organize` plans without touching anything;
`apply_organize` executes exactly the rows it is handed back, so what runs is what the user read.
A file that cannot be moved fails alone — one locked file must not abandon the other 500.
`fs::rename` falls back to copy-then-remove for cross-filesystem moves, which is the common case
(downloads on the internal disk, library on an external one).

New `OrganizeFilesView`, reachable from the sidebar as **Move & Rename**, acting on the selection
or the whole library. Preview lists rows that would *not* change too, and the success toast says a
sync is still needed — per the manual, a partial sync leaves the old locations behind.

**Find Unused Files** ships in the same crate, because it is the same concern from the other side:
which files on disk the library does not account for. Include/exclude extension filtering (an empty
list means "no filter" in either mode, since an empty include list would report nothing and look
broken), the DJ-folder skip list matched case-insensitively, `Copy paths` to export without
deleting, and a timestamped record of every deletion under the app data folder.

Its output is a list of deletion candidates, so it carries three guards the manual does not
specify. A scan against an **empty library refuses to run** — everything would look unused. Library
membership is **re-checked at delete time**, because the library can gain a track between the scan
and the click. And path comparison is case- and separator-insensitive, since Rekordbox and the
filesystem do not reliably agree and a case-only mismatch would offer a real track for deletion.
Nothing is pre-selected and deletion needs an explicit second click.

**Bulk Write Tags** rounds out the slice: `write_tags_bulk` projects the library's values into the
files' own tags, with per-field selection, over the selection or the whole library. The rule that
matters is the one the manual does not state — **a selected field whose library value is empty is
not written**, because ticking "Artist" on a library that happens not to know one would blank a
perfectly good tag in the file. Those tracks come back as `skipped` and the panel says how many.
Nothing is ticked by default: this writes to files and cannot be rolled back through the staged-
change pipeline.

The sidebar entry is now **Files** rather than Move & Rename, with Move & Rename, Write Tags and
Find Unused Files as sections — they are one domain (things that write to disk rather than to
Rekordbox's database) and `docs/lexicon/06-files.md` treats them as one.

**Local Path Mappings** (cache migration **v8**) close the slice. Per-computer prefix rewrites so a
library restored on a second machine finds its music without a bulk relocate. Longest matching
prefix wins; matching is on whole path components, so `/Music` cannot swallow `/MusicVideos`;
separators are interchangeable because these databases cross platforms; and matching is
case-insensitive while the remainder keeps its original case — the comparison has to be lenient,
the filesystem may not be.

Read-side only. The library keeps saying `D:\Music\…`, which is exactly what lets one database
work on two machines at once. The table is deliberately **not** keyed by `library_path`: a mapping
describes where this *computer* keeps its music and has to apply the moment any library is opened.
Recorded as **ADR-0014**, since it breaks the pattern every other post-v5 table follows.

Applied at every point that turns a stored path into a real one: the missing-file scan (a mapped
track is not missing), Move & Rename's sources, Write Tags, and — the one that matters most — the
unused-file sweep's known-path set, since without it every mapped track would look unused and land
on the delete list.

**Quick move** (cache migration **v9**) closes the slice: remembered destinations, favourites first,
hotkeys 1–9. Recording a folder is an upsert, so using the same one twice moves it up the list
rather than duplicating it, and the hotkeys are ignored while a text field has focus — otherwise
typing a path fires a move on every digit. The move itself reuses the Move & Rename planner, so
collisions and `TrackRelocate` staging behave identically, and the success message repeats the
full-sync warning the manual is emphatic about.

**Watch folders** (cache migration **v10**) complete the automation story, with one deliberate
substitution: arrivals are found by a **debounced scan** every 15 seconds rather than a native
filesystem watcher. That makes the arrival set a pure function of (files on disk, library,
dismissed) — testable without an event loop, unable to miss something that happened while the app
was closed, and free of a platform-specific dependency. A push-based watcher would sit behind the
same function and change nothing the user sees.

Two rules the manual does not state, both earned rather than assumed. A file whose modification
time is under **10 seconds** old is held back and reported separately: a large FLAC copied over a
network share exists on disk long before it is complete, and importing mid-copy reads truncated
tags and a wrong duration. And dismissals are recorded, so a file the user chose not to import is
not offered again on every scan.

**New `ChangeKind::TrackCreate`, and it is export-only.** Inserting a row into `djmdContent` needs
columns `decks` does not model and cannot verify against a real fixture, and a half-populated row
in a performing library is worse than no row. So the applier **refuses** it — with a message naming
the file and pointing at the XML route — and the exporter emits the new tracks into the collection,
continuing IDs past the highest existing one so an import can never silently replace a real track.

**Automatic Actions** closes the epic's automation story, honestly. The settings group exists and
**Auto Analyze New Tracks** works: importing an arrival detects BPM and key on the way in, and a
failed analysis does not undo an import that already succeeded. The other four are shown as
**disabled toggles that state what they need** — automatic drop detection, the Beatshift Fixer,
field mappings, the enrichment providers — rather than hidden or offered as switches that quietly
do nothing. A switch that does not switch anything is worse than one that says why it is off, and
hiding them would make the gap invisible. An unavailable action also reads as off at the point of
use regardless of what is stored, so a setting enabled before its feature regressed cannot silently
take effect.

**What is NOT done in Epic 4:** incoming auto-advance and its hotkey, delete-from-disk, quick
move's context-menu entry point, field mappings, auto-write-on-change, enrichment (Find Tags &
album art), Energy/Danceability, and the Beatshift Fixer.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 339, typecheck, lint, `pnpm e2e` 17 — all green. One CI-only clippy lint
(`unnecessary_sort_by`, 1.97) had to be fixed after the fact: the container's toolchain is 1.94, so
`cargo clippy` passing locally does not guarantee CI.

## 2026-08-06 — Epic 3 (part 1): cue templates and custom cue anchors
New `crates/cue-generator`, split in two so detection can land later without touching the placement logic:

**`anchor`** — the structural landmarks a template hangs off (`Start`, `Drop{ordinal}`, `Breakdown{ordinal}`, `FadeOut`, `End`) and `resolve_custom_anchors`, which maps the user's *existing* cues onto them. Lexicon's matching rules exactly: name+colour requires both, name-only takes the first cue with that name regardless of colour, colour-only takes the first with that colour regardless of name, and a rule with neither matches nothing rather than acting as a wildcard. Name matching is trimmed and case-insensitive. Anchors carry `Confidence`; a user-placed one is `Certain` because a human put it there.

**`template`** — declarative placement in beats relative to anchors, with name, colour, enabled and order. Offsets walk the beat grid rather than multiplying by a constant tempo, so they stay correct across tempo changes, falling back to arithmetic only past the end of the grid. Also: `keep_cue_position` (slot = template row, so "drop is always cue 1" survives a skipped row), overflow trimming that drops the **least confident first** then the latest in the track, and the **Rekordbox duplicate-memory-cue guard** — Rekordbox silently discards the second of two memory cues at one position, so we discard it ourselves and say which.

Nothing is silently dropped: every omission comes back as a typed `SkippedCue` (`AnchorMissing`, `OutOfRange`, `Overflow`, `DuplicateMemoryCue`) that the UI renders as a sentence.

Three Tauri commands in `src-tauri/src/cue_generator.rs`; preview and apply share one `generate` function so what the user reviews is exactly what gets staged. `suggest_anchor_rules` guesses anchors from cue names ("Drop", "Break", "Outro"/"Fade") so the panel opens with something to edit. `CueGeneratorPanel` is mounted in `TrackDetailPanel`.

**Honest labelling, per ADR-0008.** `Confidence` rides from anchor → cue → UI. Low-confidence cues render as `provisional NN%`, and the panel states plainly that automatic drop detection is not implemented rather than implying an analyser is running.

**What is NOT done:** structural segmentation — the actual drop/breakdown/fade-out detection — is the remaining half of Epic 3. Everything above is the placement machinery it will feed. Also outstanding: breakdown-min-beats, drop-at-start and auto-generate-on-play (all detection inputs), and finding a good emergency-loop spot.

One robustness fix found by the E2E suite rather than by unit tests: `CueGeneratorPanel` assumed `suggest_anchor_rules` returns an array. Two existing specs whose IPC mock returns `null` for unknown commands took the whole track panel down. Now guarded.

Verification: `cargo test --workspace` 725 passing, clippy `-D warnings` clean, `cargo fmt --check` clean, `pnpm test` 279, typecheck, lint, `pnpm e2e` 11 — all green.

## 2026-08-06 — Epic 2 (part 1): action registry, cue editing, quantize, beat jump
**Action registry** (`apps/desktop/src/lib/actions.ts`) — the substrate the rest of Epic 2 registers into. Every global capability is a named, bindable command; the Action Center searches that list, hotkeys bind to it, and inline hints read from it. Pure module covering binding serialisation, display formatting, matching (Cmd and Ctrl interchangeable), persisted user rebinding, conflict detection and fuzzy search. `ActionProvider` owns a single global key listener reading the action list through a ref so it installs once. App's four global shortcuts migrated onto it; component-local key handling (track-table arrows) deliberately stays in `useKeyboardShortcuts`. `ActionCenter` is the `Cmd/Ctrl+Space` palette.

**Cue editing.** New `ChangeKind::TrackDeleteCue` + applier (deleting a missing cue errors rather than silently succeeding — it means the staged change was built against a stale view). New `crates/rekordbox-db/src/quantize.rs`: pure beat-grid arithmetic — nearest beat, snap at 1/2/4/16/64-beat resolutions measured from the first grid marker, `is_on_grid`, beat jump clamped at both ends, and `cues_following_grid`, which returns only the cues that were already on the grid. That last one is the subtle bit: a grid nudge must move on-grid cues and leave deliberately off-grid cues alone. Seven Tauri commands in `src-tauri/src/cues.rs`, all staging changes rather than writing `master.db`. `CueEditor` is mounted in `TrackDetailPanel`, so cues are editable from the UI: 1–8 set or play, Cmd/Ctrl+1–8 delete, Q toggles quantize, plus colour, loop length in beats, move-to-playhead, beat jump and grid nudge.

Quantising an existing loop shifts its out-point by the same delta as its in-point, so loop *length* is preserved rather than stretched.

**Not done in this slice, and why** — recorded in ROADMAP and PARITY rather than left implicit:
- **Active loops are blocked**, not deferred by choice: they need a `djmdCue` column our schema does not model. Loop length works; auto-engaging loops do not.
- **Beatgrid writing** (ANLZ) and half/double BPM: the grid nudge stages the cue moves that *follow* a grid change, but nothing writes the grid itself yet.
- Cue placement is on the cue list and slot grid; dragging cues on the waveform is not implemented.
- Find Popup, play queue, cue templates, and the Cue Destination round-trip remain.
- Hotkey rebinding exists in the registry (with persistence and conflict detection) but has no settings UI, and there are no system-wide hotkeys.

Verification: `cargo test --workspace` 698 passing, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `pnpm test` 269, `pnpm typecheck`, `pnpm lint`, `pnpm e2e` 11 — all green.

## 2026-08-05 — Epic 1: Smartlists engine
New `crates/smartlists` implementing the rule model from ADR-0013: a **two-level** structure (AND of OR-clauses) rather than a general boolean tree, matching what Lexicon actually exposes — OR grouping is only offered in "All Rules" mode. Evaluation is pure and in-memory over `&[Track] + EvalContext`, where `EvalContext` mirrors the frontend `FilterContext` so both sides share semantics. Key equality canonicalises through `changes::key_format` plus a new Open Key parser, so `8A`, `8a`, `8m`, `Am` and `A minor` all match the same track. Archived tracks are excluded unless a rule mentions `IsArchived`; an empty rule set matches nothing rather than the whole library. Also ships the Smartlist Generator (by field / tag category / decade / BPM range / play count) with `only_missing` as the idempotency guard, and a 30-second recompute throttle with injected time.

Cache migration **v7** adds a `smartlists` table storing rules as a JSON document. Normalised clause/rule tables were dropped from the plan: evaluation is in-memory so nothing ever queries an individual rule, and `staged_changes` already stores JSON in this DB. `parent_folder_id` doubles as the generator's ledger — generated smartlists sit in the reserved `Lexicon` folder and moving one out is what makes the generator recreate it, so no extra bookkeeping state exists.

Sync: `SyncOptions.all_smartlists_to_playlists` is now honoured (it had been accepted and ignored since Sub-Plan 6). `sync_execute` stages `PlaylistCreate` + `PlaylistAddTrack` for each smartlist before collecting the change set. Playlist IDs derive from the smartlist ID rather than being freshly generated, because the applier requires `target_id` to be stable between preview and apply. `Excluded From Sync` conventions implemented for both names (case-insensitive prefix) and the custom tag. `rekordbox_compatibility` encodes what Rekordbox 6/7 can hold — MyTag only, `has all`/`has none` only, ≤2 rules and ≤4 categories — and drives the editor's indicator.

Nine Tauri commands in `apps/desktop/src-tauri/src/smartlists.rs`; `SmartlistList` / `SmartlistEvaluate` added to `agent-tools::ToolRequest` and MCP. Frontend: `SmartlistsView.tsx` (list + rule editor + generator) with live match count, wired into `SidebarNav`.

Deviations from the plan, all deliberate: (1) evaluation is in-memory rather than the SQL/in-memory hybrid ADR-0013 proposed — an **Implementation note** amending the ADR explains why; (2) smartlists ship as a dedicated view rather than inside the playlist tree, since Rekordbox playlists come from `master.db` and smartlists from the cache DB — merging the trees is deferred to Epic 6; (3) the tag query language for the *search box* and the Tags-page OR/AND selection are deferred to Epic 5 — the rule engine already expresses both.

Also found: `field_mappings` and `sync_runs` were created in migration v5 and are **read and written by nothing**. `field_mappings` is additionally the wrong shape — its `(library_path, source_field)` PK cannot express multi-source combining or per-app mappings, so Epic 4 should replace rather than extend it. Recorded in `docs/data-model.md` and `PARITY.md`.

Verification: `cargo test --workspace` (incl. `decks-desktop`), `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm test` (224), `pnpm typecheck`, `pnpm lint`, `pnpm e2e` (11) — all green. GTK/webkit/ALSA dev packages were installed in the dev container, so the Tauri shell and Playwright now run locally instead of only in CI.

## 2026-08-05 — Epic 0: Lexicon parity spec, parity matrix, roadmap, GPL-3 relicense
Kicked off the Lexicon parity initiative. Wrote `docs/lexicon/` — twelve domain specs (overview, interop, library, smartlists, analysis, cues/player, files, health, streaming, history/backup, recipes, extensibility) derived from the official 95-section Lexicon manual supplied by the project owner, plus `PARITY.md` (a 102-row feature matrix: 4 done, 31 partial, 51 missing, 16 deferred) and `GAPS.md`. Added `docs/ROADMAP.md` with eight epics, one branch and draft PR each, Rekordbox-first scope. Appended ADR-0011 (relicense MIT -> GPL-3.0-or-later so libKeyFinder/aubio/audiowaveform become usable), ADR-0012 (analysis stack: beat_this_cpp MIT, libKeyFinder GPL-3, libebur128 MIT, Chromaprint non-FFTW; excludes madmom and Essentia TF models as non-free CC-BY-NC weights, and Spotify audio-features as deprecated), and ADR-0013 (two-level smartlist rule model, not a general boolean tree). Added a root `CLAUDE.md`, which the repo previously lacked. Relicensed: `LICENSE` is now GPL-3 with the reklawdbox MIT grant preserved in `NOTICE`; `Cargo.toml` and `package.json` declare `GPL-3.0-or-later`; `crates/stratum-dsp` keeps its own MIT OR Apache-2.0 and is noted in `NOTICE`. The Lexicon manual itself is gitignored at `docs/lexicon/source/` — it is copyrighted third-party documentation and this repo is public.

Three findings worth flagging from the manual that were not previously known:
1. **Lexicon deliberately does not ship Camelot notation** ("due to licensing restrictions") and uses Open Key instead. `decks` uses Camelot throughout, including a palette explicitly labelled Mixed In Key's. Decision needed before the repo goes public — logged in `docs/lexicon/GAPS.md`.
2. **Recipes is a second, much larger bulk-edit system** (~44 parameterized operations across 8 categories) distinct from the ten fixed Smart Fixes. `decks` has parity on Smart Fixes and none of Recipes.
3. **`crates/scoring::score_transition` and the `suggest_next_tracks` Tauri command have no frontend caller** — Mixable Tracks is half-built and stranded. Epic 6 picks it up.

Verification: `cargo test --workspace --exclude decks-desktop` all green (0 failures). The Tauri shell cannot build in the Linux dev container (`gdk-3.0` missing; project targets macOS/Windows), so `decks-desktop`, `pnpm test/typecheck/lint` and `pnpm e2e` were not run — this epic changes no code, only docs and licence metadata.

## 2026-05-25 — Sub-Plan 8c: Library-wide duplicate detection + DuplicatesView
Extended `rekordbox_db::DuplicateGroup` with `kind: DuplicateKind` (ExactTitleArtist | FuzzyTitle | AudioFingerprint) and `confidence: f32`; both `#[serde(default …)]` so legacy responses still deserialize. Rewrote `audio_fingerprint_duplicates` to bucket by the first 4 chromagram bytes then pairwise within buckets (turns 50k-track O(n²) into a sum of O(k²) per bucket), with `FINGERPRINT_HAMMING_MAX_BITS = 10` constant and per-bit-count Hamming (`hamming_bits`) so the threshold is on bits, not bytes. Added `library_duplicate_groups` (queries + `RekordboxDb` method) that runs all three strategies in one call, and a new `library_duplicate_groups` Tauri command in `apps/desktop/src-tauri/src/lib.rs` that pulls fingerprints from `cache.sqlite3.audio_fingerprints` via `CacheDb::get_all_fingerprints` (degrades to empty map on cache miss/error). Frontend: new `DuplicatesView.tsx` with per-group radio "keep one" picker → `archive_tracks`, "Open in inspector" per row, and grouped header chips by kind; wired into `SidebarNav` (`duplicates` entry between Smart Fixes and Track Matcher) and `App.tsx` (`onOpenInspector` swaps inspector to `details`). New IPC: `listLibraryDuplicateGroups`. Tests: 4 new Rust unit tests in `queries::health` (`groups_exact_title_artist`, `groups_fuzzy_title_above_threshold`, `groups_fingerprints_within_hamming_threshold` covering both pair-match and >10-bit reject, `library_groups_combines_all_three_strategies`) + 4 new vitest cases in `DuplicatesView.test.tsx` (renders three kinds, keep-one archives the others with correct IDs, empty state, open-inspector callback). Verification: `cargo test --workspace` ok, `cargo clippy --workspace --all-targets -- -D warnings` clean, `pnpm test` 208/208, `pnpm typecheck` + `pnpm lint` clean, `pnpm e2e` 8/8.

## 2026-05-24 — Sub-Plan 8b: Waveform peaks persisted across restarts
Added migration v6 (`crates/cache/src/migrations.rs`) creating `waveform_peaks(track_uri PK, peaks BLOB, sample_count, generated_at)` plus `CacheDb::set_waveform_peaks` / `get_waveform_peaks` accessors storing little-endian `f32` blobs. The `get_audio_waveform` Tauri command now checks the cache first (honoring the requested `bars` count via `sample_count` match), only decoding via symphonia on miss, and persisting non-empty results. 3 new Rust tests (`waveform_peaks_round_trips`, `_returns_none_for_unknown`, `_overwrite_replaces_data`).

## 2026-05-24 — Sub-Plan 8a: Filter persistence per library
`loadPersistedFilters` / `persistFilters` now key localStorage by `libraryPath` (`decks.filters.v1::<path>`, with the un-keyed `decks.filters.v1` retained as a legacy fallback for `null`). `App.tsx` re-loads filters when the active library changes and writes persist scoped to the current library. Added 6 vitest cases in `apps/desktop/src/lib/filters.test.ts` covering round-trip, library scoping, `query`/`missingFiles` reset on reload, legacy null-key behaviour, quota-exceeded silent-fail, and malformed-JSON recovery.

## 2026-05-24 — Sub-Plan 6: Sync options wired end-to-end
`SyncPanel`'s previously-stubbed option controls (`cue_destination`, `keep_grids`, `convert_keys`) are now live. Added `changes::applier::SyncOptions` + `CueDestination` + `KeyFormat` (snake_case serde), `apply_with_options(tx, &[changes], &opts)`, new `crates/changes/src/key_format.rs` with `to_camelot` / `to_open_key` (24-key table + enharmonics, parse-failure-returns-original semantics), and writer-side honoring in `applier/tracks.rs` (Key field conversion) and `applier/cues.rs` (`Kind` slot selection for hot/memory/both). The `SyncOptions` struct in `apps/desktop/src-tauri/src/lib.rs` now forwards into `apply_with_options`; `ipc.ts` `SyncOptions` gained `cue_destination` / `keep_grids` / `convert_keys` / `change_to_nearest_color` / `all_smartlists_to_playlists`. UI controls are no longer disabled. Per ADR-0010, the "never mutate master.db" invariant is formally relaxed for the opt-in Sync feature under `WriteGuard` (lock probe + timestamped backup + transactional writes). New tests: 4 in `key_format`, 4 in `applier::tracks` (`key_conversion_camelot_rewrites_value`, `_open_key_`, `_original_passthrough`, `_invalid_falls_back_to_original`), 3 in `applier::cues` (`add_cue_memory_destination_forces_kind_zero`, `_both_inserts_two_rows`, `_hot_preserves_staged_slot`), 2 in `applier` (`keep_grids_skips_bpm_edit`, `_false_writes_bpm_edit`), plus 1 vitest in `SyncPanel.test.tsx` (`forwards non-default … to syncExecute`). Partial / deferred (documented in `docs/MANUAL_TEST_PLAN.md`): `keep_grids` only covers BPM `TrackMetadataEdit` — beat-grid ANLZ writes are not staged anywhere today, so there is nothing else to skip; `cue_destination` only governs the `Kind` value of newly inserted cue rows, it does not retroactively re-slot existing cues; `change_to_nearest_color` / `all_smartlists_to_playlists` are plumbed through the option struct but not yet honored. Real-library disposable-DB smoke is queued for manual verification — synthetic-fixture tests pass on this branch.

## 2026-05-24 — Sub-Plan 7: enhanced track columns
TrackTable now exposes an **Energy** column (bar visual; hydrated from `cache.audio_features.energy` per `Track.folder_path`), tints **Key** values with the Mixed In Key Camelot palette (new `lib/camelot.ts`), and conditionally renders an **inline Tags** column with up to three chips + overflow when any tag bindings exist for the active library. Rust `Track` gained `energy: Option<f32>`; `list_tracks` / `library_search` / `list_incoming_tracks` / `list_archived_tracks` call a new `hydrate_energy` helper backed by a batched `CacheDb::get_energy_by_uris` lookup (no N+1).

## Current phase
QA-pass remediation (2026-05-17): fixed six functional bugs that evaded the green test suites — missing `stream_claude_code_chat` Tauri command (re-implemented), ANLZ path-join bug in intro-cue staging (two sites, now share `anlz::resolve_anlz_path` helper), wrong Claude model id (`claude-opus-4-5` → settings-driven, defaults to `claude-sonnet-4-6`), global spacebar handler that swallowed button activation (moved to shared `useKeyboardShortcuts` with button/link/role=button exclusions), `is_playing` never clearing at end of track (audio thread now emits `playback-ended` and clears state), and Relocate flow staging `old_value: null` instead of the original path (now stages real old path + invalidates library/missing-files queries). Manual real-library verification still the v0.1.0 blocker.

## Current task
Manual real-library verification — the data-layer half is now automated via `scripts/real-library-smoke.sh` (13/13 against `~/Library/Pioneer/rekordbox/master.db`, sha256 unchanged). What still requires a human at the UI: first-run wizard walkthrough, track-table scroll smoothness, column sort interaction, theme persistence across restarts, spacebar focus rules, Anthropic key keychain prompt, chat panel mount/unmount, and a fresh `pnpm --filter desktop tauri build` artefact. `master.db` writes are still prohibited.

## Verification baseline
- `cargo test --workspace`: passing as of 2026-05-17 (test count up after adding `claude_agent::parse_stream_line` parser tests and `anlz::resolve_anlz_path` regression tests)
- `cargo clippy --workspace --all-targets -- -D warnings`: clean as of 2026-05-17
- `pnpm test`: passing as of 2026-05-17 (139 tests — +5 for new `useKeyboardShortcuts` test, +1 for SettingsPanel model select, -1 stale spacebar test in `useAudioPlayer`)
- `pnpm typecheck`: passing as of 2026-05-17
- `pnpm lint`: passing as of 2026-05-17
- `pnpm build`: passing as of 2026-05-15
- `pnpm e2e`: passing as of 2026-05-15 (4 Playwright tests)
- `pnpm --filter desktop tauri build`: passing as of 2026-05-16 — fresh `target/release/bundle/dmg/decks_0.1.0_aarch64.dmg` (9.1 MB) and `target/release/bundle/macos/decks.app/Contents/MacOS/decks-desktop` (arm64 Mach-O). Info.plist reports CFBundleShortVersionString=0.1.0, CFBundleIdentifier=app.decks.desktop. Launch verification still pending.

## Current true implementation state
- [x] Repo scaffold, Cargo workspace, pnpm workspace, CI workflow.
- [x] `crates/rekordbox-db`: read-only SQLCipher connection, tracks, playlists, playlist entries, cues, ANLZ beat grid parser.
- [x] `crates/rekordbox-xml`: parse and emit Rekordbox XML with round-trip tests.
- [x] `crates/cache`: SQLite WAL cache with schema migrations and audio-feature cache.
- [x] Desktop app: Tauri 2, React, Vite, Tailwind, first-run library selection and validation.
- [x] Library UI: virtualized track table with filter and sort.
- [x] Track detail UI: metadata and cue display, with visible cue-load failures.
- [x] Audio preview: native rodio play/pause for selected track.
- [x] Waveform rendering and scrub controls: high-fidelity native Pioneer color waveform (Phase 17) plus interactive seek/playhead (Phase 21).
- [x] Settings: theme, library path change, Anthropic API key in OS keychain, and Claude Code install/login/subscription detection.
- [x] Agent read-only MVP tools: search, get track, list playlists, get playlist, list cues, orphan scan, duplicate scan, broken metadata scan.
- [x] Playlist support: backend playlist detail tool and basic playlist panel UI.
- [x] Conversation persistence.
- [x] Safe staged changes and diff review.
- [x] Export accepted changes to Rekordbox XML.
- [x] One-click audit workflow entry point in the agent panel.
- [x] Playwright E2E tests.
- [ ] Real Rekordbox library manual verification.
- [x] macOS release build artifacts generated.
- [x] Final UI audit and redesign recommendations documented.
- [x] Implemented phase 11 UI polish (empty states, panel layout, zero values, placeholder waveform).
- [x] Deterministic synthetic fixture generator: `scripts/seed-test-library.sh`.
- [x] Playlist view fills available workspace height instead of a fixed short band.
- [x] Cue query supports additional real-library `djmdCue` column variants.
- [x] Phase 12 — second UI polish pass: denser track table (28px rows, IBM Plex Mono numerics), labeled sidebar (168px, amber active rule), structured filter system with drawer + chips (BPM/year ranges, key/genre multi-select, missing-metadata toggles, has-cues, not-in-any-playlist, comment-contains), playlist duplicate badges + duplicate count, expanded playlist columns (health dot, genre, time, year), inspector empty state, always-visible Details toggle.
- [x] Two new read-only IPC commands: `list_tracks_with_cues`, `list_tracks_in_any_playlist`.
- [x] Confirmed playlist duplicates are real Rekordbox `djmdSongPlaylist` entries — surfaced via DUP badge, not deleted.
- [x] Shared `agent-tools` Rust service for provider-neutral tool execution.
- [x] `decks mcp` local stdio MCP server for Claude Code, Gemini CLI, and other local MCP hosts.
- [x] `decks tools call` diagnostic CLI for direct tool invocation.
- [x] HTTP MCP transport for OpenAI Responses API remote MCP usage (`decks mcp-http --bind <addr>`).
- [x] `crates/stratum-dsp`: vendored DSP engine (BPM detection, key detection, beat-grid HMM) from reklawdbox.
- [x] `crates/audio-tags`: lofty-based tag read/write for MP3/FLAC/M4A/WAV (title, artist, album, genre, BPM, key, comment, year, duration).
- [x] `crates/audio-analysis`: Symphonia decode + stratum-dsp analyze + Camelot key conversion + `audio_features` cache integration.
- [x] Tauri commands: `read_audio_tags`, `analyze_track`, `write_audio_tags`.
- [x] Agent MCP tools: `library.read_file_tags`, `library.analyze_track`, `library.scan_and_propose_missing`.
- [x] Track inspector: "Analyze" button → analysis result section with BPM/key/confidence + "Propose correction" buttons.
- [x] Phase 14 — ElevenLabs UI integration: synthetic StaticWaveform behind cue markers (clearly labeled "preview" — real audio analysis still deferred), Message + Response components for chat bubbles, ShimmeringText for agent thinking state, Conversation + scroll button for the message list. Added `@/*` path alias, shadcn-compatible Tailwind/CSS-var aliases, and shadcn Button/Avatar primitives.
- [x] Phase 15 — Fixed Claude Code `stream-json` output format parsing bug in Rust backend. Sub-process text is now properly emitted and integrated via `useAgent` progressive chunking, making the local subscription-backed Claude Code chat fully operational.
- [x] Phase 16 — UI/UX Layout & Filtering Enhancements (implemented by Gemini agent): Collapsible sidebar nav, resizable inspector panel (Chat/Details), resizable table columns, inline column search filters, non-blocking filter drawer, searchable multi-select Radix UI dropdowns for Key/Genre, Cmd/Shift multi-select track selection with summary action bar, and a toggle to hide the playlist sidebar for more space.
- [x] Phase 17 — Native Pioneer Waveform Rendering (implemented by Gemini agent): Reverse-engineered ANLZ parser in Rust (`PWAV`, `PWV3`, `PWV4`, `PWV5`) and high-fidelity `<ColorWaveform>` Canvas component replacing the synthetic placeholder.
- [x] Phase 18 — Smart Missing File Relocation (implemented by Gemini agent): `relocate` crate with fuzzy filename matching (Levenshtein), exposed via `relocate.scan` agent tool and `<RelocateBanner>` bulk-fix UI.
- [x] Phase 19 — Analytics Dashboard (implemented by Gemini agent): Efficient SQLite backend aggregation and `recharts` frontend UI (`<AnalyticsView>`) for genre, BPM, and key distributions.
- [x] Phase 20 — Audio-Fingerprint Duplicates (implemented by Gemini agent): Chromagram 128-byte hash extraction via `stratum-dsp`, cached persistently, with Hamming-distance grouping for detecting true duplicate audio files.
- [x] Phase 21 — Audio Playback Scrubbing (implemented by Gemini agent): `rodio` seek wiring and active polling for interactive waveform clicking and playhead tracking.
- [x] Phase 22 — The Inbox Workflow & Bulk Cues (implemented by Gemini agent): Dedicated `InboxView` for tracks missing metadata/cues/playlists, and a bulk "Add Intro Cues" tool that parses ANLZ beat grids to calculate mathematically perfect 1.1 downbeats and 4-bar loops.
- [x] Post-Gemini remediation (2026-05-15): finished the missing `library_stage_intro_cues` Tauri command + `Relocate*` enum variants the Gemini sessions left dangling, added unit tests for intro-cue logic and synthetic ANLZ PWAV/PWV3/PWV4/PWV5 parsers, added `PlaylistRemoveTrack` / `PlaylistDelete` export tests, added `audio_fingerprints` migration test, and removed dead `health__audio_fingerprint_scan` UI plumbing that called an unimplemented Tauri command. Removed the unfinished `SetBuilderView.tsx` Phase 3 stub (out-of-scope, didn't typecheck).
- [x] Custom-tags hardening (2026-05-24): `usage_count` badge surfaced from a new `list_tags` projection, `list_track_tags_map` IPC + `FilterContext.tagsByTrack` hydration, `tagIds`/`tagMatchAll` filter dimension, FilterDrawer Tags section + FilterChips chips, global `T` shortcut + right-click "Edit tags…" action wired to `TagPickerModal`, and a "Show N tags in library" jump-to-filter button on `CustomTagsPanel`.

## MVP phase checklist
- [x] Phase 0 — Repo familiarization and status reconciliation.
- [ ] Phase 1 — Stabilize current foundation and tag `v0.1.0`. (Ready for manual verification)
- [x] Phase 2 — Define MVP agent and playlist scope.
- [x] Phase 3 — Implement missing read-only agent tools and playlist view.
- [x] Phase 4 — Persist conversations.
- [x] Phase 5 — Safe staged changes system.
- [x] Phase 6 — Inline diff review UI.
- [x] Phase 7 — XML export.
- [x] Phase 8 — One complete MVP workflow.
- [x] Phase 9 — Playwright E2E.
- [x] Phase 10 — Local macOS release build.
- [x] Phase 11 — Full UI audit and redesign suggestions.

## Blockers
- Real Rekordbox 7 `master.db` manual testing requires access to a local user library. **Data-layer portion is now automated** — see `scripts/real-library-smoke.sh` (13/13 against the real library at `~/Library/Pioneer/rekordbox/master.db` on 2026-05-16, sha256 unchanged; covers `library_search`, `library_get_track`, `library_list_playlists`, `library_get_playlist`, `library_list_cues`, all four health scans, `staging_list_changes`, `library_read_file_tags` and (opt-in) `library_analyze_track`). UI-only items still need a human.
- Packaged app artifacts rebuilt fresh on 2026-05-16 at `target/release/bundle/macos/decks.app` and `target/release/bundle/dmg/decks_0.1.0_aarch64.dmg`; bundle structure verified (Info.plist OK, arm64 Mach-O binary present). Manual launch verification against a real/disposable library is still required.
- Claude Code is detected locally. Subscription-backed Claude use is now supported through Claude Code as the MCP host; the embedded Tauri chat still uses Anthropic API keys.
- Custom Tags drag-to-reorder/move (Sub-Plan 1 Step 9) is deferred. The backend `move_tag` IPC exists, but wiring `@dnd-kit` for chip drag is blocked on a new `reorder_tags` command for within-category ordering; see comment in `apps/desktop/src/components/CustomTagsPanel.tsx`.

## Sub-Plan 2 — Genre/Artist Cleanup test coverage (2026-05-24)
Added `apps/desktop/src/components/CleanupPanel.test.tsx` (7 tests covering list/rename/delete for both `mode="genre"` and `mode="artist"`, shift-click multi-select, and empty/disabled states) plus Playwright `apps/desktop/e2e/cleanup.spec.ts` exercising the full rename → stage → accept → export round-trip. Smoke-script `list_genres` block deferred: no MCP/CLI tool exists for it (Tauri-only command), and adding one would be new feature code beyond this sub-plan's scope.

## Sub-Plan 3 — Smart Fixes E2E + rigor (2026-05-24)
Added `apps/desktop/e2e/smart-fixes.spec.ts` — full preview → deselect-one → stage → "Changes" round-trip against `smart_fix_preview` / `smart_fix_apply` / `list_changes` IPC mocks (fix_casing, 3 proposals, deselect middle, assert staged count = 2 and right field/old/new values surface in DiffReviewPanel). Added two Rust edge-case tests to `crates/smart-fixes/src/fixes/add_mix_parens.rs`: bare-suffix-only titles ("Original Mix", "Remix") yield no proposal, and a title already ending in a parens group is never double-wrapped even when an earlier suffix word matches.

Deferred (documented gaps, not fixed — they are feature work beyond this rigor sub-plan):
- Smoke-script `smart_fix_preview` block: deferred because `smart_fix_preview` / `smart_fix_apply` / `common_text_blocklist_*` are Tauri-only commands and have no `decks tools call` (MCP) exposure (`grep smart_fix crates/agent-tools/src/mcp.rs` → no matches).
- Common-text blocklist UI: only the IPC wrappers exist (`commonTextBlocklistList` / `Add` / `Remove` in `apps/desktop/src/ipc.ts:603-613`). No UI in `SmartFixesPanel.tsx` consumes them, so there is nothing to vitest. Surfacing the blocklist as a settings sub-panel is a follow-up feature.

## Sub-Plan 5 — Track Matcher CSV + round-trip (2026-05-24)
Moved CSV parsing for the Track Matcher off the UI thread and onto the Rust backend. New `track_matcher::csv_input::parse_csv(input, title_col, artist_col)` powers a Tauri command `parse_csv_for_matcher` (IPC: `parseCsvForMatcher`), so the renderer no longer carries a hand-rolled CSV tokeniser. `TrackMatcherView` keeps the existing column-mapping UI (title required, artist optional) but now delegates the parse step, and the headers used by the dropdowns come from the file's first line on upload. Added 6 integration tests in `crates/track-matcher/tests/csv_roundtrip.rs` covering happy-path, extra-column ignore, missing-title error, empty-row skip, artist-omitted, and a full CSV → `match_all` → assert-results pipeline against an in-memory library. Vitest in `TrackMatcherView.test.tsx` now mocks `parseCsvForMatcher` and asserts the column-mapping UI surfaces. External APIs (Spotify, YouTube, etc.) remain explicitly out of scope — paste/.txt/.csv only.

## Sub-Plan 4 — Incoming/Archive verification (2026-05-24)
Confirmed `IncomingView` and `ArchiveView` ship the documented header actions (`Mark all reviewed`, `Archive selected`, `Unarchive`, `Delete from library`) with vitest coverage already in place (`IncomingView.test.tsx` 3 tests, `ArchiveView.test.tsx` 3 tests). Added `apps/desktop/e2e/incoming-archive.spec.ts` (2 tests): inbox "mark all reviewed" round-trip (`1 new track` → confirm dialog → `0 new tracks`) and archive unarchive round-trip (`1 archived track` → row click selects → Unarchive → `0 archived tracks`), both driven via mocked `list_incoming_tracks` / `list_archived_tracks` / `clear_incoming` / `unarchive_tracks` IPC.

Deferred (documented gaps, not fixed — feature work beyond this rigor sub-plan):
- **Track context-menu actions for Incoming/Archive.** Per spec, right-click on a row in Incoming should offer "Mark as reviewed" + "Archive", and Archive should offer "Unarchive" + "Delete from library". `useTrackContextActions` (`apps/desktop/src/hooks/useTrackContextActions.tsx`) does NOT include any of these actions — its menu only exposes Show details / Play / Analyse / Edit tags / Stage intro cue / Reveal / Copy path / Copy ID / Remove from playlist (when playlistId is set). The views still surface the same operations via header buttons (which the new E2E spec covers), so the user-visible workflow is intact, but the right-click parity is missing. Wiring this requires plumbing a per-view "extra actions" param through `App.tsx#handleTrackContextMenu` → `useTrackContextActions` to inject view-specific items keyed on the current view, plus 4 new menu entries; >20 lines + multiple tests, so deferred as feature work.
- **Smoke-script `list_incoming_tracks` / `list_archived_tracks` blocks.** Deferred — `grep -nE 'list_incoming|list_archived|incoming|archived|archive_tracks|unarchive|clear_incoming|stage_track_delete' crates/agent-tools/src/mcp.rs` returns zero matches, so neither command is reachable via `decks tools call`. Adding the MCP exposure is new feature code.
