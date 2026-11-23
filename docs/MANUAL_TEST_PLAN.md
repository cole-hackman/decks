# Manual Test Plan

> Run this checklist before release tags and after major workflow changes.

## Test Data

- Real Rekordbox 7 `master.db` for local validation.
- Disposable Rekordbox library for XML import validation.
- Synthetic fixture library generated with `./scripts/seed-test-library.sh`. It writes only
  inside `fixtures/tiny-library` or a temporary directory.

## Automated Read-Only Smoke (run this FIRST)

`./scripts/real-library-smoke.sh` runs the full read-only MCP surface against a
real Rekordbox 7 library and verifies `master.db` is byte-identical (sha256)
before and after. Run this before touching the UI — if the smoke fails, the UI
won't behave either.

It exercises: `library_search`, `library_get_track`, `library_list_playlists`,
`library_get_playlist` (asserts non-empty for a non-smart playlist),
`library_list_cues` (probes multiple tracks until it finds one with cues),
`health_orphan_scan`, `health_duplicate_scan`, `health_fuzzy_duplicate_scan`,
`health_broken_link_scan`, `staging_list_changes`, and `library_read_file_tags`
on a track with a resolvable `folder_path`. Set `RUN_ANALYZE=1` to additionally
exercise `library_analyze_track` against real audio (slow on debug builds — use
a release build with `cargo build -p decks-cli --release`, then re-point the
script at `target/release/decks` by setting `BIN=$PWD/target/release/decks`).

Evidence (JSON responses) lands in `target/smoke/` for diffing across runs.

Last successful run against `~/Library/Pioneer/rekordbox/master.db` (2026-05-16,
99 MB library, 99 playlists, 16 folders): 13/13 with `RUN_ANALYZE=1` (12/12
without), sha256 unchanged. `library_analyze_track` on real audio took ~16 s
on a release build.

## v0.1.0 Foundation Checklist

Items marked `[auto]` are exercised by `scripts/real-library-smoke.sh` against a
real `master.db`. Run the smoke script first and only walk the remaining
human-required items.

- [ ] Launch app in dev mode with `./scripts/dev.sh`.
- [ ] If no library is configured, first-run wizard appears.
- [ ] Select a copy of a Rekordbox 7 `master.db`.
- [auto] App validates DB and shows a nonzero track count. *(smoke: `library_search`)*
- [ ] Track table loads and scrolls smoothly.
- [auto] Filter input matches title, artist, album, and genre. *(smoke: `library_search` exercises the same query path)*
- [ ] Column sorting works.
- [ ] Selecting a track opens the detail panel.
- [auto] Detail panel shows metadata and cues when present. *(smoke: `library_get_track` + `library_list_cues` against a real cue row)*
- [ ] Play button starts audio for tracks with a valid `folder_path`.
- [ ] Spacebar toggles play/pause when focus is not in an input.
- [ ] Theme changes persist after restart.
- [ ] Library path change validates and persists.
- [ ] Anthropic key save/remove works through OS keychain.
- [ ] Chat panel opens and closes.
- [auto] Current chat tools work: search, list playlists, orphan scan. *(smoke: `library_search`, `library_list_playlists`, `health_orphan_scan`)*
- [auto] No code path writes directly to `master.db`. *(smoke: sha256 verified byte-identical pre/post)*

Known `v0.1.0` limitations:

- Waveform rendering is a placeholder.
- Scrub controls are deferred.
- Playlist view is not yet dedicated UI.
- Chat history is not persisted.
- Agent cannot stage/export changes yet.

## MVP / v0.2.0 Checklist

- [ ] App opens packaged macOS build, not only dev mode.
- [ ] Real library opens and displays tracks/playlists.
- [ ] Playlist list and selected playlist track view work.
- [ ] Agent answers read-only track and playlist questions.
- [ ] Conversation history survives restart.
- [ ] Audit workflow finds missing files, duplicates, missing/weird metadata, and playlist issues.
- [ ] Agent proposals appear as staged diffs.
- [ ] User can accept/reject individual changes.
- [ ] User can batch accept/reject safe groups.
- [ ] Export writes Rekordbox-importable XML.
- [ ] Generated XML parses with `rekordbox-xml`.
- [ ] Disposable Rekordbox library can import the generated XML.
- [ ] Exported changes are marked exported.
- [ ] Rejected changes are not exported.
- [ ] `master.db` remains unmodified.

## MCP Host Checklist

- [ ] Build CLI with `cargo build -p decks-cli`.
- [ ] `cargo run -p decks-cli -- tools call library_search --library fixtures/tiny-library/master.db --json '{"query":"Dark","limit":5}'` returns JSON.
- [ ] Raw `tools/list` smoke test returns `library_search`.
- [ ] Claude Code registers the server with `claude mcp add -s user rekordagent -- /Users/coleh/rekordagent/target/debug/decks mcp`.
- [ ] `claude mcp list` shows `rekordagent`.
- [ ] Claude Code can call `library_search` against a real Rekordbox `master.db`.
- [ ] Gemini CLI registers and lists the same MCP server.
- [ ] MCP tools do not write directly to `master.db`.
- [ ] OpenAI path is treated as pending until HTTP MCP transport exists.

