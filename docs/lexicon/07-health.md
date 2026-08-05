# 07 — Library Health

Duplicates, missing files, broken files, and metadata backfill. This is where `decks` is closest to
parity already.

---

## Find Duplicates

*What it does* — Two independent detection strategies in one scan:

1. **Audio signature** — a fingerprint of the decoded audio, so an MP3 and a WAV of the same
   recording match despite different files, tags and formats. This is the headline capability.
2. **Artist/Title** — a strict string comparison. No fingerprint match, but strict enough to be
   near-certain in practice. Explicitly depends on clean tags, and the manual points users at Smart
   Fixes first.

*Scan boundaries, stated precisely* — only tracks **between 15 seconds and 15 minutes** are
fingerprinted. Longer tracks are skipped (DJ sets, mixes), as are very short ones (stabs, samples).

*Caching* — signatures are stored in the library, so the first scan is slow and later scans are
fast.

*Interruptible* — the scan can be stopped at any point and the partial results are usable. For a
large library the manual recommends doing it in passes. Worth copying: a long scan that can't be
paused is a scan users won't run.

*Resolution flow* — results are grouped; Lexicon **preselects a keeper** using bitrate, cue
presence and more. A `Prefer` option applies a preference rule across all groups at once for bulk
resolution. A `Review` step shows exactly what is about to be archived before committing.

*The important guarantee* — losers are **archived, not deleted**, and **playlists are rewritten to
point at the keeper**. Nothing breaks; the discarded copies sit in the Archive until the user
cleans them up.

*Manual merge* — for tracks too dissimilar to be auto-detected: select → right-click → Send to →
Duplicate merge.

*decks status* — **partial, and genuinely close.** `library_duplicate_groups` already runs three
strategies (exact title/artist, fuzzy title, audio fingerprint) with `kind` and `confidence` on
each group, backed by a chromagram hash with bucketed Hamming grouping
(`FINGERPRINT_HAMMING_MAX_BITS = 10`), and `DuplicatesView` offers a per-group keep-one picker that
archives the rest. Missing: duration bounds, interruptible scans, preselection heuristics, a bulk
`Prefer` rule, an explicit review step, **playlist re-pointing to the keeper**, and manual merge.

Playlist re-pointing is the significant gap — archiving a duplicate without rewriting playlist
membership leaves holes.

*Epic* — **5**.

---

## Find Lost Tracks / Relocate

*What it does* — Repairs broken file paths. Two tiers.

**Easy path** — missing tracks carry an orange triangle in the browser; right-click → `Relocate`.

- *One track selected* — pick any replacement file. If that file is **already in the library**,
  Lexicon offers a choose-which-to-keep merge: the other entry is removed and **replaced across
  every playlist**, so nothing breaks. This doubles as the mechanism for turning a streaming track
  into a local file.
- *Multiple tracks selected* — pick a folder; Lexicon filename-matches within it and relocates in
  bulk.

**Advanced path** — the Find Lost Tracks utility does **source-prefix → target-prefix** rewriting,
optionally across *all* tracks rather than only missing ones. Built for known, deterministic
changes like a drive letter change, with nothing automatic happening.

*Constraint* — you may only relocate to a path that is not already in the library.

*Extension change* — the utility can substitute a new file extension, for the WAV→MP3 re-encode
case where the originals are gone.

*Backup* — optional automatic backup before rewriting locations, and the manual recommends always
taking it.

*Freshness* — missing-state is re-checked **at most every 5 minutes**; opening a track's Edit popup
or restarting forces a re-check. Listing all missing files is done with an
`Is file missing` smartlist rule rather than a bespoke view — a nice illustration of the smartlist
engine paying for itself.

*decks status* — **partial.** `crates/relocate` does fuzzy filename + size matching and surfaces
through a `RelocateBanner`, and the Cleanup view exposes the flow. Missing: prefix rewriting,
all-tracks mode, extension substitution, the single-track merge-with-existing case, pre-change
backup, and the re-check cadence.

*Epic* — **5**.

---

## Find Broken Tracks

*What it does* — Scans for files that are missing **or unplayable** — a decode check, not just an
existence check. Optionally removes broken files from the library, from playlists, and from disk.

*Reporting* — writes a text report to `Documents/Lexicon` listing every deleted file **and which
playlist contained it**, specifically so the user can source replacements. Paths can also be
exported without deleting anything.

*Aftermath* — the DJ app still holds links to the deleted files; the manual tells users to let the
app prune them or to wipe and re-import.

*decks status* — **missing.** `decks` has a broken-link scan that checks existence, but nothing
verifies the file actually decodes.

*Epic* — **5**.

---

## Find Tags & Album Art

*What it does* — Metadata backfill. Right-click → Track tools → Find tags & album art. Fills:
Genre, Year, Label, Album, **album art image**, Energy, Danceability, Popularity, Happiness. Album
art is downloaded and embedded into the music file.

*Data sources* — **SonoVault** for most fields, **Spotify** for Energy/Danceability/Popularity/
Happiness. The manual is explicit that Lexicon's own audio Energy analyzer is a *different*
algorithm giving *different* numbers, and leaves the choice to the user.

> ⚠️ Spotify's `audio-features` endpoint was deprecated on 2024-11-27 and returns 403 for
> applications registered after that date. Whatever Lexicon relies on here, **we cannot build the
> same thing** — our Energy/Danceability path has to be our own analysis. See ADR-0012.

*Accuracy* — depends entirely on clean incoming artist/title tags; users are pointed at Smart Fixes
first.

*`Original release` option* — strips remix text from titles and searches for the original version,
so re-releases and remasters resolve to the oldest known release data. The manual calls this best
practice for older tracks.

*Genre handling, and this is a good idea* — the Genre field is only ever filled with the **main**
genre (House, Rock, Pop). **Subgenres are written to Custom Tags** instead, with configurable
pairing. Keeps the single-value genre field clean while retaining detail.

*Album art caveat* — Rekordbox does not support album art on WAV files at all.

*decks status* — **missing.** `crates/enrichment` is a stub (10 LOC placeholder). No album art
anywhere in the product.

*Epic* — **4**.
