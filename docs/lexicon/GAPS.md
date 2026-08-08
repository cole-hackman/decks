# Gaps in this specification

What is **not** fully captured, and where to look when it matters.

## Method

Written from the official Lexicon manual (95 top-level sections, ~4,100 lines) supplied by the
project owner, plus the eight UI screenshots in `docs/lexicon_feats/`.

`lexicondj.com` is unreachable from the Claude Code remote environment — the egress proxy returns
`CONNECT tunnel failed, response 403` for that host, and `WebFetch` 403s universally. GitHub and
web search work. So the manual file is the source of record; the live site could not be
cross-checked.

## Known gaps

| # | Gap | Where to look |
|---|---|---|
| 1 | **The `/features` page was never read.** Research indicated it enumerates named features beyond what the manual covers ("…and 20 more features"), so a handful of small features may be unrecorded. | `lexicondj.com/features` |
| 2 | **Per-feature plan gating is partial.** Only a few Free/Essential/Ultimate assignments are recorded. Not load-bearing for us — we have no commercial tiers. | `lexicondj.com/pricing` |
| 3 | **Settings are summarised, not enumerated.** `Music Player`, `File Management`, `Accessibility`, `Advanced` and `Other` groups are named but their individual settings are not transcribed. | manual → Lexicon Settings |
| 4 | **Non-Rekordbox import/sync detail is deliberately thin.** Serato, Traktor, VirtualDJ, Engine DJ, djay Pro and Apple Music each have dedicated manual pages that were not transcribed, since the scope decision is Rekordbox-first. | manual → the `import-*` and `sync-*` sections |
| 5 | **DIRECT2CDJ / Pro DJ Link** is only sketched. `crates/prodjlink` is a stub and this is deferred. | manual → DIRECT2CDJ |
| 6 | **Plugin/Local API surface is undocumented here.** We record that it exists, not its shape. | `lexicondj.com/docs/developers/plugin` |
| 7 | **Mobile app** is out of scope and only noted in passing. | `lexicondj.com/mobile-app` |
| 8 | **Cue Point Generator model internals** are not knowable from documentation. The manual describes inputs, settings and failure modes but not the architecture — expected, and we are building our own approach regardless (see `05-cues-player.md`). | — |

## Duplicated content in the source

The manual file contains the **Smartlists** section twice (once under Playlists, once under
Playlist Tools) with identical text. Not a transcription error on our side.

## Deliberate divergences

| # | Divergence | Why |
|---|---|---|
| A | **`ReleaseDecade` subfolder pattern.** The manual's special-subfolder table lists only date-of-run buckets (current year / month / decade). `decks` also offers the decade of the track's *release* year. | A decade computed from today is the same string for every track in a run; filing a library by release decade is the obviously intended use, and it costs nothing. The manual's `1990 - 1999` example format is preserved. |
| C | **The watch folder scans rather than watching.** Lexicon describes a folder "under continuous observation"; `decks` re-scans every 15 seconds while the Files view is open. | The arrival set becomes a pure function of (files on disk, library, dismissed): testable without an event loop, unable to miss anything that happened while the app was closed, and free of a platform-specific dependency. A push-based watcher would sit behind the same function and change nothing the user sees. |
| D | **New tracks are export-only.** Importing an arrival stages a `TrackCreate` change that sync deliberately refuses. | A `djmdContent` row needs columns `decks` does not model and cannot verify against a real schema. A half-populated row in a performing library is worse than no row, so new tracks go through Rekordbox's own XML import — which the export emits them into, and which the refusal message names. |
| B | **`FileNameL` / `FileNameS` are written only when present.** Rekordbox stores a denormalised filename alongside `FolderPath`; `decks` does not model those columns and has no real fixture to verify them against. | The `TrackRelocate` applier probes `PRAGMA table_info` and writes them if the database has them. Assuming would fail the whole sync on a schema we have not seen; skipping would leave Rekordbox showing a stale filename. |

## Environment blockers

Verified in the Claude Code container on 2026-08-06, and relevant to whoever picks these up next:

- **Metadata APIs are unreachable.** `musicbrainz.org` returns `403` through the agent proxy, so
  provider responses cannot be *verified* here. An earlier revision of this bullet also said they
  could not be **written** here, and that turned out to be wrong: making the transport a seam
  (`enrichment::http::Http`) leaves query construction, parsing, rate limiting and caching fully
  testable without a socket, and they are — 85 tests. What is genuinely unverified is only whether
  the live services return the documented shape. Detail and the unblocking check are below.