## Sync Sub-Plan 6 Verification (disposable-DB smoke)

Sub-Plan 6 (2026-05-24) wires `cue_destination` / `keep_grids` / `convert_keys` from the SyncPanel through to direct `master.db` writes. Per ADR-0010, this is the first feature that mutates `master.db` directly under `WriteGuard`. The synthetic-fixture Rust + vitest tests already pass on `codex/mvp-implementation`; this section is the **manual** disposable-DB step that must be run on a real Rekordbox 7 library before each release that touches the applier.

**Never run this against `~/Library/Pioneer/rekordbox/master.db`.** Always copy it first.

Setup:

```sh
# 1. Stage a disposable copy.
mkdir -p /tmp/decks-sync-smoke
cp "$HOME/Library/Pioneer/rekordbox/master.db" /tmp/decks-sync-smoke/master.db
# Also copy share/peers if present so the WAL probe sees the real layout.
cp "$HOME/Library/Pioneer/rekordbox/master.db-wal" /tmp/decks-sync-smoke/ 2>/dev/null || true
cp "$HOME/Library/Pioneer/rekordbox/master.db-shm" /tmp/decks-sync-smoke/ 2>/dev/null || true
shasum -a 256 /tmp/decks-sync-smoke/master.db > /tmp/decks-sync-smoke/before.sha256
```

In the app (point library path at `/tmp/decks-sync-smoke/master.db`):

- [ ] **Camelot key conversion.** Stage a `TrackMetadataEdit` on field `Key` with new value `"C minor"`, set `Convert keys` to `Camelot`, click Apply. Confirm `SELECT k.ScaleName FROM djmdContent c JOIN djmdKey k ON c.KeyID=k.ID WHERE c.ID=<track_id>` returns `"5A"`.
- [ ] **Open Key conversion.** Same flow, `Convert keys = Open Key`, expect `"5m"`.
- [ ] **Invalid key falls through.** Stage with new value `"Banana"`, `Convert keys = Camelot`, click Apply. Expect ScaleName = `"Banana"` (the conversion logs a warning but does not fail the sync).
- [ ] **keep_grids skips BPM.** Stage a `TrackMetadataEdit` on field `BPM` (e.g. 120 → 128), tick "Don't touch my grids", click Apply. Expect the change to appear in `res.applied` but the `djmdContent.BPM` value unchanged.
- [ ] **cue_destination=memory.** Stage a `TrackAddCue` (e.g. via the "Add intro cue" agent tool — or hand-stage via DevTools) with `new_value.kind=3`, set `Cue destination = Memory cues`, click Apply. Expect one row in `djmdCue` with `Kind=0` (not 3).
- [ ] **cue_destination=both.** Same staged change, `Cue destination = Both`. Expect two rows: one with `Kind=0` and one with `Kind=<staged hot slot>`.
- [ ] **Backup created.** Verify `ls /tmp/decks-sync-smoke/master.db.*.bak` shows a fresh timestamped backup from the first Apply of the session.
- [ ] **Lock probe.** Open Rekordbox against a different library so it holds a lock on its own DB, then try Apply against `/tmp/decks-sync-smoke/master.db`. SyncPanel should *not* show the locked banner (different file), and the write should succeed. To actually exercise the lock branch, run `sqlite3 /tmp/decks-sync-smoke/master.db "BEGIN IMMEDIATE;"` in a held shell and re-Apply — the lock banner should appear and the Apply button should be disabled.
- [ ] **Round-trip.** After all the above, run `shasum -a 256 /tmp/decks-sync-smoke/master.db` and confirm the DB has been mutated (sha differs from `before.sha256`). Then copy the `.bak` back over and confirm sha returns to baseline.

If any of these fail, revert the disposable DB from the `.bak` and file an issue with the exact step + the SyncPanel option payload before debugging.

## macOS Build Artifacts

Latest local build completed on 2026-05-16:

- App bundle: `target/release/bundle/macos/decks.app` (arm64 Mach-O)
- DMG: `target/release/bundle/dmg/decks_0.1.0_aarch64.dmg` (9.1 MB)

Bundle metadata: `CFBundleShortVersionString=0.1.0`, `CFBundleIdentifier=app.decks.desktop`.
Signing/notarization has not been configured. Treat the DMG as a local unsigned test artifact until release signing is added.

## Verification Commands

```sh
cargo test --workspace
pnpm test
pnpm typecheck
pnpm lint
pnpm build
pnpm e2e
pnpm --filter desktop tauri build
```
