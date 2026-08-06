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
  the enrichment providers cannot be written *or* verified here — only on a machine with open
  egress. Response parsers written against invented shapes would be untestable production code.
- **There is no audio to calibrate against.** `fixtures/audio/` contains only `.gitkeep`; real
  fixtures are gitignored by design. Energy, Danceability and beatshift detection all need real
  encoder-padded, genre-varied audio before their numbers mean anything.

## Open questions for the project owner

1. **Camelot vs Open Key.** Lexicon avoids Camelot for licensing reasons and ships Open Key.
   `decks` uses Camelot throughout, including a palette explicitly labelled as Mixed In Key's.
   Before this repo goes public under GPL-3 we should decide whether to follow suit. See
   `04-analysis.md` and `PARITY.md`.
2. **Energy scale.** Lexicon documents an *absolute* energy scale and a second, incompatible one
   from Spotify. Spotify's `audio-features` endpoint is deprecated and unavailable to new
   applications, so we need our own definition and should write it down before implementing.
3. **Undo vs staged changes.** `decks` gates changes before they apply; Lexicon offers 60-minute
   undo after. These are different safety models and we should decide whether we want both.
