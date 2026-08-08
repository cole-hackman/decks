# 04 — Analysis

Audio analysis, the Lexicon-only analysis fields, harmonic matching, and the beatshift problem.

---

## BPM & beatgrid analyzer

*What it does* — Detects tempo and lays a beatgrid. Lexicon publishes a first-beat accuracy
breakdown against their test corpus with four result classes worth stealing as a vocabulary:

| Class | Meaning |
|---|---|
| Within 3ms | Precisely correct first beat |
| On-Grid | Grid is right but the downbeat may be offset by 4 or 16 beats — still DJ-usable |
| Range-Error | Correct analysis, but the user must set an explicit BPM range (very low/high tempo tracks) |
| Fail | — |

*Dynamic tempo* — **not implemented in Lexicon either.** Variable-BPM analysis does not exist;
instead the user may add unlimited **BPM changepoints** manually, and changepoints import from any
DJ app. This is a useful scope signal: we do not need variable-tempo analysis for parity, we need
changepoint storage and editing.

*decks status* — **partial.** `crates/stratum-dsp` has tempogram-based period detection and an HMM
beat tracker; `crates/audio-analysis` wires it to Symphonia decode and the feature cache. No BPM
range control, no changepoint model, no accuracy-class reporting.

*Epic* — **2** (changepoints), **3** (accuracy classes surfaced in the generator).

---

## Waveform analysis

*What it does* — Pre-generates waveform data in bulk rather than on demand. Colour presets: three
standard options plus two three-band variants, plus fully custom colours. Because colour is baked
into the generated waveform, **changing colours requires re-analysis** — and the analyzer is smart
enough to skip tracks whose colours have not changed, so leaving "Waveform" checked on a
whole-library run is safe and cheap.

*decks status* — **partial.** `decks` renders the *native Pioneer ANLZ* colour waveform
(PWAV/PWV3/PWV4/PWV5) rather than generating its own, and caches decoded peaks in
`cache.waveform_peaks` (migration v6). Reading Pioneer's own waveform is arguably better than
Lexicon's approach for a Rekordbox-first tool. Missing: user-configurable waveform colours, bulk
pre-generation as an explicit operation, three-band rendering.

*Epic* — **2**.

---

## Key detection

*What it does* — Two algorithms: a built-in analyzer described as "adequate, fast, no setup", and
a free **OpenKeyScan** integration (same developer, standalone app, `MusicalKeyCNN` model) that
Lexicon positions as matching industry-leading paid software. OpenKeyScan runs as a background
process and Lexicon delegates to it when present.

*The Camelot licensing constraint* — **Lexicon does not offer Camelot notation at all.** The manual
states plainly that converting to Camelot is unavailable due to licensing restrictions, and steers
users to Open Key, which is structurally identical (same wheel, same adjacency) with different
letters and no licensing encumbrance.

> ⚠️ **Action item for `decks`.** We currently use Camelot everywhere: `key_format::to_camelot`,
> `apps/desktop/src/lib/camelot.ts` explicitly labelled "the Mixed In Key Camelot palette", and
> Camelot tinting in the track table. Camelot is Mixed In Key's mark. Before this repo goes public
> under GPL-3 we should follow Lexicon's lead — keep the wheel, make **Open Key** the default
> presentation, and treat Camelot as an optional output format rather than the house notation.
> Tracked in `PARITY.md` and worth its own ADR.

*decks status* — **partial.** `stratum-dsp` chroma-based key detection exists and
`key_format.rs` already converts to both Camelot and Open Key (with the 24-key table and
enharmonics). Missing: a second/better algorithm, and the notation posture above.

*Epic* — **6** (notation posture is cheap and can land earlier).

---

## Energy

*What it does* — Fills an `Energy` field from the audio itself, so it works for bootlegs and
unknown remixes. Explicitly an **absolute** scale, not a per-library relative one: chill tracks
should land low and powerful/fast tracks high, on a fixed scale. Lexicon is candid that this
doesn't always hold given how music is structured.

*Second source* — the Find Tags utility can also populate Energy, but that pulls from Spotify, uses
a different algorithm, produces different numbers, and fails for anything not in Spotify's catalog.
Two sources, two scales, documented as such.

*decks status* — **done.** The scale is defined in **ADR-0015**: an absolute 1–10 built from
loudness (0.35), percussive drive (0.25), brightness (0.25) and tempo (0.15), each anchored to a
fixed physical quantity (dBFS, a level-independent ratio, Hz, BPM) so a track's number never moves
because the library moved around it. `crates/audio-analysis/src/energy.rs` implements it and
`analyze_file_cached` fills it, so every existing caller — the context-menu Analyse, watch-folder
arrivals, the agent tools — gains it at once. `ANALYZER_VERSION` is bumped to `stratum-dsp-v2` so
pre-existing cache rows, which have BPM and key but a NULL energy, do not satisfy the lookup
forever.

