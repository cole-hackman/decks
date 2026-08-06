# Roadmap — Lexicon Parity Initiative

The epic queue. One epic per branch, one draft PR each, reviewed before the next begins.

Source of truth for *what* each feature means: [`docs/lexicon/`](lexicon/).
Source of truth for *where we stand*: [`docs/lexicon/PARITY.md`](lexicon/PARITY.md).

## Scope

**Rekordbox-deep first.** Reach full Lexicon parity within Rekordbox before adding any second DJ
app. Serato, Traktor, VirtualDJ, Engine DJ, djay Pro, Apple Music, USB export and DIRECT2CDJ are
deferred past this initiative, as are cloud storage, cloud backup, the mobile app, and the plugin
host.

## Definition of done

Per epic, in addition to the standing contract in
[`CLAUDE_CODE_PROMPT.md`](CLAUDE_CODE_PROMPT.md) §0:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm test && pnpm typecheck && pnpm lint
pnpm e2e
```

Plus: the feature is reachable from the UI (never tests-only); `STATUS.md`, `JOURNAL.md` and the
relevant `docs/*.md` update in the same commit; new capability is exposed through
`crates/agent-tools::ToolRequest` where it makes sense, so the chat panel, MCP server and CLI all
gain it at once.

---

## Epic 0 — Spec, parity, roadmap, relicense ✅

Branch `claude/lexicon-spec-docs`.

- [x] `docs/lexicon/` — 12 domain specs written from the official manual
- [x] `docs/lexicon/PARITY.md` — 102-row feature matrix
- [x] `docs/lexicon/GAPS.md` — what's unverified, plus open questions
- [x] `docs/ROADMAP.md` — this file
- [x] ADR-0011 (relicense), ADR-0012 (analysis stack), ADR-0013 (smartlist rule model)
- [x] Root `CLAUDE.md`
- [x] Relicense MIT → GPL-3.0-or-later

---

## Epic 1 — Smartlists engine ✅

Branch `claude/lexicon-smartlists`. Spec: [`03-smartlists.md`](lexicon/03-smartlists.md).

Highest structural leverage: it replaces the ad-hoc filter system with a real query model, and
several later epics consume it (`Is file missing` as a rule rather than a bespoke view; smartlists
materialised on sync; the tag OR/AND selection semantics).

- [x] `crates/smartlists` — rule model per ADR-0013, evaluator, generator
- [x] `crates/cache` migration **v7** — single `smartlists` table with a JSON rule document
      (normalised clause/rule tables were dropped: evaluation is in-memory, so nothing queries
      individual rules)
- [x] Operator vocabulary (`None`, `>`, `<`, `>=`, `<=`, ranges, `!`) — implemented in the rule
      engine; sharing it with the track-browser search box is deferred to Epic 6
- [x] Key-notation-aware equality via `changes::key_format`
- [ ] Tag query language (`~` requires all, `!` negates) in the track-browser search box, and the
      OR-within-category / AND-across-category selection semantics on the Custom Tags page —
      **deferred to Epic 5**. The rule engine already expresses both through
      `has_all` / `has_any` / `has_none`; what remains is the search-box syntax and the Tags page.
- [x] 30-second recompute throttle with a visible loading state
- [x] Archived-tracks-excluded-unless-asked
- [x] Smartlist Generator — by field, by tag category, Decade / BPM range / play count; idempotent
      into a reserved `Lexicon` folder
- [x] Rekordbox compatibility indicator (MyTag: 4 categories, 2 rules)
- [x] Honor `SyncOptions.all_smartlists_to_playlists`
- [x] `Excluded From Sync` name-prefix and tag conventions
- [x] `SmartlistsView.tsx` (editor + generator) wired into `SidebarNav`. Ships as a dedicated
      view rather than inside the playlist tree — Rekordbox playlists come from `master.db` and
      smartlists from the cache DB, so merging the trees is a larger refactor, deferred to Epic 6

**Acceptance:** create a smartlist with a nested OR clause, watch it populate, sync it to Rekordbox
as a materialised playlist, and confirm the tag rules land as MyTag rules where expressible.

---

## Epic 2 — Player, cues, beatgrid, action registry (in progress)

Branch `claude/lexicon-cue-editor`. Spec: [`05-cues-player.md`](lexicon/05-cues-player.md).

Turns `decks` from a viewer into an editor, and is a hard prerequisite for Epic 3.

- [x] **Action registry** — `id`, label, handler, default binding, context predicate. Migrate
      `useKeyboardShortcuts` onto it. Everything below registers into it.
- [x] Cue CRUD via `CueEditor`; `ChangeKind::TrackDeleteCue` added. Placement is on the cue
      list and slot grid — dragging cues on the waveform itself is not done
- [ ] Interaction model: `1`–`8` set/play, `Cmd+1`–`8` delete, double-click seek, `Shift`+click
      move-to-playhead, `Ctrl`+click delete, `Shift`-drag slow scrub
- [x] Loops (length in beats via `OutMsec`)
- [ ] **Active loops** — blocked: needs a `djmdCue` column we do not model
- [x] Quantize, including grid-move-carries-on-grid-cues
- [ ] Cue templates (unlimited, first 8 hotkeyed)
- [ ] Beatgrid *writing* (ANLZ), half/double BPM, BPM changepoints — the grid nudge stages the
      cue moves that follow a grid change, but nothing writes the grid itself yet
- [x] Beat jump along the real ANLZ grid
- [ ] Play queue with autoplay, shuffle, clear
- [ ] Cue Destination round-trip: hidden merged memory cues restored on sync
- [x] Action Center (`Cmd/Ctrl+Space`)
- [ ] Find Popup (`Cmd/Ctrl+F`)
- [ ] Compatible-key indicator in the browser

**Acceptance:** load a track, place and colour cues by keyboard alone, turn one into an active loop,
nudge the beatgrid and watch on-grid cues follow, sync to Rekordbox, verify in Rekordbox.

---

## Epic 3 — Cue Point Generator (in progress)

Branch `claude/lexicon-cue-generator`. Spec:
[`05-cues-player.md`](lexicon/05-cues-player.md#cue-point-generator).

- [x] **Custom cue anchors** — pure name/colour matching, no ML, and it delivers the whole
      template system standalone while giving us ground truth to evaluate detection against
- [x] Cue template model: offsets in beats relative to anchors, name, colour, enabled, order
- [ ] **Structural segmentation** on `stratum-dsp` primitives (beat-synchronous Foote
      self-similarity + novelty peaks; energy contrast separates drop from breakdown) — the
      remaining half of this epic
- [ ] Fade-out detection from low frequencies only
- [x] Start-cue behaviour and keep-cue-position
- [ ] Breakdown min. beats, drop-at-start, auto-generate-on-play — all inputs to detection
- [x] Overflow handling — lowest confidence first, then latest in the track
- [x] Guard: Rekordbox rejects two memory cues at one position
- [x] Honest confidence surfacing per ADR-0008 — `Confidence` rides from anchor to cue to UI

**Acceptance:** generate against a genre-labelled fixture set and report per-anchor accuracy versus
hand-placed cues; never present a low-confidence anchor as certain.

---

## Epic 4 — Files, automation, enrichment

Branch `claude/lexicon-file-organizer`. Specs: [`06-files.md`](lexicon/06-files.md),
[`07-health.md`](lexicon/07-health.md#find-tags--album-art).

- [x] Watch folder → arrivals queue (scan-based), plus `Selected done` auto-advance on the D hotkey
- [x] Rename pattern language: `%field%`, literals, `{}` optional segments
- [x] Up to three nested subfolder patterns, incl. special patterns (bitrate buckets, first tag,
      current year/month/decade)
- [x] Move & Rename over a selection, preview-then-apply, staging `TrackRelocate` per moved file
- [x] Quick move with favourited folders on hotkeys 1–9
- [x] Bulk Write Tags (ID3) with per-field selection
- [x] **Field Mappings** — per-target, overwrite vs append, multi-source combining (ID3 profile; per-DJ-app profiles and sync-time application outstanding)
- [ ] Revive `crates/enrichment`: Find Tags & Album Art; main genre → Genre, subgenres → Custom Tags
- [ ] Album art: fetch, embed, replace, remove, reload
- [ ] Energy / Danceability / Popularity / Happiness from our own analysis (**not** Spotify — see
      ADR-0012)
- [ ] Beatshift detection on import/sync + Beatshift Fixer re-encode with an already-done ledger
- [x] Find Unused Files with include/exclude extensions and DJ-folder skips
- [x] Local Path Mappings
- [x] Automatic Actions settings group — 1 of 5 wired; the rest disabled with their blockers named

**Acceptance:** drop a file in the watch folder, see it analysed, tagged, art-fetched, renamed by
pattern, filed into a genre/BPM tree, and marked done — untouched by hand.

---

## Epic 5 — Recipes, editing, health, backup

Branch `claude/lexicon-library-editing`. Specs: [`10-recipes.md`](lexicon/10-recipes.md),
[`02-library.md`](lexicon/02-library.md), [`07-health.md`](lexicon/07-health.md).

- [x] Recipes engine + the casing / field / text / number operations (18)
- [x] Cue recipes — 9 of 11. `Change Active Loops` needs a `djmdCue` column we do not model;
      `Half/Double BPM` moves grid markers, so it belongs with the beatgrid recipes below
- [ ] Beatgrid recipes (3 ops) — `Delete Beatgrid`, `Round BPM`, `Quantize Beatgrid`. All three
      write a grid, so they need an ANLZ **writer**; `crates/rekordbox-db::anlz` only reads
- [x] `Import Tags from Text` (hashtag → custom tags, idempotent), plus add/remove/replace/clear
- [x] The three "other" recipes: Mark as Incoming, Remove from All Playlists, Import Date from
      Filesystem
- [x] Multi-track manual editor with `<multiple values>` — `E` over the selection; untouched
      fields are never written. Album art and arrow-key track navigation deliberately out
- [x] Import Tags From CSV — match on `Location` or `Artist`+`Title`, write fields, per-row report
- [x] Undo History — Sync runs record inverses; undo re-stages them for review. Kept for 50 runs
      per library rather than 60 minutes, and blocked entries carry a reason (ADR-0008)
- [x] Database backup/restore of the cache DB's derived state — a JSON document rather than a ZIP
      (inspectable, schema-tolerant); analysis caches excluded; nothing auto-deleted
- [x] Duplicates: duration bounds, preselection, bulk Prefer, review step, **playlist re-pointing
      to the keeper**. Interruptible scans and manual merge outstanding
- [x] Relocate: prefix rewriting, all-tracks mode, extension change. Backup rides on `WriteGuard`.
      **Merge-with-existing and the 5-minute re-check cadence outstanding**
- [x] Find Broken Tracks — real decode check at two depths, with a per-playlist report. Deleting
      audio from disk is deliberately not offered; removing a track stays a staged change
- [x] Archive: context-sensitive playlist rule, selection helper, staged cleanup.
      **Delete-from-disk deliberately not offered** — no undo, and the library is read-only first
- [x] Genre/Artist Cleanup: locking, pinned letters, alt-click filter, sort modes. The extra
      artist fields (Remixer/Producer/Composer/Lyricist) need a wider `Track` — same gap as
      label/mix/colour, so they belong with the epic that widens it
- [x] Common-text blocklist settings UI — Settings → Remove Common Text, with the manual's
      `(Original Mix)` and Camelot-key presets offered rather than seeded

---

## Epic 6 — Set preparation (in progress)

Branch `claude/lexicon-set-prep`. Specs: [`02-library.md`](lexicon/02-library.md),
[`04-analysis.md`](lexicon/04-analysis.md#mixable-tracks).

- [x] Surface `scoring::score_transition` — was unreachable from the UI — as **Mixable Tracks**:
      9 of 13 advanced rules, `Use as next track`, saveable templates (cache migration **v15**),
      and a `mixable_tracks` agent tool. The 4 missing rules (colour, date added, Popularity /
      Danceability / Happiness) need fields `Track` does not carry — same gap as label/mix/remixer
- [x] Key Mixing Mode: Harmonically Compatible vs Fuzzy Key Mixing, global, with the
      compatible-key set surfaced in the panel
- [ ] Track Timeline (Key / BPM / Rating / Energy / Danceability / Popularity / Happiness; Key and
      BPM-change bar colouring)
- [x] Playlist tools: Merge, Sort, Cross Reference, Prefix, **Rewrite Order** — all five, in a
      Playlist Tools view. Sort needed a new `ChangeKind::PlaylistReorder` (writes
      `djmdPlaylist.Seq`). Rewrite Order sorts on a field picked in the tool rather than following
      the browser's transient column sort — a documented divergence
- [x] Playlist Occurrence for arbitrary N — in Playlist Tools, counting *distinct* playlists, with
      the full distribution so N does not have to be guessed
- [ ] Favorite Playlists with hotkeys
- [ ] Sidepanel (second track browser)
- [ ] History: snapshot semantics, ratings, locations, deleted-set ledger, save-as-playlist
- [x] Share/export: CSV, M3U, HTML/PDF with column selection — `crates/share`, in Playlist Tools.
      Default CSV columns are exactly what the `dj-setlist-builder` skill reads. Header
      drag-to-reorder not done; the picker orders by tick order
- [x] Key conversion leading-zero option — a Sync setting, applied after conversion and
      independently of it. **Colors → nearest is blocked**: `Track` has no colour field and no
      change kind writes `ColorID`, so there is nothing to map; the flag stays accepted and
      unexposed rather than shipping a switch that does nothing

---

## Epic 7 — Streaming & discovery

Branch `claude/lexicon-streaming`. Spec: [`08-streaming.md`](lexicon/08-streaming.md).

Last, because it carries the most external risk — see ADR-0012 for what is and isn't reachable.

- [ ] Streaming track model (reference, not file) — conversion works even when playback doesn't
- [ ] SoundCloud (playable, cueable), Tidal / Beatport / Beatsource (reference-only)
- [ ] Paste-a-link ingestion
- [ ] Track Matcher: `.m3u8`, configurable separator, create playlist from results
- [ ] Send To Spotify / Tidal / Beatport with a not-found report
- [ ] Transfer Streaming To Local + purchase replacement (re-points every playlist)
- [ ] Charts, Store Links, Track Discovery
- [ ] Beatport catalog browse + cart *(partner-gated API — may prove impossible)*

---

## Sequencing rationale

1 before everything — the rules engine is consumed by 4, 5 and 6.
2 before 3 — the generator needs a cue model to write into.
4 and 5 are independent and could swap.
6 depends on 2 (key mixing) and 1 (rules).
7 last — external APIs may block outright, and nothing else depends on it.

Stopping after any epic leaves a coherent product.
