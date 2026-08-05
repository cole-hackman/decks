# 02 — Library: Browser, Playlists, Tags, Editing

---

## Track Browser

*What it does* — The primary screen. Click any field **twice** to edit it inline.

*Keyboard navigation* — `Tab` next field · `Cmd/Ctrl+Home` / `End` first/last field of the track ·
`Cmd/Ctrl+↑` / `↓` same field on the track above/below. Spreadsheet-style, which is the right model
for bulk tag work.

*Columns* — right-click any header to toggle additional columns.

*Track previews* — an inline waveform per row, clickable to audition instantly (10s default, `Esc`
stops). Previews are generated when a track is played or during waveform analysis; changing
waveform colours requires re-analysis for previews to update.

*Compatible-key indicator* — when a track is loaded in the player, harmonically compatible tracks
are marked in the browser with a white music symbol, honouring the global Key Mixing Mode
(Harmonically Compatible vs Fuzzy Key Mixing — see [`04-analysis.md`](04-analysis.md)).

*Search* — hover a column header to reveal its filter; multiple columns filter simultaneously.
Accent- and case-insensitive. Full operator set documented in
[`03-smartlists.md`](03-smartlists.md#smartlists). Key search auto-converts notation (`4M` finds
`Am`). Dates use `YYYY-MM-DD` with `>` / `<`.

*Custom tag search syntax* — a compact language worth copying verbatim:

| Query | Meaning |
|---|---|
| `Techno, House` | has Techno **OR** House |
| `~House, Vocals` | leading tilde makes all tags **required** — House **AND** Vocals |
| `!Techno` | does **not** have Techno |
| `~Techno, !Vocals` | has Techno **AND** does not have Vocals |

Only **full label matches** — partial matching is deliberately unsupported for speed.

*Sorting* — click any column. Sorting by `#` restores the playlist's original order. Custom Tags
sort by tag *count* per track.

*decks status* — **partial.** Virtualized table with resizable columns, inline per-column search,
multi-select, right-click actions, Camelot-tinted keys, an Energy column and inline tag chips all
exist. Missing: inline per-row waveform previews, the `None` keyword, numeric/date operators, the
tag query language, key-notation-aware search, the compatible-key indicator, and spreadsheet
keyboard navigation.

*Epic* — **1** (operators, shared with the rules engine), **2** (compatible-key indicator).

---

## Track Timeline

*What it does* — A chart above the browser showing a playlist's flow: Key + key compatibility, BPM,
Rating, Energy, Danceability, Popularity, Happiness. Customisable via a `Customize` button;
toggled from the View menu.

*Bar colour modes* — `Key` (matches the browser's key colours) or `BPM change` (green if BPM rose
versus the previous track, red if it fell, grey if unchanged). The BPM-change mode is a genuinely
good idea for reading set flow at a glance.

*Large playlists* — hidden by default beyond a size threshold, since it's a set-building tool, not
a collection tool. Also appears for history sets.

*decks status* — **missing.**

*Epic* — **6**.

---

## Playlists

*What it does* — Nested folders, playlists and smartlists in one tree.

*Notable behaviours*

- `New playlist & add tracks` from the Playlists header creates a playlist from the current
  selection **without losing that selection**.
- Drag tracks onto the tree to move them between playlists; drag **files from the OS** onto a
  playlist to import and add in one gesture (already-present tracks are added to the playlist, not
  duplicated in the library).
- **Playlists From Filesystem** — drag a *folder* onto the tree and its structure becomes a
  playlist folder hierarchy. Dragging an M3U/M3U8 does the same.
- Manual drag-to-reorder tracks works **only when the playlist is unsorted** — click the column
  header until the sort arrow disappears.

*decks status* — **partial.** Playlist panel, playlist detail, duplicate badges and staged
create/rename/delete/add/remove/reorder changes exist. Missing: folder-drop import, M3U import,
drag between playlists, create-from-selection.

*Epic* — **6**.

---

## Favorite Playlists

*What it does* — Star any playlist to pin it above the track browser as a drop target. **Hotkeys
per favourite** jump to that playlist or add the selection to it. A fast filing system.

*decks status* — **missing.**

*Epic* — **6**.

---

## Playlist Tools

| Tool | Behaviour |
|---|---|
| **Merge** | Combine N playlists into one new playlist; duplicates dropped |
| **Sort** | Sort the playlists themselves (A–Z, Z–A, track count) in place — *not* the tracks inside them |
| **Cross Reference** | Tracks common to N selected playlists; alternatively, tracks in the library that are in *none* of them (warns this can be huge) |
| **Prefix** | Prepend text, or an incrementing number with optional leading zero and optional replacement of an existing number prefix |
| **Rewrite Order** | Persist the current visible sort as the playlist's *stored* order |

