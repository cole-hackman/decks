# 06 — Files: Watch Folder, Move/Rename, Tag Writing

Owned by **Epic 4**. This is the domain where Lexicon stops being a database editor and starts
managing the filesystem.

---

## Watch Folder

*What it does* — A folder under continuous observation. Any music file dropped in is imported
automatically. Default location `Music/Lexicon/Watch Folder`.

*decks status* — **done**, with one deliberate substitution. `WatchFolderPanel` (Files view) plus
cache migration v10. Arrivals are found by a **debounced scan** every 15 seconds rather than a
native filesystem watcher: the arrival set is a pure function of (files on disk, library,
dismissed), so it is testable without an event loop, it cannot miss something that happened while
the app was closed, and it needs no platform-specific dependency. A push-based watcher would sit
behind the same function and change nothing the user sees.

Two rules the manual does not state. Files whose modification time is less than **10 seconds** old
are held back and reported separately — a large FLAC copied over a network share exists on disk
long before it is complete, and importing mid-copy reads truncated tags and a wrong duration. And
dismissals are recorded, so a file the user chose not to import is not offered again on every scan;
`Un-ignore everything` clears them.

Importing stages a `TrackCreate` change. It is **export-only**: inserting a row into `djmdContent`
needs columns `decks` does not model and cannot verify against a real schema, and a half-populated
row in a performing library is worse than no row. New tracks reach Rekordbox through its own XML
import, which the export emits them into, and the applier's refusal message says exactly that
rather than failing generically.

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

*decks status* — **partial.** `IncomingView` has `Selected done`, `Archive selected` and
`Mark all reviewed`, and the Files view has a watch-folder queue with per-file and bulk
import/ignore.

**`Selected done` auto-advances**, which the manual is right to call out as the detail that makes
triage fast: it marks the selection reviewed and immediately selects the next track, so an inbox
clears with one repeated keystroke (`D`) instead of a click-and-reach cycle per track. The next
track is chosen from the list as it stood *before* removal, so it is the one that visually followed
what the user was looking at, and advancing is skipped entirely if marking failed — advancing past a
track still in the inbox would lose it.

This needed cache migration **v12**: the existing watermark answers "what arrived since I last
cleared" and cannot express "I have dealt with these three", so per-track review state is recorded
separately and filtered out alongside archived tracks.

Missing: delete-from-disk.

*Epic* — **4**.

---

## Auto Move & Rename

*What it does* — When an incoming track is marked done, it is moved to a target folder. If no
target folder is configured, nothing moves — but renaming still happens.

*Subfolder patterns* — up to **three** nested levels, each independently optional, each driven by a
field. `Genre` then `BPM` yields `…/Music/House/128/track.mp3`. **If a field is empty the track
still moves to the target folder, just without that subfolder level** — no orphaning.

*decks status* — **partial.** `crates/file-organizer::subfolder` implements the three levels, the
empty-level rule and all five special patterns; `OrganizeFilesView` (sidebar → Files) runs it over
the selection with a full preview, and each move stages a `TrackRelocate` change. What is missing is
the *auto* half — the watch folder now exists, but marking an arrival done does not yet trigger a
move; that needs the Automatic Actions settings group.

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

*decks status* — **done.** `QuickMovePanel` (Files view) with cache migration v9 behind it.
Destinations are remembered on use (upsert, so the same folder moves up the list rather than
duplicating), favourites sort first and get hotkeys 1–9, and the hotkeys are ignored while a text
field has focus so typing a path does not fire a move on every digit. Moving reuses the Move &
Rename planner, so collisions and `TrackRelocate` staging behave identically, and the success
message repeats the full-sync warning.

The right-click → **Send to → Move files…** entry exists: it opens the Files view scoped to the
right-clicked track, or to the current multi-selection when the right-clicked track is part of it,
so "send these twelve" works as well as "send this one".

Not done: opening a dedicated picker popup from a hotkey — the entry navigates to the Files view
rather than showing a modal.

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

## Delete from disk

*What it does* — Lexicon offers removing a track's audio file, not just its library row, from
Incoming triage, from the broken-track report and from duplicate resolution. In Lexicon this is a
confirm-then-`unlink`.

*decks status* — **done, deliberately differently.** This was declined three times during Epic 5 —
in Find Broken Tracks, in Archive cleanup and in duplicate resolution — on the grounds that it is
the only operation in the program with no undo, and that a program whose first rule is "the library
is read-only" should not be the thing that destroys a DJ's files. It ships now because the user
asked for it explicitly, **with guard rails**, which is what the divergence below buys.

**It is a quarantine, not an `unlink`.** `crates/file-organizer::trash` moves files into a
timestamped batch folder under the app data directory and writes a `manifest.json` beside them
recording each file's original absolute path. `restore` puts a batch back; `purge` is a separate
call, invoked per batch by name, and is the only step that removes anything. There is no
"skip the trash" option, and no "empty all" — both are the shape of a mistake.

**The guards are refusals, not warnings.** `plan` runs before anything moves and drops a candidate
outright when it is:

| Refusal | Why |
|---|---|
| outside every configured music folder | a bad path mapping must not walk a bulk delete out of the collection |
| a symlink | we would either delete the link or destroy a file at a path the user never saw |
| not an ordinary file | directories and devices are not audio |
| missing | there is nothing to delete; the library row is `stage_track_delete`'s problem |
| pointed at by another track | deleting it would break a track the user did not select |
| already in the quarantine | that is `purge`'s job, and has its own confirmation |
| still in a playlist | overridable, once, in the open — see below |

Only the last is overridable, by a single checkbox on the dialog that **re-plans** rather than
waving the rule through per file. "Shared with another track" is not overridable at all.

**The feature is off until the user says where their music is.** With no music folders configured,
`plan` refuses every candidate — a fail-closed default, not a bug. Settings → Deleted audio is
where the list gets filled, and `suggest_music_roots` reads the directories the library already
draws from so filling it is one click. Nothing is stored until the user confirms a suggestion.

**Preview-first, like everything else.** `plan_delete_from_disk` is a separate command from
`delete_from_disk`; the dialog names every file it will move *and* every file it will refuse, with
the reason, before the first click. The first click only arms the confirmation; the second does the
move. `delete_from_disk` re-plans from scratch rather than trusting the plan the renderer sent
back, because between preview and confirmation the disk can change and a guard that ran against
stale state is not a guard.

**Copy verification.** A move across filesystems falls back to copy-then-remove, and the copy is
verified by length before the source is removed. A failed verification leaves *both* copies: a
duplicate is something the user can clean up, a lost file is not.

Reachable from Archive (`Delete from disk`), Find Broken Tracks (`Delete N unplayable file(s) from
disk`, offered only for files that are present but do not decode) and duplicate resolution
(`Delete rest from disk`, alongside — never instead of — the reversible `Keep one, archive rest`).

*Epic* — **6**.

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

*decks status* — **done.** Playlist Tools → Occurrence, for any N. Counted with
`COUNT(DISTINCT PlaylistID)`, because Rekordbox allows the same track twice in one playlist and
"appears in two playlists" must not be satisfied by appearing twice in one.

**Addition:** the report ships the whole distribution — how many tracks sit in 0, 1, 2 … playlists
— and each row is clickable. A bare "how many playlists?" box asks the user to guess a number; the
distribution is what makes the guess unnecessary.

*Epic* — **6**.
