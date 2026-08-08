# 08 — Streaming, Discovery, Export

Owned by **Epic 7**, deliberately last: this domain carries the most external dependency risk.

---

## Streaming tracks

*What it does* — Streaming tracks live in the library alongside local files. Support is explicitly
**partial** and differs sharply per service:

| Service | Playable in Lexicon? | Cues / beatgrids? | Add by pasting a link |
|---|---|---|---|
| **SoundCloud** | ✅ fully | ✅ yes | ✅ track links |
| **Beatport** | Preview only | ❌ (needs the full track) | ✅ track + release links |
| **Beatsource** | ❌ | ❌ | — |
| **Tidal** | ❌ | ❌ | ✅ track links |

*Conversion* — **converting streaming tracks between DJ apps is fully supported for all sources**,
even those that can't be played. The reference survives even when the audio doesn't.

*Adding by paste* — `Cmd/Ctrl+V` a web link into the track browser.

*Note:* **Spotify is not a streaming source.** It appears only as a matching target (below).

*decks status* — **missing.**

*Epic* — **7**.

---

## Beatport integration

*What it does* — Browse and search the entire Beatport catalog from the sidebar, with filters,
drill-down, artist/label **following**, and a **cart**. Purchase completes on the Beatport website.

*The purchase-replacement trick* — drag a purchased file into Lexicon and it **automatically
replaces the matching Beatport streaming track everywhere**, re-pointing all playlists to the local
file. This is the same machinery as single-track relocate
(see [`07-health.md`](07-health.md#find-lost-tracks--relocate)).

*decks status* — **missing.**

> Access risk: Beatport's v4 API is partner-gated with no self-serve signup.

*Epic* — **7**.

---

## Charts

*What it does* — Multiple chart sources; Beatport charts give a top 100 per genre (Beatport limits
this to tracks added in the last week). An **only new tracks** filter hides what's already in the
library.

*decks status* — **missing.** *Epic* — **7**.

---

## Store Links

*What it does* — Given a chart or a list, automates the tedious per-track search across online
stores to find where each track can be bought, with price comparison.

*decks status* — **partial.** `track_matcher::store_links` generates the correct search URL per
track for Beatport, Bandcamp, Discogs, Spotify, Tidal, SoundCloud and YouTube — the three the
manual names for onward search, plus the stores a DJ actually buys from. That is the tedious part,
and it is honest: a search link claims nothing about whether the track exists or what it costs, and
cannot be wrong in a way that costs the user anything.

**Price comparison is not built**, and neither is anything else needing an authenticated store API.
Doing it properly means a registered application and a per-user token per store — an account action
with terms attached. The UI says plainly that these are search links only, rather than implying a
comparison that is not happening.

*Epic* — **7**.

---

## Track Discovery

*What it does* — Recommends tracks not yet in the library.

*decks status* — **missing.** *Epic* — **7**.

---

## Track Matcher

*What it does* — Paste or upload a tracklist (`.txt` / `.m3u8`, one entry per line, selectable
separator such as ` - `) and fuzzy-match it against the library. Tolerates typos, edits and remix
suffixes. Matches become a new playlist. **Unmatched entries can be sent onward to Spotify, Tidal
or Beatport** to hunt down. Aimed squarely at wedding and event DJs working from request lists.

*decks status* — **partial, and solid.** `crates/track-matcher` does normalisation (strip `feat.`,
strip `(Original Mix)`/`(Extended)`/`(Radio Edit)`), exact match on normalised artist+title, then
token-sort Levenshtein ≥ 85, returning `exact` / `fuzzy` / `unmatched` with a confidence score.
CSV parsing moved to the Rust backend (`parse_csv_for_matcher`) with a column-mapping UI.

**Now done.** `tracklist::parse` reads `.txt` and `.m3u8` with one reader — an `.m3u8` is a text
file whose non-track lines start with `#` — preferring `#EXTINF` titles over the path lines beneath
them, because a path is a location and matching a library by path is what Relocate is for. The
separator is *selectable* as the manual specifies, not guessed: hyphen, en dash, em dash,
`Title by Artist` (the one form where the sides swap), none, or custom. Matches become a playlist
via the existing `create_playlist_from_tracks`.

Two things real request lists forced, neither in the manual:

- **Numbered indices are stripped.** `1. Daft Punk - ...` otherwise makes the artist `1. Daft Punk`,
  which normalisation does not remove and which pushes a genuine match under the fuzzy threshold.
  A digit alone is never an index, though — `99 Problems` and `1979` are titles — so the number
  must be followed by punctuation, or introduced by `#`.
- **`#` is ambiguous** between the two formats: a directive in `.m3u8`, a list index in a
  hand-written setlist. What follows settles it — letters mean directive, digits mean index.

Onward search is built as **generated store search URLs** (see §Store Links). The
push-to-service half is not; it needs a registered application and a per-user token per service.

*Epic* — **7**.

---

## Send To (streaming)

*What it does* — Push a Lexicon playlist out to **Spotify, Tidal or Beatport** as a new playlist.
It **matches rather than uploads**, and produces a report of everything it couldn't find. The
manual points users at Smart Fixes first, since match quality depends on clean titles.

*decks status* — **missing.** *Epic* — **7**.

---

## Transfer Streaming To Local

*What it does* — Converts streaming references into local files once the user owns them, including
a single-track relocate path.

*decks status* — **missing.** *Epic* — **7**.

---

## Share / export

*What it does* — Right-click a playlist → `Share`. Explicitly distinguished from Sync: sharing
produces a file, syncing updates a DJ app.

| Output | Notes |
|---|---|
| Quick Copy | Title + artist to clipboard |
| Quick Copy (With Numbers) | Same, line-numbered |
| CSV | Exactly the columns selected, in the order shown |
| M3U | Paths plus extended artist/title info |
| HTML / PDF | Printer-friendly HTML; PDF via the browser's Save to PDF |

*Column control* — right-click the header to choose exported columns and drag to reorder; sort by
one column, or by several with `Cmd/Ctrl` held (e.g. Key then BPM). The export mirrors the view.

*decks status* — **done.** `crates/share` renders all five outputs; **Playlist Tools → Share**.
The default CSV columns are title / artist / BPM / key / duration — exactly what the user-level
`dj-setlist-builder` skill reads, so an export drops straight into that tooling.

Rendering lives in Rust rather than the renderer so the CLI and MCP server can reach the same
export, and so CSV escaping has one implementation. Notes:

- **CSV formula injection is defused.** A comment field starting `=`, `+`, `-` or `@` is quoted and
  prefixed with `'`. Comments are free text a DJ pasted from somewhere, and Excel treats those as
  executable.
- **M3U reports what it could not carry.** A track with no file path cannot be in a list of paths;
  handing back a quietly short playlist is how a set goes missing on the night.
- **HTML is self-contained** — inline CSS, no external references — so it works off a USB stick.
  PDF is the browser's Save to PDF over it, which is how Lexicon does it too; there is no PDF
  writer here and there should not be one.
- **A playlist name cannot become a path.** `Friday 8/6` exports as `Friday 8-6.csv`.

**Not done:** dragging header columns to reorder them. The picker orders by the order columns were
ticked, which covers the same intent for a list being built rather than rearranged.

*Epic* — **6**.
