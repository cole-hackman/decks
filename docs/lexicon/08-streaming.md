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

*decks status* — **missing.** *Epic* — **7**.

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
CSV parsing moved to the Rust backend (`parse_csv_for_matcher`) with a column-mapping UI. Missing:
`.m3u8` input, configurable separator, playlist creation from results, and all onward-search.

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

*decks status* — **missing.** Worth noting this is exactly the export shape the user-level
`dj-setlist-builder` skill expects as input (TSV/CSV with BPM and Key columns), so building CSV
export connects `decks` to that tooling.

*Epic* — **6**.
