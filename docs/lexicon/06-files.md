# 06 — Files: Watch Folder, Move/Rename, Tag Writing

Owned by **Epic 4**. This is the domain where Lexicon stops being a database editor and starts
managing the filesystem.

---

## Watch Folder

*What it does* — A folder under continuous observation. Any music file dropped in is imported
automatically. Default location `Music/Lexicon/Watch Folder`.

*decks status* — **missing.** `decks` has an Incoming view, but ingest is a manual filesystem pick
with fuzzy matching, not a watcher.

*Epic* — **4**.

---

## Incoming Tracks

*What it does* — The staging area for watch-folder arrivals (Sidebar → Tracks → Incoming). A place
to clean tags, add cues, and assign playlists *before* the track joins the library proper.

*Workflow, and the detail that makes it fast* — `Selected done` / `All done` mark tracks processed.
On `Selected done`, Lexicon **immediately loads and selects the next track in the list**, and the
button is bindable to a hotkey. That turns triage into a single repeated keystroke. Copy this.

*Deleting* — `Delete selected` removes tracks from the library **and from disk**. Destructive, and
labelled as such.

*decks status* — **partial.** `IncomingView` exists with `Mark all reviewed` and `Archive selected`
header actions, backed by a `last_incoming_cleared_at` watermark. Missing: auto-advance, hotkey
binding, delete-from-disk, and the whole point of the page — that it is fed by a watcher.

*Epic* — **4**.

---

## Auto Move & Rename

*What it does* — When an incoming track is marked done, it is moved to a target folder. If no
target folder is configured, nothing moves — but renaming still happens.

*Subfolder patterns* — up to **three** nested levels, each independently optional, each driven by a
field. `Genre` then `BPM` yields `…/Music/House/128/track.mp3`. **If a field is empty the track
still moves to the target folder, just without that subfolder level** — no orphaning.

*decks status* — **partial.** `crates/file-organizer::subfolder` implements the three levels, the
empty-level rule and all five special patterns; `OrganizeFilesView` (sidebar → Move & Rename) runs
it over the selection with a full preview, and each move stages a `TrackRelocate` change. What is
missing is the *auto* half — this runs on demand, not when an incoming track is marked done, because
there is no watch folder yet.

*Epic* — **4**.

---

## File rename patterns

*What it does* — A small template language.

- `%field%` interpolates a field.
- Literal text passes through: `(Favorites) %artist% - %title%`.
- `{ }` marks an **optional segment**: everything inside is emitted only if the fields inside it
  have values. This is the whole trick — `%artist% - %title% (%key%)` yields
  `Daft Punk - Get Lucky ()` on a keyless track, while `%artist% - %title% {(%key%)}` yields
  `Daft Punk - Get Lucky`.
- Optional segments compose: `%artist% - %title% {%key%}{|%bpm%}`.
- Optional segments do **not** nest in `decks`; nesting is a parse error rather than a surprise.
- Renders are trimmed, since a dropped segment usually strands its separator.

*Field vocabulary* (verbatim from the manual, and identical to the Lexicon field list):
`artist, title, albumTitle, label, remixer, mix, composer, producer, grouping, lyricist, comment,
key, genre, bpm, rating, color, year, durationSeconds, bitrate, playCount, sizeBytes, sampleRate,
trackNumber, energy, danceability, popularity, extra1, extra2`

*Special subfolder patterns* — computed values rather than raw fields:

| Pattern | Yields |
|---|---|
| Bitrate | `320+` or `320-` — two buckets, not the raw number |
| First tag | The first tag from the **first tag category**, ordered by category order on the Tags page |
| Current year | e.g. `2026` |
| Current month | Zero-padded `01`–`12` |
| Current decade | A range, e.g. `1990 - 1999` |

`decks` adds one more, `Release decade`, for the decade of the track's own release year — see
`GAPS.md` §Deliberate divergences.

*decks status* — **done.** `crates/file-organizer::pattern` implements the language;
`validate_pattern` and `pattern_fields` back the editor, which marks the fields `decks` cannot
supply yet rather than rendering them blank. Illegal filename characters become `-`, and a render
that is nothing but punctuation falls back to the original filename.

*Epic* — **4**.

---

## Quick move

*What it does* — Right-click → Send to → Move files. Pick a folder; Lexicon moves and optionally
renames from tags. Recently used folders are remembered and can be favourited, and **favourited
folders get hotkeys 1–9**. A hotkey opens the popup itself.

