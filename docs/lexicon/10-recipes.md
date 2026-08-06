# 10 — Recipes, Smart Fixes, CSV Import, Undo

Owned by **Epic 5**.

**The key structural insight:** Lexicon has *two* bulk-editing systems, and `decks` has only
implemented the smaller one.

- **Smart Fixes** — ten fixed, one-click, zero-parameter cleanups. `decks` has these.
- **Recipes** — ~40 *parameterized* operations across eight categories, applied to a selection and
  executed per track. `decks` has none of these.

Recipes are reached via select tracks → right-click → Edit → Recipes.

---

## Smart Fixes (the fixed cleanups)

Ten operations, each with its own manual page: Remove Number Prefix, Fix Casing, Remove URLs, Fix
Encoded Characters, Extract Remixer, Add (Re)mix Parenthesis, Remove Garbage Characters, Replace
Characters With Space, Extract Artist From Title, Remove Common Text.

Two documented Remove Common Text presets worth having as defaults: **remove the Camelot key from
the title**, and **remove `(Original Mix)` from the title**.

*decks status* — **done.** `crates/smart-fixes` implements eleven fixes across thirteen modules
with a preview/apply split (`smart_fix_preview` / `smart_fix_apply`) and a configurable common-text
blocklist. This is genuine parity, and in one respect ahead: `decks` previews every proposal as a
deselectable diff row before applying.

One loose end: the blocklist has IPC wrappers (`commonTextBlocklistList/Add/Remove` in
`apps/desktop/src/ipc.ts`) but **no UI consumes them**.

*Epic* — **5** (surface the blocklist settings panel).

---

## Recipes

*Execution model* — a recipe runs against each selected track individually. Fields referenced are
the standard Lexicon field vocabulary.

### Casing recipes

`To Upper Case` · `To Lower Case` · `To Title Case` — each takes a **Field** and a **Words to
ignore** list. `To Sentence Case` takes a field only.

The words-to-ignore list is the meaningful difference from `decks`'s `fix_casing`, which hardcodes
its article/preposition list.

### Cue point recipes

This category is the most valuable and the furthest from anything `decks` has.