- **There is no audio to calibrate against.** `fixtures/audio/` contains only `.gitkeep`; real
  fixtures are gitignored by design. Energy, Danceability and beatshift detection all need real
  encoder-padded, genre-varied audio before their numbers mean anything.
- **ANLZ *writing* cannot be verified here, and four parity rows depend on it.** Investigated
  2026-08-07. `crates/rekordbox-db/src/anlz.rs` parses; nothing writes.

  Producing the bytes is the easy half and is not the blocker. The format is self-describing —
  `PMAI` magic, a big-endian header length, then a chain of sections each carrying its own tag,
  header length and total length — so rewriting `PQTZ` in place is mechanical, and `for_each_section`
  already proves we can walk it correctly.

  What cannot be answered without a real Rekordbox install is whether Rekordbox **accepts** a file
  we wrote:

  1. Whether anything beyond the length fields is validated.
  2. Whether the `.DAT` and its `.EXT` companion must stay mutually consistent — we only read one.
  3. Whether `master.db` carries state that must change alongside it (`AnalysisUpdated`,
     `AnalysisDataPath`), and what Rekordbox does on next launch if it does not.

  The failure mode is not data loss but it is not nothing: a rejected or misparsed ANLZ leaves a
  track with no waveform and no grid in Rekordbox until it is re-analysed there.

  **So it is deliberately unbuilt rather than half-built.** A writer we cannot verify would be
  untestable production code by the same argument that keeps the enrichment providers unwritten,
  and shipping it unwired would violate this project's own definition of done ("reachable from the
  UI, never tests-only"). The four rows that depend on it — **Beatgrid editing**, the last two
  **cue-point recipes**, **Don't Touch My Grids**, **Beatshift correction** — stay `partial` with
  this as their shared reason.

  **What would unblock it:** the disposable-DB smoke harness described in ADR-0010's notes
  (`scripts/real-library-smoke.sh`), extended to write one `PQTZ` section into a copy of a real
  ANLZ file and then open that copy in Rekordbox. That is a fifteen-minute check on a machine with
  Rekordbox installed, and it converts all four rows at once.

### The enrichment providers cannot be reached from this container

The agent network policy denies `musicbrainz.org` (`CONNECT` answered 403), and by extension
`coverartarchive.org` and `api.discogs.com` were never exercised either. The MusicBrainz and
Discogs response parsers are therefore written against the providers' **documented** schemas and
have not seen a live response.

This is a smaller risk than it sounds, and deliberately so. Every parse is tolerant by
construction: an absent or renamed field yields `None` for that value rather than an error, so
schema drift costs *proposals*, never a wrong value written into a library. Compare the ANLZ case
above, where the failure mode is a corrupted user file — that asymmetry is the whole reason one
ships unverified and the other does not.

**The unblocking check**, on any machine with ordinary network access:

```sh
curl -sS -A "decks/0.1.0 ( https://github.com/cole-hackman/decks )" \
  "https://musicbrainz.org/ws/2/recording?query=recording:%22Around%20the%20World%22%20AND%20artist:%22Daft%20Punk%22&fmt=json&limit=1"
```

Compare the shape against `musicbrainz::parse_search`'s expectations — `recordings[]`, `score`,
`artist-credit[].name` / `.joinphrase`, `tags[].name` / `.count`, `releases[].date`,
`releases[].label-info[].label.name`. The equivalents for Discogs are in `discogs::parse_search`.

## Open questions for the project owner

1. **Camelot vs Open Key.** Lexicon avoids Camelot for licensing reasons and ships Open Key.
   `decks` uses Camelot throughout, including a palette explicitly labelled as Mixed In Key's.
   Before this repo goes public under GPL-3 we should decide whether to follow suit. See
   `04-analysis.md` and `PARITY.md`.
2. ~~**Energy scale.**~~ **Resolved** — **ADR-0015** defines it: absolute 1–10 from loudness,
   drive, brightness and tempo, anchored to fixed physical quantities. Spotify's endpoint is
   deprecated and closed to new applications, so its scale was never available to adopt. The
   weights are a judgement call tuned against synthesised signals — `fixtures/audio/` holds only a
   `.gitkeep`, so there is nothing in the repository to validate against — and are stated in the
   ADR so they can be argued with in one place.
3. **Undo vs staged changes.** `decks` gates changes before they apply; Lexicon offers 60-minute
   undo after. These are different safety models and we should decide whether we want both.