*Critical follow-up* — after moving files you must **Full Sync** to the DJ app. A partial sync
leaves the old locations behind; only a full sync clears them.

*decks status* — **missing.**

*Epic* — **4**.

---

## Write Tags (ID3)

*What it does* — Writes the Lexicon database back into the audio files' own tags, so the files look
right in any other program. Right-click → Write tags. **Per-field selection** — write only titles
and leave everything else untouched. Honors field mappings, so Lexicon-only fields can be projected
into real tag fields on the way out. Can be configured to run automatically whenever a change is
detected.

*Why it's separate from sync* — syncing updates the DJ app's database; this updates the files. A
user whose music is also in a plain music player needs both.

*decks status* — **partial.** `crates/audio-tags` (lofty) reads *and writes* title, artist, album,
genre, BPM, key, comment and year for MP3/FLAC/M4A/WAV. `write_tags_bulk` plus `WriteTagsPanel`
(Files view) add the bulk flow with per-field selection over the selection or the whole library.

One rule the manual does not state but the feature needs: **a selected field whose library value is
empty is not written.** Otherwise ticking "Artist" on a library that happens not to know an artist
would blank a perfectly good tag in the file. Those tracks come back as `skipped` and the UI says
how many.

Still missing: field-mapping projection and auto-write-on-change.

*Epic* — **4**.

---

## Find Unused Files

*What it does* — Scans a folder tree and lists every file **not** in the library — the inverse of a
missing-file scan. Aimed at reclaiming disk space.

*Details worth copying*

- Extension filter with `Include` / `Exclude` modes, e.g. include `PNG,JPG,JPEG,BMP` to sweep
  stray images out of a music folder.
- **Known DJ folders are skipped automatically**: `_Serato_`, `Traktor`, `PioneerDJ`, `iTunes`,
  `Engine Library`, and `Lexicon` under Music — plus OS system folders.
- Deletion is **irreversible and says so**; a text report of everything deleted is written to
  `Documents/Lexicon`.
- The scan results can be exported as a plain path list **without deleting**, so users can hand
  them to their own scripts.

*decks status* — **done.** `crates/file-organizer::unused` implements the sweep;
`UnusedFilesPanel` (inside Move & Rename) runs it. Include/exclude extension filtering with an
empty list meaning "no filter" in either mode; the DJ-folder skip list plus OS and VCS
directories, matched case-insensitively; `Copy paths` exports the list without deleting; and a
timestamped record of every deletion is written under the app data folder.

Three guards the manual does not specify but this needs, given the output is a deletion list:
a scan against an **empty library refuses to run** (everything would look unused); library
membership is **re-checked at delete time**, not just at scan time, because the library can gain
a track in between; and path comparison is case- and separator-insensitive, since Rekordbox and
the filesystem do not reliably agree and a case-only mismatch would offer a real track for
deletion. Nothing is pre-selected, and deletion is behind an explicit second click.

*Epic* — **4**.

---

## Local Path Mappings *(Ultimate tier)*

*What it does* — Per-computer mapping from a stored path prefix to a local one, so a database
restored on a second machine finds its music without a bulk relocate. The documented two-computer
workflow is cloud database backup plus path mappings.

*decks status* — **done.** `crates/file-organizer::mappings` plus cache migration v8 and a
`PathMappingsSection` in Settings. Longest matching prefix wins, matching is on whole path
components (so `/Music` cannot swallow `/MusicVideos`), separators are interchangeable because the
databases cross platforms, and matching is case-insensitive while the remainder keeps its original
case — the comparison has to be lenient, the filesystem may not be.

Read-side only: the library keeps saying `D:\Music\…`, which is what lets one database work on two
machines at once. Mappings live in the local cache and are **not** keyed by library path — they
describe where this *computer* keeps its music, and must apply the moment any library is opened.
Never staged, exported or synced.

Applied wherever a stored path is turned into a real one: the missing-file scan (a mapped track is
not missing), Move & Rename's source paths, Write Tags, and the unused-file sweep's known-path set
— that last one matters, since without it every mapped track would look unused and land on the
delete list. `crates/relocate` still solves the adjacent problem of genuinely broken paths.

*Epic* — **4**.

---

## Playlist Occurrence

*What it does* — "Which tracks appear in exactly N playlists?" Setting N to 0 finds orphans; N=2
finds tracks in exactly two. Utility → Other.

*decks status* — **partial.** A `list_tracks_in_any_playlist` IPC command and a
`not-in-any-playlist` filter exist, which covers the N=0 case only.

*Epic* — **6**.
