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

**Status in `decks`:** nine of the eleven are implemented in `crates/recipes::cues` and reachable
from the Recipes panel's Cue Recipes section. Deliberate divergences and omissions:

- **`Change Active Loops` and `Half/Double BPM` are not implemented.** The first needs a
  `djmdCue` "active loop" column `decks` does not model; the second has to move beatgrid markers,
  which means writing an ANLZ file — that is a beatgrid recipe wearing a cue recipe's name, and it
  belongs with the beatgrid category below.
- **`Shift Cues/Beatgrid` shifts cues only.** The grid half is the same ANLZ write. Loops move
  whole rather than having only their start shifted, which would silently resize them.
- **"Random" is spelled `Cycle` and is deterministic**, walking an eight-colour palette in track
  order. A preview that showed different colours from the apply would be worse than no preview.
- **"First" and "last" mean first and last in the *track*,** not in storage order — `djmdCue` rows
  come back in insertion order, and a user means the timeline.
- **`Sort Cues` reassigns hot-cue slots 1–8** in the new order, because `djmdCue` stores no cue
  ordering of its own. Memory cues have no slot and stay put; a ninth hot cue keeps its slot.
- **`Quantize Cues` reports "this track has no beat grid"** rather than quietly changing nothing,
  per ADR-0008. It preserves loop length instead of snapping both ends independently.
- **`Remove Cues by Label` refuses an empty needle** — it would match every named cue, which is
  `Remove Cue Text`'s job.

Everything stages as `CueMetadataEdit` / `TrackDeleteCue` and goes through Sync; nothing here
writes to `master.db` directly.

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

The three **"other" recipes** are done as well. Each reaches into a different subsystem, so they run
one at a time rather than joining the ordered recipe list, and the UI states what each does before
it runs — `Remove from All Playlists` leaving smartlists alone is exactly the sort of thing that
otherwise reads as the recipe having missed some.

`Import Date from Filesystem` takes the file's **modification** time, not its creation time:
creation time is not portable (Linux has no reliable `birthtime`), and a file copied between drives
keeps its mtime while its ctime becomes the copy date — worse than useless as a release year.

Still missing: the **cue and beatgrid recipes** (14 ops).

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

*decks status* — **done, differently.** `decks` gates changes *before* they land — a staged-change
pipeline (`Proposed → Accepted/Rejected → Exported/Applied`), an opt-in Sync, and a `WriteGuard`
backup. That covers the change you never should have accepted; it does nothing for the one you did.

Undo closes that. Every Sync run records the **inverse** of each change it applied
(`crates/changes::undo`, `undo_runs` / `undo_entries` in cache migration v13). Undoing stages those
inverses as ordinary proposed changes, so they go back through review and the same guarded Sync.
Two steps rather than one, which is the right trade for a program whose first rule is that
`master.db` is read-only: there is no second write path, and no change reaches the library without
the user seeing it.

Deliberate divergences:

- **Retention.** Lexicon expires undo after 60 minutes or on restart. `decks` keeps the last **50
  runs per library**: the cache is already persistent, and noticing a bad sync the next morning is
  at least as common as noticing it within the hour. A count bound rather than a clock bound,
  because "the last fifty syncs" is something a user can reason about and "anything since 09:14"
  is not.
- **Not everything can be inverted, and the UI says which.** Per ADR-0008 a blocked entry carries a
  named reason rather than being silently omitted — an undo that quietly restored eight of twelve
  edits would be worse than one that restored none.

| Change kind | Undo |
|---|---|
| Metadata / cue / relocate edits, playlist rename, reorder | reversible — the inverse swaps the change's two ends |
| Playlist add ↔ remove track | reversible — same payload, opposite verb. A re-added track lands at the end |
| Cue deletion | reversible **when the deletion recorded the cue**. The cue recipes do; the restored cue gets a new row id |
| Add cue, create playlist | blocked — the new row's id is generated inside the apply transaction, so there is nothing to point a delete at |
| Delete playlist, delete track | blocked — the contents were not recorded first. Points at the backup Sync took |
| Track create | n/a — export-only, Sync never wrote it |

*Epic* — **5**.