**Rewrite Order deserves attention.** It has no visible effect inside Lexicon — its entire purpose
is that CDJs and Denon hardware can only sort by a few columns and know nothing about Energy or
Danceability. Sort by Energy in Lexicon, rewrite the order, and the playlist arrives on the gear in
that order. For a Rekordbox-first tool this is high-value and cheap.

*decks status* — **missing**, all five.

*Epic* — **6**.

---

## Custom Tags

*What it does* — Two-level categories → tags, fully user-defined. Categories carry a colour and are
drag-reorderable; tags are drag-movable between categories; double-click renames; right-click
deletes (removing the tag from every track).

*Applying tags* — via the Tags column popup in the browser, or in bulk via tag Recipes.

*The selection semantics on the Custom Tags page* — selecting multiple tags means **OR within a
category, AND across categories**. Selecting Techno + Drum & Bass in Genre and No Vocals in Vocals
yields *(Techno OR D&B) AND No Vocals*. This is the same two-level shape as smartlist OR clauses —
implement it once.

*Importing* — Rekordbox 6/7 **MyTags import automatically**. Hashtag-convention comments
(`#Techno #Vocals`) import via the `Import tags from text` recipe; unknown tags land in an
`Imported Tags` category for the user to sort.

*Exporting* — only via the Field Mapper. `All Custom Tags → Comment` writes the hashtag form; a
single category can be the source instead. **Rekordbox is limited to 4 MyTag categories.**

*Keyboard* — `T` opens the tag popup; `↑`/`↓` navigate, typing filters (navigation stays within the
filtered set), `Enter` toggles, `Esc` saves and closes. Individual tags can be assigned a number,
and that number bound to a hotkey.

*decks status* — **partial, and well advanced.** Categories, tags, CRUD, usage-count badges, a
picker modal, bulk apply, the `T` shortcut, tag filter dimensions and inline chips all exist.
Missing: category colours, drag-to-reorder/move (backend `move_tag` exists; blocked on a
`reorder_tags` command), the OR-within/AND-across selection semantics, MyTag import, hashtag
import, per-tag number hotkeys, and Field-Mapper export.

*Epic* — **1** (selection semantics), **5** (imports).

---

## Manual Editing

*What it does* — Multi-select → track editor. Fields where the selection disagrees show
`<multiple values>`; setting one writes it to every selected track.

*Hotkeys* — `E` opens the editor · `←`/`→` move between tracks (auto-saving) when one was selected ·
`Tab` switches between the manual editor and the Recipes page · `Enter` saves and closes ·
`Esc` discards.

*Album art* — replace, remove, or `Reload` (re-read art changed outside Lexicon) for any number of
tracks.

*decks status* — **missing.** `decks` edits through staged changes and Smart Fixes; there is no
multi-track field editor and no album art anywhere.

*Epic* — **5**.

---

## Archive

*What it does* — Hide tracks from the whole app without deleting. Archived tracks keep cues and all
data, and appear only in the Archive view and in playlists they already belonged to.

*The playlist rule, which is subtle and correct* — archiving **from inside a playlist** removes the
track from *that* playlist immediately but leaves it in others. Archiving **from the main browser**
leaves it in every playlist. The stated intent: archive freely without breaking playlists.

*Cleanup* — a **Selection helper** auto-selects archived tracks by age, or those without cue
points, or those in no playlist. `Cleanup selection` removes from Lexicon, optionally from disk
(permanent). Cleanup is also the point at which tracks finally leave all playlists.

*decks status* — **partial.** `archived_tracks` table, `ArchiveView` with unarchive and
delete-from-library. Missing: the context-sensitive playlist rule, the selection helper, and
delete-from-disk.

*Epic* — **5**.

---

## Genre Cleanup / Artist Cleanup

*What it does* — Chip clouds of every distinct genre (or artist) with counts; select, type a new
value, save, and every matching track is rewritten. Artist Cleanup is the same tool over
artist-like fields.

*Shared interactions* — `Shift+Click` a chip auto-fills it as the new value ·
`Alt/Option+Click` filters the track browser to it · right-click **locks** a value so it can't be
selected accidentally (and locked values rank higher in autocomplete) · `Cmd/Ctrl+A` selects all
unlocked, `Esc` clears.

*Genre-specific* — **Import to tags** creates a tag per genre and applies it, safe to re-run. The
manual warns: clean genres *first*, or you import a mess.

*Artist-specific* — **Pin Letter** for alphabet navigation, persisted across sessions. Settings
extend the tool beyond `Artist` to `Remixer`, `Producer`, `Composer` and `Lyricist` — and enabling
a field means renames update it too. Sort by name or by track count.

*decks status* — **partial.** `CleanupPanel` handles both modes with list/rename/delete,
shift-click multi-select, and `import_genres_as_tags`. Missing: locking, pinned letters,
alt-click-to-filter, the extra artist fields, and sort modes.

*Epic* — **5**.
