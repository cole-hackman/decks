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

*decks status* — **done**, minus the fields we do not model. A chart above the playlist tracks —
and above a history set, per the spec — showing **BPM, Energy, Rating or Key**, with bars coloured
by **key** or by **BPM change** (green rose, red fell, grey held).

- **Heights scale within the set**, not against an absolute range: a warm-up running 118–124 shows
  its shape rather than six flat bars near the bottom of a 60–200 axis.
- **A missing tempo is `unknown`, not `same`.** "Unchanged" is a claim about two numbers; painting
  an absence grey would read as information the chart does not have.
- **A hover label carries the value and the direction**, so colour is never the only way to read
  the chart.
- **Hidden by default past 200 tracks**, per the spec's reasoning: a set-building tool, not a
  collection tool. Still available on request.
- It also counts **key changes that leave the wheel**, which is the fastest way to spot the one
  transition that will not work.

**Not offered:** Danceability, Popularity and Happiness — `Track` does not carry them (Epic 4).
Absent beats a flat empty chart.

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

*decks status* — **done**, except the drop target. Star from **Playlist Tools → Favourites**; the
bar pins above the track browser with a hotkey each — **`1`–`9` opens, `Shift+1`–`9` files the
selection**. Cache migration **v16**.

Notes:

- **The hotkey is the position, and positions are stable.** Un-starring closes the gap rather than
  leaving a hole — a key that quietly changes what it does between sessions is worse than one that
  does nothing.
- **Nine is the cap**, because that is where the hotkeys stop. A tenth star is refused with that
  reason rather than stored where nobody could press it.
- **A favourite whose playlist is gone is pruned on read**, from the table as well as the response,
  so the stored order and the shown order never disagree.
- Filing skips tracks the playlist already holds and reports how many, rather than staging
  duplicates or silently doing less than asked.

**Not done:** drag-and-drop onto a favourite. The hotkeys and the `+` button cover the same intent,
and the track table has no drag source yet.

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

*decks status* — **done**, all five. **Playlist Tools** in the sidebar; every tool previews before
it does anything, and everything it does is a staged change that goes through review and Sync.

- **Merge** creates a new playlist and leaves the sources alone; the preview reports how many
  duplicate rows were dropped, not just the final count.
- **Sort** needed a new `ChangeKind::PlaylistReorder`, which writes `djmdPlaylist.Seq`. The parent
  folder is part of the applier's `WHERE`, so a reorder cannot reparent a playlist by accident.
- **Cross Reference** warns before the `in none` mode, per the spec, and an empty selection returns
  nothing rather than the vacuous "in all zero playlists" answer.
- **Prefix** numbers in the order playlists were ticked, and `replace existing number` stops
  prefixes from stacking on a second run. A number that is part of the name (`7empest`) is not
  stripped — the signal is the separator.
- **Rewrite Order** stages a `PlaylistReorderTrack`. Tracks the sorted view left out are appended
  rather than dropped: a filter being active must not remove tracks from the playlist.

**Divergence on Rewrite Order.** Lexicon persists "the current visible sort" of the browser;
`decks` sorts inside the tool, on a field you pick. The browser's column sort is transient UI state
that is not plumbed to this view, and a button whose result depends on which column you last
clicked elsewhere is worse than one that states its input.

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

*decks status* — **done, except album art.** `E` opens the editor over the selection;
`changes::multi_edit` collapses the fields and plans the writes.

**The whole feature turns on one rule: a field the user did not touch is not written.** Open the
editor on forty tracks, change the genre, press Save — and the other nine fields must come out
exactly as they went in, even though the form had to show *something* in each of them. So the
form's state is not "the values", it is "the values plus which ones were edited", and
`FieldValue::Multiple` is a value the caller can never accidentally write because it is not a value
at all. In the UI it is a **placeholder**, not text.

Other decisions, each tested:

- **A missing value and an empty string are the same field state.** A form cannot tell them apart,
  and "clear this field" must not behave differently depending on how the field became empty.
- **One track missing the field is a disagreement**, not agreement on the value the others hold —
  otherwise the editor would show "House" while half the selection was empty, and pressing Save
  would be indistinguishable from doing nothing.
- **A track already holding the value produces no change.** Most of a forty-track selection is
  usually already right; staging forty no-ops would bury the two that matter.
- **Clearing a field is a real edit**, distinct from not touching it.
- Edits stage as `TrackMetadataEdit` and go through review and Sync. The editor never writes.

`Enter` saves, `Esc` discards, `Cancel` discards. **Not implemented:** `←`/`→` auto-saving
navigation between tracks, `Tab` to the Recipes page, and album art (replace / remove / Reload) —
`decks` has no album art anywhere, which is a separate feature rather than part of this one.

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

*decks status* — **done, except delete-from-disk.**

**The context-sensitive playlist rule** is implemented where the user actually archives: the track
context menu. From inside a playlist the action reads "Archive (and remove from this playlist)" and
stages a `PlaylistRemoveTrack` for *that* playlist only; from the browser it reads "Archive" and
stages nothing. The asymmetry is the point — from a playlist you are saying "not in this set", from
the browser you are saying "not in my way" — so the label says which one you are about to get.

Archiving itself is cache-only and takes effect at once; the playlist removal is a staged change
like any other and goes through review and Sync. The two halves are reported separately because
they land at different times.

**The selection helper** offers the spec's three criteria — archived over six months ago, without
cues, in no playlist — over the archive only. Two details worth recording: "older than 0 days" does
**not** sweep up something archived a second ago (almost certainly a misclick on the way to picking
a real threshold), and a criterion that matches nothing says so rather than silently clearing the
selection.

**Cleanup** is where tracks finally leave every playlist, not archiving — which is exactly what
makes archiving safe to do on a whim. It stages the playlist removals *before* the track deletes,
so the playlist rows are never left pointing at a track that no longer exists.

**Delete-from-disk is deliberately not implemented**, on the same grounds as Find Broken Tracks: it
is the one operation with no undo, and a program whose first rule is that the library is read-only
should not be the thing that deletes a DJ's audio. The confirmation dialog says so in as many
words. If this is wanted it should be an explicit decision with its own guard rails, not something
that arrives as part of a cleanup button.

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

*decks status* — **done, except the extra artist fields.**

**Locking** persists in cache migration v14, scoped by kind (`genre` | `artist`) because the same
string can be a good genre and a misspelt artist — but **not** by library: a value the user has
declared canonical is canonical for *them*, and re-locking the same fifty genres per library would
defeat the point. Right-click a chip to toggle. A locked value cannot be selected at all, and
`Cmd/Ctrl+A` selects everything *unlocked* — which is the half that matters, since select-all is
the gesture most likely to sweep a good value into a rename. Locking something already selected
deselects it, or it would sit there selected and unselectable, reading as the lock not working.

**Pinned letters** persist the same way. The letter bar only offers letters actually present, so it
never advertises a dead jump; non-alphabetic values group under `#`.

**Alt/Option-click filters the browser.** Genre has a real filter dimension; artist does not, so it
goes through the search box — which searches artist among other fields, making the result a superset
rather than an exact match. Worth naming rather than pretending otherwise.

**Sort by track count (default) or name.** Count sorts descending with name as the tie-break, or
equal counts would shuffle between loads.

`Esc` clears the selection.

**Not done: the extra artist fields** (`Remixer`, `Producer`, `Composer`, `Lyricist`). `decks`'s
`Track` does not model them and the core `SELECT` does not read them — this is the same gap that
puts `label`, `mix` and `colour` out of scope, and it belongs with whichever epic widens the track
model rather than being bolted on here.

*Epic* — **5**.
