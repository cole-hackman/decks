# Status

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