The loudness term is frame RMS rather than the gated ITU-R BS.1770 measurement ADR-0012 adopted
`libebur128` for; that substitution is contained to one function plus a version bump, and is
recorded in ADR-0015 as a known approximation rather than left implicit.

*Epic* — **4**.

---

## Danceability, Popularity, Happiness

*What it does* — Three further Lexicon-only fields. None exist in any DJ app (see the compatibility
matrix in [`01-interop.md`](01-interop.md)); they reach a DJ app only through Field Mappings. They
feed smartlist rules and Mixable Tracks options.

*decks status* — **missing.**

*Epic* — **4**.

---

## Auto-analysis

*What it does* — On by default. New tracks get missing analysis as they arrive. Crucially scoped:
it applies to tracks **dragged in or picked up by the Watch Folder**, and explicitly **not** to
tracks imported from a DJ app — those are assumed already analysed by that app.

*decks status* — **missing.**

*Epic* — **4**.

---

## Mixable Tracks

*What it does* — Pick a track, get a ranked list of tracks that mix well with it. Successor to
Rekordcloud's "Similar Tracks". Reached via right-click → Track tools → Find mixable tracks.

*Two tiers of options.* Basic mode considers only BPM and key. Advanced mode exposes a full rule
set:

| Option | Behaviour |
|---|---|
| BPM range | Allowed BPM difference |
| Match key | Current or compatible key, following the global Key Mixing Mode |
| Include half/double BPM | Also accept half-time / double-time candidates |
| Match color | Exact track-colour match |
| Recently added | Constrain by date added |
| Must have cue points | Skip un-cued tracks |
| Genre(s) | Restrict to a genre set |
| Year | Match input year or a range |
| Energy / Popularity / Rating / Danceability / Happiness | Match input ±1, or a supplied range |
| Must have tag / Must not have tag | Custom-tag include and exclude lists |

*Key Mixing Mode* — a global setting with two modes, also used by the track browser's
compatible-key indicator:

- **Harmonically Compatible** — traditional harmonic mixing: `10m → 11m`, `10m → 10d`, etc.
- **Fuzzy Key Mixing** — expanded to adjacent Open Key numbers: `10m → 9m/9d/11m/11d`.

*Performance workflow* — a `Use as next track` button re-seeds the list from the track you just
picked, so the tool can be driven live through a set.

*Templates* — option sets are saveable and reusable.

*decks status* — **done.** `crates/scoring::mixable` holds the rule engine; the panel is reached
from the track context menu (`Find mixable tracks`) or the header's `Mixable` toggle, and stays
open as a right-hand inspector so it can be driven through a set. `Use as next track` re-seeds it
from the row just picked. Option sets save as templates (cache migration **v15**). Key Mixing Mode
is a global setting with both modes, and the panel shows the resulting compatible-key set.

Eleven of the thirteen advanced options are implemented: BPM range, Match key, Include half/double
BPM, Must have cue points, Genre(s), Year, Energy, Rating, Must have tag / Must not have tag, and —
since `Track` gained colour and date-added — `Match color` and `Recently added`.

`Match color` is case-insensitive, and a source track with **no** colour admits nothing rather than
everything: the rule means "the same colour as this one", and "the same as nothing" is not a set
worth returning. `Recently added` takes an ISO-8601 cutoff and compares lexicographically, matching
the smartlist date rules; a track with no date is excluded, because we do not know when it arrived
and cannot claim it is new.

**Two remain absent, and not for want of a column.** Popularity / Danceability / Happiness come
from Spotify's `audio-features` endpoint in Lexicon, which was deprecated on 2024-11-27 and returns
403 for applications registered since; Popularity is a catalog metric that cannot be measured
locally at all. See ADR-0012. They are missing rather than present-and-inert, per ADR-0008.

Two things were fixed on the way. `score_transition` carried its own Camelot-only key parser, so
every spelled-out key (`C minor`) scored as "Missing Key Data"; it now routes through
`changes::key_format`, which also learned to read Open Key input (`10m`) since that is what a
library edited in Lexicon stores.

*Epic* — **6**.

---

## Beatshift

*What it does* — Some MP3/MP4/M4A encodings carry leading silence that different DJ apps decode
differently, so cues and beatgrids shift when a library moves between apps. Lexicon:

1. Runs a **beatshift correction scan automatically during every import and sync**.
2. Ships a **Beatshift Fixer** utility for files the scanner misses — it **re-encodes** the file so
   the problem cannot recur. It records which files it has already re-encoded, so re-running is
   safe and idempotent.
3. Offers a global option to auto-re-encode every newly added MP3/MP4/M4A, so files are clean
   before any cues exist.

*A documented workaround worth noting* — re-analyse to get a fresh beatgrid, then run the
**Quantize Cues recipe** to snap existing cues back onto it.

*decks status* — **missing entirely.** This is a real correctness issue for any tool that moves cue
data, and `decks` writes cues today.

*Epic* — **4**.