| Recipe | Parameters |
|---|---|
| Delete Cue Points | Mode: cues & loops · first · last · keep first · keep last · loops only · without colour · without text · memory cues |
| Change Cue Colors | Scheme: Basic, Grayscale, Cold, Warm, Random (never repeats a colour), No colors, First cue color (all cues take cue 1's colour) |
| Find & Replace Cues | Match colour + match text → new colour + new text. Empty match text matches *untitled* cues; `*` matches any text; empty match colour matches *uncoloured* cues; empty replacement keeps the existing value |
| Change Active Loops | Promote/demote first/last/all loops or cues to active loops; new loop size in beats (`-1` keeps current) |
| Quantize Cues | Snap to nearest marker at 1 / 2 / 4 (1 bar) / 16 (4 bars) / 64 (16 bars) beats. Requires a beatgrid |
| Half/Double BPM | Adjusts BPM **and** beatgrid markers together |
| Shift Cues/Beatgrid | Millisecond offset, with independent toggles for cues and grid — this is the manual remedy for beatshift |
| Sort Cues | Time ↑/↓, Label A–Z/Z–A, empty labels first/last, cues-before-loops, loops-before-cues |
| Replace Cue Text | Find/replace in cue names, case-insensitive toggle |
| Remove Cue Text | Strip all cue names |
| Remove Cues by Label | Delete cues whose label contains a string (case-insensitive) |

### Beatgrid recipes

`Delete Beatgrid` · `Round BPM` (rounds track BPM and every marker to a whole number — for
electronic music where fractional BPM is an analysis artefact) · `Quantize Beatgrid` (moves the
**first grid marker to the first cue point**; no-op when the track has no cues).

### Field recipes

`Copy Field` (source intact) · `Move Field` (source cleared) · `Merge Fields` (two sources → target
with a separator) · `Prefix Field` · `Suffix Field` · `Swap Fields` · `Split Field`.

`Split Field` is the richest: source field, delimiter, two target fields, plus **preserve split
text** (keeps the delimiter attached to the first part) and **append** (add to the target rather
than overwrite). Splitting `"Get Lucky - Daft Punk"` on `" - "` yields the two halves.

### Text recipes

| Recipe | Notes |
|---|---|
| Remove Text | Field, text, case-insensitive toggle |
| Replace Text | Field, find, replace, case-insensitive toggle |
| Change Extension | Changes the extension `decks` looks for — the WAV→MP3 re-encode case |
| Extract Text | Between a start and end delimiter → target field. Options: include delimiters, delete matched text from source, append vs overwrite |
| Shorten Text | Abbreviate each word to N characters — `"Get Lucky"` at 2 → `"GeLu"` |
| Remove Special Characters | Two modes. **Special characters**: currency (`$→S`, `@→A`, `€/£→E`, `¥→Y`), legal (`®→(R)`, `™→(tm)`, `©→(c)`), normalise mathematical and full-width alphanumerics to ASCII, strip zero-width characters, strip diacritics. **Emojis**: emoji plus modifiers — skin tones, variation selectors, ZWJ sequences |
| Remove Between | Strip text between a delimiter pair *including* the delimiters: `()`, `[]`, `{}`, `<>`, `""`, `''` |

### Number recipes

`Increase/Decrease Number` — field plus signed amount.

### Tag recipes

| Recipe | Notes |
|---|---|
| Import Tags from Text | Converts a hashtag convention (`#Techno #Vocals`) in a text field into real custom tags. Source field defaults to Comment, separator defaults to `#`. **Idempotent** — safe to re-run, existing tags preserved |
| Add Tags / Remove Tags | Bulk apply or strip |
| Replace Tag | Swap one tag for another where present |
| Clear Tags | Strip all custom tags |

`Import Tags from Text` is a strong migration path for users who have been hand-rolling tags in the
comment field for years — worth prioritising within the epic.

### Other recipes

- **Mark as Incoming** — push tracks back onto the Incoming page, using it as a to-do list, or to
  trigger the auto-move machinery. Also available as Send to → Incoming.
- **Remove from All Playlists** — strips a track from every playlist. Explicitly **does not** touch
  smartlists, which are derived.
- **Import Date from Filesystem** — take the file's creation date as the track's date.

*decks status* — **partial.** `crates/recipes` implements the casing, field, text and number
categories — 18 operations — as pure functions of (recipe, fields) → fields. `RecipesPanel`
(sidebar → Recipes) builds a list, previews every proposed change as a deselectable before/after
row, and stages what survives review as `TrackMetadataEdit` changes. Recipes serialise, so one built
today can be saved and replayed on next month's downloads.

Three rules the manual leaves open, decided and tested:

- A recipe whose source field is empty **reports why** rather than doing nothing silently —
  `SourceEmpty`, `NoMatch`, `NotANumber`, `Misconfigured`. "340 of 400 changed" needs an
  explanation attached.
- `Merge Fields` with one half missing yields the other half, not a stray separator.
- `Extract Text` with no match leaves the target **untouched**. Writing an empty string would blank
  a good remixer field, which is worse than not running.

The field vocabulary offered is deliberately the intersection of what `decks` models and what the
applier's allowlist will actually write — offering a field that cannot be persisted would produce a
preview full of changes that silently vanish at sync time.

The **tag recipes** are done too, in `crates/recipes::tags`. They are modelled as a *delta* to the
track's tag set rather than a new value, which is both what the cache's add/remove accessors want
and what lets a preview say "adds 3, removes 1".

`Import Tags from Text` is idempotent as the manual requires, and in two senses: a tag the track
already has is not re-added, and nothing existing is ever removed — so a tag added by hand survives
a re-run. Matching is case-insensitive, since a library holding both `#techno` and `#Techno` is
exactly the mess the feature exists to clean up. A tag runs from the marker to the next whitespace,
matching how the convention is actually written (`#PeakTime`, not `#Peak time`), and trailing
punctuation is trimmed so `#Techno, #Vocals` gives two clean tags.

Two rules the manual leaves open: replacing a tag with one the track *already has* is a removal
only, or the track ends up holding it twice; and replacing with an empty tag is refused rather than
silently becoming a delete.

Tag recipes apply directly rather than staging — tags live in the local cache, so there is no sync
step to carry them. A tag name with no existing tag is created in the first category, and the result
says which were invented.

Still missing: the **cue and beatgrid recipes** (14 ops), and the "other" recipes
(`Mark as Incoming`, `Remove from All Playlists`, `Import Date from Filesystem`).

*Epic* — **5**. Within the epic, the cue recipes depend on the cue-editing model from Epic 2, so
field/text/tag recipes came first as the spec advises.

---

## Import Tags From CSV

*What it does* — Bulk metadata import from a spreadsheet. Rows match either on the `Location`
column or on `Artist` + `Title` together; at least one matching strategy must be present. Produces
a report. Documented Excel caveats.

*decks status* — **partial.** `crates/track-matcher/src/csv_input.rs` parses CSV with a
column-mapping UI (`parse_csv_for_matcher`) but only to *match* tracks, never to *write* fields.

*Epic* — **5**.

---

## Undo History

*What it does* — Undo for smart fixes, playlist deletes, track deletes and edits. Retained for
**60 minutes or until restart**, whichever comes first. Ordered, so undo walks back through events.

*decks status* — **missing.** `decks` has something adjacent but different: a staged-change
pipeline (`Proposed → Accepted/Rejected → Exported/Applied`) that gates changes *before* they land.
That is arguably safer, but it offers no recourse once a change is applied.

*Epic* — **5**.
