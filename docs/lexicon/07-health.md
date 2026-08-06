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

*decks status* — **done, except interruptible scans and manual merge.** Three strategies (exact
title/artist, fuzzy title, audio fingerprint) with `kind` and `confidence` per group, backed by a
chromagram hash with bucketed Hamming grouping.

**Playlist re-pointing** was the significant gap and is closed. Resolving a group stages a
`PlaylistAddTrack` for the keeper and a `PlaylistRemoveTrack` for each loser, in every playlist that
held one. Archiving a losing copy without that leaves a hole in every set it was in, and the user
finds out on stage. The keeper is added **before** the loser is removed, so applying the batch never
leaves the set briefly short.

A playlist that already holds the keeper gets the loser removed rather than the keeper added twice.

**Duration bounds** are the spec's: only tracks between 15 seconds and 15 minutes are
fingerprint-matched. A track with **no recorded duration is included** — an unknown length is not
evidence of a long one, and excluding it would silently drop everything unanalysed.

**Preselection and `Prefer`** are pure functions in `duplicates::preselect`, so the heuristic is
inspectable and testable rather than buried in a click handler. The default rule puts **cue presence
above bitrate**: losing someone's cue work is the expensive mistake, losing 64kbps is not. Ties fall
through bitrate → playlist membership → play count, and a genuine tie resolves the same way every
run, or a bulk `Prefer` over 200 groups would give a different answer each time it was previewed.

**The review step** names the playlists that will be re-pointed before anything happens, and says
so plainly when none will be. **Losers are archived, never deleted** — the spec's guarantee, and
the confirmation says it.

Still missing: **interruptible scans** (the scan runs to completion; the manual's advice to work in
passes is not yet possible) and **manual merge**.

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

*decks status* — **the advanced path is done; the single-track merge case and the re-check cadence
are not.**

`crates/relocate` already did fuzzy filename + size matching through the `RelocateBanner`.
`relocate::rewrite` adds the deterministic half, at **Files → Rewrite Paths**: a source prefix, a
target prefix, and every matching path is rewritten. **Nothing is inferred** — the spec calls this
the deterministic path, and a tool that guessed the rewrite would eventually guess wrong across an
entire library, so there is no "detect" button.

Decisions, each tested:

- **Separators and case are ignored when matching**, because a user typing `D:\Music` means the
  folder stored as `D:/music/`. But **the remainder keeps its original case** — a rewrite that
  lower-cased the rest of the path would break every file on a case-sensitive filesystem.
- **All-tracks mode is off by default**, and warns when switched on. The common case is repairing
  breakage; sweeping working paths into a rewrite is how a working library stops working.
- **A path already in the library is refused**, per the spec's constraint — and so is a collision
  between two rewrites in the *same plan*, which does not have to pre-exist to be a collision.
- **An empty source prefix rewrites nothing.** It would match every path in the library.
- **Extension substitution** handles the WAV→MP3 case, swapping only the last dot *after* the last
  separator, so `/Music/v1.0/track` gains an extension rather than having `0` replaced.
- The preview reports **"1 of 3 would be rewritten"**, not "1 rewritten" — and lists collisions but
  not the thousands of paths that simply did not match, which would be noise rather than
  information.

Missing-ness is judged **through local path mappings**, so a library opened on a second machine is
not reported as entirely missing before the user has typed anything.

Rewrites stage as `TrackRelocate` and go through review and Sync — whose write guard takes the
"optional automatic backup" the spec recommends, except that here it is not optional.

Still missing: the **single-track merge-with-existing** case (relocating onto a file already in the
library, replacing the other entry across every playlist) and the **5-minute re-check cadence**.

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

*decks status* — **done, with two deliberate divergences.** `audio_analysis::playable` decodes the
file rather than stat-ing it, reachable from **Audit → Find Broken Tracks** and exposed as
`health_playable_scan` through `crates/agent-tools`.

**Two depths, because the honest ones cost different amounts** and the UI names the trade rather
than picking silently:

- **Header** probes the container and builds a decoder. Fast; catches wrong-format files
  (the `.mp3` that is really an HTML error page), unsupported codecs, and empty files. It does
  **not** catch a file that is fine until the last ten seconds.
- **Full** decodes every packet and discards the audio. This catches truncation — the common real
  case — and costs roughly what analysing the track costs.

Outcomes are named rather than boolean: `Missing`, `Unreadable`, `Undecodable`, `Truncated`,
`Damaged { bad_packets }`. Deleting a file that is absent is a different fix from replacing one
that is corrupt, and a track that plays with glitches is a third thing again.

**Truncation needed a second signal.** Raw PCM has no framing to fail on, so a half-downloaded WAV
decodes cleanly and simply ends early. The check therefore compares frames decoded against the
frame count the header declares, with a 1% tolerance for encoder padding — which also makes
truncation detectable in formats where the stream itself would not complain.

Divergences:

- **Nothing is deleted.** The spec optionally removes broken files from the library, from playlists
  and from disk. `decks` reports; removing a track is `stage_track_delete`, reviewed and applied by
  Sync under the write guard. Deleting audio from disk is not offered at all — it is the one
  operation with no undo, and this program's entire posture is that the user reviews first.
- **The report is saved where the user chooses**, not to `Documents/Lexicon`. Writing into a
  directory nobody asked for is the sort of thing that makes a tool feel like it is taking
  liberties. Each entry still names **which playlists held the track**, which is the whole reason
  the report exists.

Paths resolve through local path mappings first, so a library restored on a second machine is not
reported as four thousand missing files.

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
