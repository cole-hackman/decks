# Status

## 2026-08-07 — A drag source, and why ANLZ writes stay unbuilt

Two things: the last Library & browser gap, and an investigation that closes off four others by
saying no.

**Track rows are a drag source now**, which also lit up the Favorite Playlists drop target that had
been waiting on exactly this. One rule worth stating: dragging a row *inside* the selection carries
the whole selection, and dragging one *outside* it carries only that row without silently extending
the selection. Dragging one of five highlighted rows to mean only that row would make the highlight
a lie.

The drop carries its own ids rather than reading the current selection at drop time — the selection
can change between picking up and letting go, and the payload is the record of what the user
actually grabbed. The favourite only accepts a drag carrying our own MIME type, so the chip does
not light up for a dragged file and then do nothing.

**ANLZ writing: investigated, and deliberately not built.** Four `partial`/`missing` rows depend on
it — Beatgrid editing, the last two cue-point recipes, Don't Touch My Grids, Beatshift correction —
so it was worth answering once rather than four times.

Producing the bytes is the easy half. The format is self-describing (`PMAI` magic, big-endian
lengths, a chain of tagged sections), `for_each_section` already walks it correctly, and rewriting
a `PQTZ` section is mechanical.

What cannot be answered here is whether Rekordbox **accepts** a file we wrote: whether anything
beyond the length fields is validated, whether the `.DAT` and its `.EXT` companion must stay
consistent, and whether `master.db` carries state that has to change alongside it. The failure mode
is not data loss, but a rejected ANLZ leaves a track with no waveform and no grid in Rekordbox until
it is re-analysed there.

So it stays unbuilt rather than half-built. A writer we cannot verify is untestable production code
by the same argument that keeps the enrichment providers unwritten, and shipping it unwired would
break this project's own definition of done. The four rows now share one recorded reason instead of
four vague ones, and `GAPS.md` names the fifteen-minute check on a machine with Rekordbox installed
that would unblock all of them at once.

Library & browser is now **17 done / 0 partial** once the tree PR lands.

**Next:** with ANLZ ruled out for now, the remaining unblocked work is thin — Energy's defined
scale (ADR-0012 adopted `libebur128`; check whether it is wired) and the Rekordbox direct-DB-write
row, which may be a divergence rather than a gap. Album art and `crates/enrichment` still need the
provider decision.

## 2026-08-07 — Playlists move between folders

The playlists tree's last named gaps: folder-drop and drag-between. Both come down to one missing
change kind — `PlaylistReorder` writes `Seq` within a parent and **deliberately refuses** to change
that parent, because a reorder that silently restructured the tree would be a nasty surprise.

So `PlaylistMove` is its own kind, and it carries its own refusals — `djmdPlaylist` enforces
neither, and getting either wrong corrupts the tree:

- **The destination must be a folder.** Rekordbox nests under folders only; a playlist parented to
  a playlist is a shape nothing can render.
- **A folder cannot go inside itself or any descendant.** That detaches the whole subtree from the
  root: it still exists, and it is unreachable from the tree forever.

Three smaller decisions:

- **The UI mirrors both refusals rather than trusting the applier to catch them.** A drop that sync
  would reject should not look like it worked until the user opens the review table, so a folder
  only highlights when the drop would actually be accepted.
- **The ancestor walk goes upward from the destination**, not downward from the dragged folder — a
  tree is far wider than it is deep — and both the Rust and TypeScript versions carry a `seen` set,
  because a database that already contains a cycle must not hang the sync or the render.
- **`old_parent_id` rides on the change.** Without it `changes::undo` blocks the inverse and the
  tree cannot be put back, which for a drag — the easiest edit in the app to make by accident — is
  the one place undo really matters.

The move is **staged, not written**: the tree redraws from `master.db`, so the row does not appear
to move until Sync applies it. That is the honest behaviour for a change that has not happened yet.

Library & browser is now 16 done / 1 partial.

**Next:** the track table's remaining gap is a drag source, which would also unblock the Favorite
Playlists drag target. Beyond that the `partial` rows need ANLZ *writes* — worth establishing
whether that is feasible at all before committing to Beatgrid editing or the last two cue recipes.

## 2026-08-07 — Cue Destination, and a round-trip we do not need

Closed the Cue Destination row, mostly by working out that half of it is a problem `decks` does not
have.

**The half that was real** is the sync options `All to hot cue` / `All to memory cue` /
`All to hot and memory cue` — the spec calls it "how you copy hot cues into memory cues wholesale",
and it is the standard Rekordbox workflow, because hot cues do not show on every player and memory
cues do. That is now a `MirrorCues` cue recipe. `Both` is **idempotent**: a position that already
exists as both kinds is left alone, because this is an operation people run after every session and
a second run must not double the cue list.

**The half that is a divergence** is the hidden-duplicate round-trip. Lexicon's internal model has
hot cues only, so it *collapses* memory cues into hot cues on import and has to remember what it hid
in order to restore it on sync back. `decks` never imports — it reads `djmdCue` live and shows both
kinds as they are. Nothing is collapsed, nothing is hidden, nothing needs restoring. Building the
ledger would be machinery for a problem we do not have, and the per-cue `M` toggle is likewise a
state that cannot arise here: a cue already *is* one kind or the other.

**Two real bugs surfaced while wiring it up**, both in `diff_cues`, both silent:

- **A recipe that *adds* a cue had it dropped.** The diff walked the result list and skipped
  anything with no `before` to compare against, so `MirrorCues` produced an empty preview and
  looked like a no-op. Additions now stage as `TrackAddCue`.
- **A cue that changed *kind* staged nothing.** Nothing diffed the `memory` flag, so converting
  hot→memory silently did nothing too.

Neither could have been caught by the existing recipes, because none of them added a cue or changed
a kind — the new operation is the first to do either, and it found both.

**Next:** the playlists tree's folder-drop and drag-between; the last two cue recipes need an
unmodelled `djmdCue` column and ANLZ writes, so check feasibility before committing. Album art and
`crates/enrichment` still need the provider decision.

## 2026-08-07 — Auto-write file tags, and a blocker that had outlived its cause

Audited the four disabled Automatic Actions against what the last few PRs shipped. Three are still
genuinely blocked — drop detection, the Beatshift Fixer, the enrichment providers. The fourth was
not blocked at all.

**`AUTO_WRITE_TAGS` claimed it needed field mappings.** Those shipped in Epic 4, and Write Tags has
honoured them ever since; the Rekordbox profile landed earlier today. The blocker text had simply
outlived its cause, and a stale blocker is its own kind of lie about what the app can do — it reads
exactly like an honest "not yet" while being false. There is now a test asserting no action still
claims field mappings are missing.

So it ships. Three decisions:

- **It requires auto-analysis to be on.** Without it there is nothing new to write — the tags were
  read off that very file a few lines earlier, so writing them back would rewrite the user's file
  to no effect.
- **Only BPM and key.** Everything else came from the file. Every rewrite is a chance to lose a
  frame `lofty` does not model, and there is no reason to spend that risk on a no-op.
- **A confidence floor of 0.75, stated as a named constant.** Auto-writing overwrites whatever tag
  the file carried with nobody looking. ADR-0008 forbids presenting a guess as fact; writing one
  into the user's file is worse. Below the floor it is reported as a **skip with the reason** —
  a setting you turned on that silently does nothing looks broken.

The import summary now separates *analysed* from *tagged*. One reads the file, the other rewrites
it, and a count that blurred the two would hide the fact that files on disk changed.

**One divergence recorded rather than built:** the spec skips auto-tag-writing for bulk edits over
1,000 tracks. `decks` needs no such rule, because auto-writing only fires on watch-folder arrivals
— there is no bulk path that triggers it. If one is added, the cap comes with it.

**Next:** the hidden-memory-cue round-trip for Cue Destination; folder-drop and drag-between in the
playlists tree. Album art and `crates/enrichment` still need the provider decision, which also
unblocks the last of the Automatic Actions.

## 2026-08-07 — Find Lost Tracks, finished

The two outstanding halves of Relocate: merging onto a file another entry already claims, and the
5-minute re-check cadence.

**The merge did not need new playlist logic, and deliberately does not have any.** Relocating track
A onto track B's file means "these are the same track, keep one" — which is exactly what resolving
a duplicate group does. `relocate_merge` builds the plan and hands it to
`duplicates::plan_duplicate_resolution` / `resolve_duplicates`. A second implementation would drift,
and it would drift first on the case that took longest to get right there: a playlist that already
holds the keeper, where the loser is removed rather than swapped.

Three decisions:

- **Path comparison normalises separators and case.** Rekordbox stores whatever the OS handed it,
  so `D:\Music\B.mp3` and `d:/music/b.mp3` are one file. Treating them as distinct would create
  exactly the two-rows-one-file state the spec's constraint exists to prevent — a collision-check
  that misses collisions is worse than none.
- **Keeping the existing entry moves no path.** The file was already correctly attached to a row;
  the missing row was the mistake. Only the other branch stages a `TrackRelocate`.
- **That relocate records no old path.** The track is missing, so its stored path points at nothing
  — writing it into the change would hand undo a known-broken path to restore.

**The cadence is an in-memory memo**, which is what makes the spec's "restarting forces a re-check"
free: a fresh process has nothing to serve. Exactly five minutes counts as **stale**, because "at
most every 5 minutes" puts the boundary on the re-scanning side. A clock that moved backwards
expires the memo rather than freezing it — a naive `now - then < window` treats a negative age as
fresh forever. And forcing a check **invalidates** rather than bypassing, so the forced answer is
also what everything else then sees; a bypass would leave the browser on a stale list while the
Edit popup showed a fresh one.

The frontend query's `staleTime` was `Infinity`, which meant a file restored on disk stayed marked
missing until the app restarted — the shell would have re-scanned happily, but nothing ever asked.
It now matches the shell's five minutes.

**Next:** Automatic Actions (only auto-analyse of five is wired; several were blocked on fields
`Track` now carries), the hidden-memory-cue round-trip for Cue Destination. Album art and
`crates/enrichment` still need the provider decision.

## 2026-08-07 — Modified Sync gets a watermark; Full-Sync delete does not exist

Two halves of one parity row, and only one of them was buildable.

**Modified Sync is done.** Cache **v20** stores a watermark per `(library_path, app)`, stamped
after a sync that actually wrote something. The bug it fixes was live: `since_ts` defaulted to `0`
when the caller did not supply one, so Modified Sync quietly behaved as a Full Sync over the whole
library — the exact opposite of what the mode promises, and invisible unless you counted the rows.

Four decisions:

- **Absent is not zero.** `None` means no sync has ever run and locks the mode; `0` would mean
  "synced at the epoch" and unlock it over everything.
- **Locked with a reason, not hidden.** The option is shown disabled, saying there is no "since" to
  sync from yet. A mode that appears to work and does nothing reads as a bug in the app.
- **Stamped only when something was written.** A run that applied nothing has not established a new
  baseline; stamping one would drop whatever it failed to write out of the next window.
- **Forward only**, enforced in SQL with `MAX(...)` on conflict. A watermark that could rewind
  would re-propose changes the user has already dealt with.

**Full-Sync delete is not a gap — it is a divergence, and it is going in the docs as one.** Lexicon
means "the DJ app becomes a mirror of Lexicon; anything not in Lexicon is removed from the app".
That works because Lexicon owns a library and the DJ app is downstream of it. `decks` has no such
library: it *reads* `master.db`. There is no set of tracks Rekordbox holds and `decks` does not, so
"remove anything not in `decks`" has no referent — and the nearest literal implementation would
delete the user's entire collection. The row stays `partial` with the reason written down rather
than being closed with something dangerous or something fake.

**Next:** Find Lost Tracks' merge-with-existing and re-check cadence; more of the Automatic Actions
group; the hidden-memory-cue round-trip for Cue Destination. Album art and `crates/enrichment`
still need the provider decision.

## 2026-08-07 — Field Mappings reach the library

Mappings have projected Energy and Custom Tags into ID3 frames since Epic 4. They now reach
`master.db` too, under a second profile — which closes the last `Field Mappings` gap that was not
a deferred non-Rekordbox adapter.

The decision that shaped it: **previewed and staged, never applied directly.** "Apply mappings on
sync" could have meant transforming values inside the applier, and that would have been wrong. A
mapping rewrites Comment or Genre across the entire library; it is the most destructive shape of
edit this app can make. Every other bulk operation here goes through the staged-change pipeline so
the user sees the diff and can reject rows, and there is no argument for this one being the
exception. So it means *stage the edits sync will write* — they land in the review table and reach
the database through the same `WriteGuard` as everything else.

Three supporting calls:

- **A mapping that reproduces the current value is not a change.** Without that guard the second
  sync stages the whole library again and buries the edits that matter.
- **Targets come from `changes::applier::writes_field`**, not a second hand-kept list, so a target
  the UI offers is one sync will actually write. A mapping onto anything else is named in the
  preview rather than dropped — a mapping that silently vanishes looks like data loss.
- **Two profiles, not one list applied twice.** An audio file has no Rating frame worth writing and
  `djmdContent` has no album-art column. A shared list would offer targets that do nothing on one
  side, and the target picker resets when the destination changes for the same reason.

Danceability, Popularity and Happiness are deliberately left absent from `MappingInput` rather than
defaulted: they are blocked upstream (ADR-0012), and writing a zero we did not measure would be a
guess presented as a fact.

**Next:** the per-app modified watermark and Full-Sync delete for `Full / Playlist / Modified
sync`; Find Lost Tracks' merge-with-existing; more of the Automatic Actions group. Album art and
`crates/enrichment` still need the provider decision.

## 2026-08-07 — Custom Tags, finished

The four remaining Custom Tags gaps, closed together because they share a migration: category
colours, reorder, per-tag number hotkeys, and Field-Mapper export. Cache migration **v19** adds
`tag_categories.color` and `tags.hotkey`, both nullable — a category with no colour is the normal
state, and a default would make every existing category silently claim one the user never picked.

Four decisions:

- **`reorder_tags` takes the whole new order**, not a `(tag, position)` pair. A drag produces a
  complete order anyway, and applying it wholesale means there is never a window where two tags
  share a `seq`, which a shift-everything-down-by-one approach would have. Ids from another
  category are ignored rather than moved in — that is `move_tag`'s job, and doing it implicitly
  would let a reorder silently restructure the tree.
- **Reorder is on the keyboard, not only the mouse.** `Alt`+`←`/`→` moves the focused chip. This is
  the same class of bug as the FindPopup hover-only buttons: a gesture that only exists for a mouse
  makes the feature unreachable without one, and jsdom does not run drag events so only a
  deliberate keyboard path is testable at all.
- **A hotkey is global, and assigning a taken one steals it.** Refusing would send the user hunting
  through every category for whichever tag holds `3`. The theft is visible at once — the other
  tag's number is simply gone — and stealing is what assigning a keyboard shortcut normally means.
  Both statements run in one transaction, so a steal cannot clear the old binding and then fail to
  set the new one.
- **Clearing is its own choice.** "No colour" is a button in the colour menu, and the hotkey select
  has an explicit empty option, rather than either being reachable only by picking something else.

**A real bug found on the way.** The Field Mapper has offered a `Colour` source for some time, and
it produced nothing: `MappingInput.colour_name` was never populated, because `Track` had no colour
to populate it from. That is a control that did not do what it said — exactly what the no-stub rule
in `CLAUDE.md` is about. It works now. And per-category tag export, which the spec asks for
explicitly ("a single category can be the source instead"), was supported by the engine all along
and simply never offered in the UI; it is now, keyed by category **name** so a rename stops matching
rather than quietly exporting a different set under the old label.

Custom Tags moves to `done`. Library & browser is now 15 done / 2 partial / 1 missing.

**Next:** the remaining `partial` rows need either Epic 2 depth (beatgrid writes, active loops) or
the enrichment provider decision that album art and `crates/enrichment` are waiting on.

## 2026-08-07 — Colour, written

`Colors → nearest` was the last row sitting in the `blocked` column for a reason that had stopped
being true. `Track` gained a colour field an hour ago; what was still missing was anything that
wrote one back. Both halves exist now, so the row is `done`.

The interesting constraint is that **colour is not like Genre or Label**. Those are free-text
vocabularies — `apply_fk_edit` happily creates a `djmdGenre` row for a genre nobody has used
before, and that is correct. `djmdColor` is not a vocabulary. It is a lookup table of the eight
colours the hardware can display, and inserting a ninth would give a track a colour that renders on
no CDJ. So colour gets its own path, and that path **never creates a row**: an unmatched colour is
a warning and a skip.

`change_to_nearest_color` then decides what happens to a colour that is recognisable but outside
the palette:

- **Off** (the default) — nothing is written, and the skip is reported per track. This is the
  spec's own wording: "Off means no colour is written when there's no exact match."
- **On** — mapped to the nearest palette entry by RGB distance, and **every mapping is named in the
  warnings**. Opting in is not a reason to hide which tracks had their colour changed to one the
  user did not pick.

Three smaller calls:

- **Plain RGB distance, not CIELAB.** Eight widely separated hues; the two agree on every input
  that matters, and a colour-science dependency to break ties between Pink and Red for a track
  label is not a trade worth making. Written down so it reads as a choice rather than an oversight.
- **Something that is not a colour is never approximated**, even with the option on. `Chartreuse`
  is not a failed match — it is not a colour we can read, and mapping it would be invention.
- **Clearing a colour needs no permission.** Removing a value invents nothing, so `null` and the
  empty string clear it whatever the option says.

Also new: `changes::applier::writes_field`, so the layers that *offer* fields — the multi-track
editor, recipes, CSV import — assert against the applier's actual allowlist instead of keeping a
parallel copy that drifts. A field offered by the editor but rejected by the applier gives the user
a form control whose value vanishes at sync time, which reads as data loss rather than as an
unsupported field. `label` and `color` join that vocabulary in the same change, so both are
editable from the browser rather than merely visible.

**Next:** Custom Tags leftovers (category colours, `reorder_tags` for drag-reorder, per-tag number
hotkeys, Field-Mapper export). Album art and `crates/enrichment` still need the user's provider
decision — MusicBrainz + Cover Art Archive recommended, Discogs as an opt-in second source.

## 2026-08-07 — `Track` grows five fields

The whole stack (#10–#33) is merged; `main` carries every epic through 6 and CI is green on it.
With one branch again, the schema widening that six parity rows were waiting on became cheap, so
that is what this is.

`Track` now carries **Label**, **Remixer**, **Mix**, **Colour** and **Date added**, read from
`djmdLabel`, `djmdArtist` via `RemixerID`, `djmdContent.Subtitle`, `djmdColor` and
`djmdContent.DateCreated`. Writing `Label` already worked — `changes::applier` has treated it as a
foreign-key edit since Epic 5 — so the browser could set a label it could never show back. That
asymmetry is gone.

Four decisions worth recording:

- **The SELECT is built per connection, not constant.** Naming an absent column fails the *whole*
  query, and these five are exactly the ones an older or migrated library may not have. Each is
  probed and degrades to `NULL`, so a library without `LabelID` keeps returning all its tracks and
  simply has no Label. Losing every track read to gain a column would be a bad trade. The helpers
  `cues` grew for the same reason moved into `queries::columns` and are now shared.
- **Colour is read by name, not id.** An id means nothing outside the database it came from. The
  name lives in `djmdColor.Commnt`, not `Name` — a genuine Rekordbox quirk, so it is `COALESCE`d
  and both halves are exercised by the seed.
- **Date added is not parsed.** The column is sometimes a date and sometimes a full timestamp
  depending on how the library was migrated. It is compared lexicographically, which is correct for
  ISO-8601, and `equals` is a **prefix** match so `2025-03` means "during March". Parsing to a
  fixed precision would take that away and invent certainty the column does not have.
- **A numeric range cannot satisfy a date `between`.** `Value::TextRange` is separate from
  `Value::Range` and the mismatch fails closed rather than comparing a date against a float.

What this unblocks, in the same change: Label / Mix / Remixer / Colour / Added columns in the
browser; five new smartlist fields plus a `date` field kind with its own operator set; and two of
Mixable Tracks' four missing rules — `Match colour` and `Recently added`, taking it from 9 of 13
to 11 of 13.

**And a correction to my own bookkeeping.** `Danceability / Popularity / Happiness` was recorded as
`missing`, which reads as "not built yet". It is not: ADR-0012 already established that Lexicon
sources all three from Spotify's `audio-features` endpoint, deprecated 2024-11-27 and returning 403
for applications registered since — and that Popularity is a catalog metric no local analysis can
produce. That is `blocked`. The Mixable panel's own notice said the same wrong thing and now says
why instead. `Colors → nearest` moved the other way: it was `blocked` on a missing colour field,
which now exists, so it is `partial` — read-only until a change kind writes `ColorID`.

**Next:** album art and `crates/enrichment` are the largest remaining Epic 4 items, and they need
one decision from the user — which metadata provider. MusicBrainz + Cover Art Archive is the
recommendation (no key, no account, free); Discogs would be an opt-in second source needing a
keychain token.

## 2026-08-06 — Hashtag imports land in `Imported Tags`

Chasing the last Custom Tags gaps turned up a docs error of my own: `02-library.md` listed hashtag
import as missing. It was not — `recipes::parse_hashtags` plus the tag-recipe preview/apply path
has covered it since Epic 5.

What *was* missing is the spec's destination. Per `docs/lexicon/02-library.md §Custom Tags`,
"unknown tags land in an `Imported Tags` category for the user to sort". Invented tags went into
whichever category happened to be first, which is arbitrary: it quietly fills a real category like
Genre with unsorted imports, and afterwards there is no way to tell which tags the user put there
and which the importer did.

They now go to a reserved `Imported Tags` category — created **on demand**, so a library that never
imports never grows an empty one, and matched case-insensitively so someone who made one themselves
gets theirs rather than a second.

That also fixes a smaller bug in the same path: importing into a library with **no** categories
failed outright with "create a tag category before importing tags", which made the first import on a
fresh library impossible.

**Next:** what remains in Custom Tags is cosmetic or blocked — category colours, drag-reorder
(needs a `reorder_tags` command), per-tag number hotkeys, Field-Mapper export. Epic 7 (streaming)
still needs a scoping decision from the user.

## 2026-08-06 — MyTag import

Rekordbox's own tag system, imported into Custom Tags. Per
`docs/lexicon/02-library.md §Custom Tags`. `djmdMyTag` / `djmdSongMyTag` added to the synthetic
schema and seed, a read-only `queries::mytags`, and a preview-then-apply flow on the Custom Tags
page.

Rekordbox keeps categories and tags in the **same** table — a category is a row whose `ParentID` is
the root, a tag one whose parent is a category, with `Attribute` telling them apart. Same
self-referencing shape as playlists and folders.

Four decisions:

- **Preview, then apply.** The spec imports automatically; this does not. It merges a second
  taxonomy into the user's own tag tree, and doing that unannounced is how a tag list becomes
  unusable.
- **Matched by name**, case- and whitespace-insensitively, at both levels. Rekordbox ids are not
  stored — an id means nothing outside its database, and the name is what the user recognises.
- **Idempotent.** A second import creates nothing and says "nothing to do" rather than reporting a
  hollow success.
- **Soft-deleted rows skipped on both levels.** A deleted category takes its tags with it;
  importing a tag the user threw away would recreate exactly what they removed.

Links pointing outside this library are counted and surfaced, not hidden — a large number means the
MyTag data came from a different collection.

**This time `pnpm e2e` ran before the push**, which is the correction to the process failure logged
in the cue-presets entry below. All 59 pass.

**Next:** what is left in Custom Tags is cosmetic or blocked (category colours, drag-reorder needing
a `reorder_tags` command, hashtag import, Field-Mapper export). Epic 7 (streaming) still needs a
scoping decision from the user.

## 2026-08-06 — Inline per-row waveform previews

A `Wave` column in the browser, drawing each track's ANLZ preview at forty bars. Per
`docs/lexicon/02-library.md §Browser`. This was the last unblocked `missing` row outside streaming.

Four decisions make it viable over a four-thousand-track library:

- **Downsampled in Rust.** `anlz::downsample_preview` squashes ~400 points to forty bytes before
  the data crosses IPC. Shipping the full preview to draw forty bars moves two orders of magnitude
  more than the picture contains.
- **Peak per bucket, not mean.** Averaging flattens what the preview is for — a quiet intro with
  one stab should show the stab.
- **Batched per visible page, cached for the session.** A track asked for once is never asked
  again, *including* when the answer was "no waveform"; without that, every scroll past an
  unanalysed track re-reads the disk to be told the same thing.
- **Absence is not silence.** No ANLZ means the track is missing from the response and renders as
  nothing. Zeroes would draw a flat line, which is a claim about the audio.

One structural wrinkle worth recording: the rows a batch is fetched for come from the virtualizer,
which needs the table, which needs the columns — so the columns cannot depend on the waveform state
directly. A ref breaks the cycle, and the `setWaveforms` that lands each batch re-renders the
component anyway, so the cells pick it up. Passing the map into `buildColumns` instead would rebuild
every column on each batch and reset column widths mid-scroll.

**Next:** `Library & browser` now has one `missing` row left (album art, which the product does not
model at all). Epic 7 (streaming) still needs a scoping decision from the user.

## 2026-08-06 — Cue presets

The spec's "Cue templates" — saved name+colour pairs stamped onto individual cues. Per
`docs/lexicon/05-cues-player.md §Cue templates`. Cache migration **v18**, `cache::CuePreset`, five
IPC commands, a preset bar in `CueEditor`.

**Renamed on purpose.** `crates/cue-generator` already owns `CueTemplate` for its bulk-generation
rule sets. Two things called "template" in one player would be unreadable, so these are presets,
and the migration comment says why.

Four behaviours worth recording:

- **Immutable**, per the spec. No update path; changing one means delete and re-create. That is
  what keeps the hotkey a stable promise — `2` applies what `2` applied last set.
- **Deleting closes the gap.** The first eight carry hotkeys 1–8; a hole would retire a key while
  the ones after it kept their old numbers. Reordering is how a preset's hotkey changes.
- **Applying stages, never writes.** One `CueMetadataEdit` per field that actually changes, so a
  colourless preset stages only the name and re-applying the same preset stages nothing.
- **Not scoped by library**, unlike `favourite_playlists`. A preset describes how this DJ labels
  cues, not anything inside a particular database.

Two divergences from the manual, both written down in `05-cues-player.md`: duplicate names are
allowed, and applying goes through an explicit **target** cue rather than the playhead's position.
Position-based targeting reads well in prose and badly in practice — "exactly on a cue" is a
millisecond comparison the user cannot see, and getting it wrong stamps a preset onto the wrong cue.

**One bug this shipped and then fixed.** `listCuePresets` resolving with `null` — which is what
every e2e spec that does not mock the command gets back — put `null` into state, and the next
`.length` threw. Because the cue editor lives in the inspector, that unmounted the *whole panel*,
not just the preset bar: clicking a track appeared to do nothing. Twelve e2e tests caught it. The
loader now guards the shape as well as the rejection, and there is a unit test pinning it. The
lesson: a `catch` covers a rejected promise, not a resolved one of the wrong type.

With this, **Player, cues and generator has no `missing` rows left**.

**Next:** inline per-row waveform previews is the last unblocked `missing` row outside streaming.
Epic 7 still needs a scoping decision from the user.

## 2026-08-06 — Find Popup

`Cmd/Ctrl+F` over playlists, smartlists and tracks in one box, with per-result actions. Per
`docs/lexicon/00-overview.md §Find Popup`. It consumes the play queue shipped in the previous
commit — "Queue" on a track result is the spec's "add to the play queue".

Deliberately **not** merged into the Action Center: `Cmd+K` searches commands, this searches
content. One box over both would have to rank a track title against "Toggle Sidepanel".

`lib/find.ts` holds the ranking, pure and synchronous — the library is already in memory for the
browser, so a round-trip per keystroke would be slower and worse. Four decisions:

- **Three match tiers, not fuzzy matching.** Fuzzy subsequence matching suits a palette of a
  hundred short fixed strings; over four thousand track titles it matches almost everything and
  ranks by noise.
- **Each section caps independently**, and containers sort before tracks, so a big library cannot
  bury the one playlist that matched.
- **Ties break alphabetically.** Without it the same query returns a different top result after any
  re-sort, and `Enter` plays something other than it did a moment ago.
- **An empty query returns nothing.**

Two things worth recording:

- The Playwright run caught that the per-result buttons were **hover-only** — unreachable in a
  popup driven entirely by the keyboard. They now show on the highlighted row too. That was a real
  accessibility bug, not a test artefact.
- `useActions` and `useKeyboardShortcuts` each keep their own `isEditable`, and they have now
  **diverged on purpose**: application actions like `Cmd+F` must fire while the track table has
  focus, while widget-internal bindings like the bare arrows must not. The stale "same rule as"
  comment left by the browser-nav commit is corrected.

**Next:** cue templates and inline per-row waveform previews are the remaining unblocked rows.
Epic 7 (streaming) still needs a scoping decision from the user.

## 2026-08-06 — Play queue

The transport played one file and emitted `playback-ended`. This is the list that decides what
happens next. Per `docs/lexicon/05-cues-player.md §Music player`.

`lib/play-queue.ts` holds the list arithmetic as pure functions — advancing past a track removed
while it played, shuffling only the part that has not played yet — so it is testable without a
`rodio` sink. `usePlayQueue` resolves ids to tracks and drives the transport; `PlayQueuePanel`
draws it.

Four decisions:

- **Track ids, not tracks.** The queue survives a library refresh, a filter change and a re-sort.
  Holding whole `Track` objects would pin stale copies of rows the user has since edited.
- **One list with a marker in it**, not a "now playing" box above a separate list.
- **`Clear` keeps the playing track.** Clear means "nothing after this", not "stop the music".
  `Shuffle` likewise only permutes what has not played — shuffling history would move the marker
  under the playing track and read as the queue losing its place mid-set.
- **Advance is driven by a timestamp, not a flag.** A boolean `ended` would stay `true` after the
  first end and the second track would never advance. There is a test for exactly that.

`useAudioPlayer` gained `endedAt`; `useKeyboardShortcuts` was untouched. The queue is
**per-session and in-memory** — a queue is what you are about to play right now, and persisting it
would mean opening the app tomorrow to last night's leftovers.

**Next:** Find Popup (`Cmd+F`) is the natural follow-on — it consumes the queue for its
per-result "add to queue" action. After that, inline per-row waveform previews and cue templates.
Epic 7 (streaming) still needs a scoping decision from the user.

## 2026-08-06 — Spreadsheet keyboard navigation

The browser could move rows with j/k and the arrows. What it lacked was a **cell cursor** — a
focused (row, column) pair you can walk, page through, and open for editing without the mouse.
Per `docs/lexicon/02-library.md §Browser`.

`lib/grid-nav.ts` holds the movement rules as pure functions, so the edge cases are testable
without rendering a virtualized table. Movement **clamps rather than wraps** in both axes: holding
`↓` in a 4,000-track library stops at the bottom instead of returning to the top and letting the
next keystroke edit the wrong track, and `→` at the last column is a no-op rather than a jump to a
different row.

Inline editing opens on `Enter`, `F2` or any printable character, and **stages** through
`multi_edit_apply` — a `TrackMetadataEdit` for review and Sync, exactly like the multi-track
editor. Inline editing is a faster way to propose a change, not a way around the pipeline. Only
fields the applier will actually write are editable; Energy, Time and Tags say `aria-readonly` and
refuse to open, because a cell whose value silently vanished at sync time is worse than one that
cannot be typed into.

Two bugs the tests caught:

- **Escape committed.** Closing the editor unmounts its `<input>`, which fires `onBlur`, which
  commits. A cancel that silently saves is the worst failure this feature could have. Fixed with a
  ref checked before the commit runs, and pinned by a test.
- A **no-op edit staged a change.** Re-committing an unchanged value filled the review panel with
  rows that do nothing, which is how people stop reading it.

`useKeyboardShortcuts` now yields to a focused `role="grid"` the same way it yields to an input —
otherwise the global arrow bindings fired alongside the cursor and moved twice. The consequence is
that `j`/`k` stop moving once the table has focus, which is deliberate: a grid where `J` cannot be
typed into a genre is not a spreadsheet. They still work everywhere else.

**Next:** inline per-row waveform previews are the last `missing` row in Library & browser. Epic 7
(streaming) still needs a scoping decision from the user.

## 2026-08-06 — Delete from disk, with guard rails

Delete-from-disk was declined three times during Epic 5 — Find Broken Tracks, Archive cleanup,
duplicate resolution — on the grounds that it is the only operation in the program with no undo.
The user asked for it explicitly, "with proper guardrails". This is that.

**It is a quarantine, not an `unlink`.** `crates/file-organizer::trash` moves files into a
timestamped batch under the app data dir and writes a plain-JSON `manifest.json` beside them with
each file's original absolute path — readable by a human with a text editor if this program is
ever uninstalled. `restore` puts a batch back; `purge` is a separate call, per batch, by name, and
is the only step that removes anything. There is no "skip the trash" and no "empty all".

**The guards are refusals, not warnings.** `plan` is pure — it takes a filesystem oracle — and
drops a candidate outright when the path is outside every configured music folder, is a symlink,
is not an ordinary file, is missing, is pointed at by another track, or is already quarantined.
Only "still in a playlist" is overridable, by one checkbox that **re-plans** rather than waving the
rule through per file.

**Off until the user says where their music is.** With no music folders configured, `plan` refuses
everything — fail-closed by construction, and the dialog says so rather than looking broken.
Settings → Deleted audio owns the list; `suggest_music_roots` reads the directories the library
already draws from so filling it is one click.

Three things the tests caught that the design did not:

- `purge` accepted `..`. `PathBuf::join` does not normalise, so `starts_with(quarantine_root)`
  passed for a path that escaped it. Now it requires the batch id to be exactly one `Normal`
  component.
- Cross-filesystem moves need copy-then-remove, and the copy has to be **verified before** the
  source goes. A failed verification now leaves both copies: a duplicate is cleanable, a lost file
  is not.
- Two tracks with the same basename would have overwritten each other *inside* the quarantine —
  destroying the very file being preserved. `free_name` suffixes.

Also: `PARITY.md`'s summary table had drifted from its own body (it claimed 48/31/13/16 against
rows that actually said 46/27/20/16, and had no column for the two `blocked` rows). Recounted, and
a "how to read these numbers" note added — the counts are self-reported against a matrix written
from the manual, `lexicondj.com/features` is still 403 from this environment, and nothing has been
checked against a running Lexicon or a real library.

Reachable from all four places the spec puts it: Incoming triage, Archive, Find Broken Tracks and
duplicate resolution.

**Next:** Epic 7 (streaming) still needs a scoping decision from the user — it is all network and
third-party accounts against the local-first, no-telemetry constraint, and this container's proxy
403s outbound calls anyway. Unblocked frontend work remains in the browser (spreadsheet keyboard
navigation, inline waveform preview) and Custom Tags (category colours, drag-reorder, MyTag
import, per-tag hotkeys).

## 2026-08-06 — Custom Tags selection semantics

The Custom Tags page handed the library filter a flat list of tag ids with "match any". The spec's
semantics for that page are **OR within a category, AND across categories** — picking House and
Techno from Genre plus Peak from Mood means `(House OR Techno) AND Peak`, and a flat list with one
combinator cannot say that.

`Filters` gains `tagGroups: string[][]`, and the page groups its selection by the category each tag
came from. Details:

- **Groups take precedence over the flat list** when both are set — the grouped form is the more
  specific statement.
- **An empty group is not a constraint.** A category with nothing selected must not exclude
  everything.
- **The rule is on screen**, next to the selection count, rather than being a hidden behaviour the
  user has to infer from results.

This is the other half of the Epic 1 deferral I closed earlier today: the tag query language went
into the browser search box, and this is the Tags page.

**CI note:** the Actions queue is draining slowly — `e8a6120` took ~35 minutes to start. Several
commits still have no scheduled runs. Local verification is complete and green.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 602, typecheck, lint, `pnpm e2e` 46 — all green locally.

## 2026-08-06 — M3U import and new-playlist-from-selection

Two playlist-tree gaps, both reusing primitives that were already there.

**M3U import** — Playlist Tools → Import M3U. `crates/share` writes M3Us, so it now reads them:
one module owns the format in both directions, and a round-trip test holds them together. Paths are
matched against the library, filename as a fallback — the same priority as the history re-match, and
for the same reason: an M3U written on another machine has paths that will not resolve here, but the
filenames usually still do. **An ambiguous filename is no match**, because putting an arbitrary one
of two same-named tracks into a set is worse than saying it was not found. Unmatched lines are
listed by their `#EXTINF` label, which is the only identifier left for them.

Parser details that each cost a real bug elsewhere: a **UTF-8 BOM** is stripped (Windows tools write
them, and it otherwise makes line one unmatchable for a reason nobody can see); the `#EXTINF` label
is everything after the *first* comma, since titles contain commas; an orphaned `#EXTINF` does not
leak onto the next entry; and **relative paths come back as written**, because resolving them needs
the file's own location, which the caller has and the parser does not.

**New playlist from selection** — right-click a track. The right-clicked track joins the selection
if it was not in it, so the action never quietly excludes the row you actually clicked. It reuses
`apply_playlist_merge`, which was already the create-a-playlist-and-fill-it primitive.

`App` now depends on `DialogHost` and `ToastProvider` — it always had them in `main.tsx`, and its
test was rendering it bare. Wrapped, the same way `SettingsPanel`'s tests were when Backup
introduced a dialog.

**CI caveat:** GitHub has not scheduled check runs for the last several commits — `e8a6120` sat
queued for half an hour and the pushes after it produced no runs at all. The workflow is unchanged
and the previous run on it (`12988f5`) passed, so this is the Actions queue rather than the code.
Local verification is complete and green; that is not the same as a green check.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 594, typecheck, lint, `pnpm e2e` 46 — all green locally.

## 2026-08-06 — browser search shares the rule engine

The track-browser search box was a plain substring match across six fields. Lexicon's accepts the
same vocabulary its smartlists do, and `decks` had that vocabulary — in the rule engine, unreachable
from the box. This connects them.

**Parsed to rules, evaluated by the same evaluator.** `crates/smartlists::search` turns a query into
`Clause`s; nothing in it matches anything. The evaluator already knows what `bpm > 128` means, what
makes `4A` equal `Abm`, and how tags compare — a second implementation in the search box is exactly
how the two drift apart. This is the same reasoning as serving basic mode from Rust in the Mixable
Tracks slice.

| Input | Meaning |
|---|---|
| `deadmau5` | any text field contains it |
| `artist:deadmau5` | that field |
| `bpm>128`, `bpm<=120` | numeric comparison |
| `bpm:120-130` | inclusive range |
| `key:4A` | notation-aware — finds `Abm` |
| `genre:None` | the field is empty |
| `!remix` | negated |
| `~peak,vocal` | has **all** those tags |
| `tag:peak,vocal` | has **any** |

Decisions, each tested:

- **Plain text never leaves the renderer.** Only a query with syntax goes to the engine, so typing a
  band's name stays instant and works with no round-trip. The renderer asks the parser whether a
  query counts as syntax rather than carrying its own idea of it.
- **A negated comparison is the opposite comparison.** `!bpm>128` parses to `<=`, which keeps the
  rule model free of a "not" wrapper it does not have.
- **An unknown field is dropped, not guessed.** `remixer:` is a real Lexicon field we do not model;
  guessing which one they meant is worse than ignoring the term.
- **A non-numeric value on a numeric field matches nothing, not everything.** `bpm:fast` finds
  nothing rather than being silently discarded and widening the search.
- **A title with a dash is not a range.** `title:jump-off` is text.
- **A failed operator search says so.** Falling back silently would look like the query simply
  matched fewer tracks.
- Terms are ANDed and a bare word ORs across text fields — each word you add narrows, which is what
  a search box is for.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 594, typecheck, lint, `pnpm e2e` 46 — all green.

## 2026-08-06 — compatible-key indicator (Epic 2 loose end)

The spec describes the Key Mixing Mode as one global setting shared between Mixable Tracks and
**the track browser's compatible-key indicator**. Epic 6 built the setting and the first half; this
is the second. Select a track and the keys that mix out of it get a dot.

Two decisions:

- **A positive mark only.** An unmarked row means "not compatible *or* we cannot tell", and those
  are not worth distinguishing at a glance — marking every non-match would drown the ones that are.
- **No reference key means no marks**, not marks on everything. Better no indicator than one that
  highlights the library.

This also gives `mixable::key_compatibility` a caller. I wrote it during the Mixable Tracks slice,
noticed nothing called it, and deleted it rather than ship an unreachable command — the same
"stranded capability" problem the epic opened by fixing. It comes back now because there is
something to reach it.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 591, typecheck, lint, `pnpm e2e` 46 — all green.

## 2026-08-06 — Epic 6 (part 9): sidepanel — and Epic 6 closes

A **second track browser** on the right, resizable, toggled from the header or by `Cmd/Ctrl+\`.
Registered in the action registry rather than hard-wired, so it is rebindable like everything else
(Epic 2's design earning its keep).

**It keeps its own selection, deliberately.** The point of the spec's feature is comparing two
playlists while building a set; a shared selection would make it a mirror rather than a second
view. It is also available from every workspace view, not just the playlist browser — the reason to
open it is usually something you are looking at in the main pane.

That closes **Epic 6 — Set preparation**. Nine slices:

1. Mixable Tracks — reached `score_transition`, which had been unreachable since before the epic
2. Playlist tools — Merge, Sort, Cross Reference, Prefix, Rewrite Order
3. Playlist Occurrence — any N, with its distribution
4. Share / export — CSV, M3U, HTML, quick copy
5. Key leading-zero option, and Colors-to-nearest recorded as blocked
6. Favourite playlists with per-favourite hotkeys
7. Play history — snapshots, deleted-set ledger, save-as-playlist
8. Track Timeline
9. Sidepanel

Parity moved **34 done / 37 partial / 32 missing** at the start of the initiative to
**44 / 34 / 14 / 16 deferred**.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 589, typecheck, lint, `pnpm e2e` 46 — all green.

## 2026-08-06 — Epic 6 (part 8): track timeline

A chart above the playlist tracks — and above a history set, per the spec — showing how a set
flows: **BPM, Energy, Rating or Key**, with bars coloured by key or by **BPM change**. The
BPM-change mode is the one the spec is right about: green rose, red fell, grey held, and you read
the arc of a set in a second.

Decisions, each tested:

- **Heights scale within the set**, not against an absolute range. A warm-up running 118–124 should
  show its shape, not six flat bars near the bottom of a 60–200 axis. An all-identical set gets
  full-height bars rather than a divide-by-zero.
- **A missing tempo is `unknown`, not `same`.** "Unchanged" is a claim about two numbers; painting
  an absence grey would read as information the chart does not have.
- **Differences a DJ would not hear are `same`.** Rounded to a tenth, so 128.00 → 128.04 is not a
  red bar. A colour change for 0.04 BPM is noise dressed as signal.
- **The hover label carries the value and the direction**, so colour is never the only way to read
  the chart — and a track with no value says *which* value is missing rather than leaving a gap.
- **Key compatibility is `null`, not `false`, when a key is unreadable.** "These do not mix" and
  "we cannot tell" are different claims.
- **Hidden by default past 200 tracks**, per the spec: it is a set-building tool, not a collection
  tool. Still available on request.

**Not offered:** Danceability, Popularity, Happiness — `Track` does not carry them (Epic 4).

Also fixes a **CI-only test failure** on `7029a8d`: three `PlaylistToolsView` tests waited on
`findByTestId("playlist-picker")`, but that `<ul>` renders *before* `listPlaylists` resolves. Local
runs won the race; CI did not. They now wait on a playlist row, which only the loaded list can
produce. This is the same lesson already in the journal from `FieldMappingsSection` — wait on
something only the thing you are asserting about renders — and it cost a red check to relearn.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 589, typecheck, lint, `pnpm e2e` 43 — all green.

## 2026-08-06 — Epic 6 (part 7): play history

The gig log. **History** in the sidebar imports every session Rekordbox has logged, from
`djmdHistory` / `djmdSongHistory` into our own snapshot tables (cache migration **v17**).

**The snapshot rule is the whole design.** History is a historical record, not a view over current
data: the track data is copied in at import time and never re-joined to the library, so editing a
track later does not rewrite what the log says you played — and a set survives its tracks being
deleted from the library entirely. The view says so in as many words, because otherwise a row
differing from the library looks like a bug.

Decisions, each tested:

- **Import is idempotent** by `djmdHistory.ID`. Sets are never duplicated, and the report says how
  many were already known.
- **The deleted-set ledger remembers the source id**, so a re-import does not resurrect the
  practice sessions and false starts you cleared out. The report counts those separately — "why is
  my deleted set not back?" should never be a mystery.
- **Rekordbox's own tombstone is honoured.** A set deleted in Rekordbox is not imported at all.
- **Save as playlist re-matches id → path → filename, and names which rule hit.** "We found
  something with the same filename" is a materially weaker claim than "this is the same track", and
  the user sees the difference before anything is staged (ADR-0008).
- **An ambiguous filename is no match rather than a guess.** Two library tracks called `a.mp3` and
  the row comes back unmatched — picking one would silently put the wrong track in the set.
- **Removing a track from a set does not renumber the rest.** The number is the position in the set
  as played; renumbering would make the log claim a different set happened.
- **The deletion confirmation says it sticks**, and that audio files and the library are untouched.

Nothing here writes to `master.db`; saving a set as a playlist stages changes like everything else.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 558, typecheck, lint, `pnpm e2e` 43 — all green.

## 2026-08-06 — Epic 6 (part 6): favourite playlists

The spec calls it a fast filing system, and that is exactly the shape: star up to nine playlists,
and each gets a hotkey. **`1`–`9` opens the playlist, `Shift+1`–`9` files the current selection
into it.** Star from **Playlist Tools → Favourites**; the bar pins above the track browser.

Decisions, each tested:

- **The hotkey is the position, and positions are stable.** Un-starring closes the gap rather than
  leaving a hole. A hole would either strand a key on nothing or renumber silently on the next
  read — and a key that quietly changes what it does between sessions is worse than one that does
  nothing at all.
- **Nine is the cap, and the refusal says why.** A tenth favourite would be one nobody could press.
- **A favourite whose playlist is gone is pruned on read** — from the table as well as the
  response, so the stored order and the shown order can never disagree.
- **`e.code`, not `e.key`.** With Shift held, `key` is `"!"`, not `"1"`. The test presses
  `Shift+1` specifically to catch that.
- **Digits are never stolen from a text field**, and modified chords are left to whatever owns
  them.
- **Filing reports what it skipped.** Tracks already in the playlist are not staged twice, and the
  toast says how many were already there rather than quietly doing less than asked.
- **The bar renders nothing when nothing is starred**, and survives a null list — it sits above the
  browser and must never take the view down with it.
- **Jumping to a favourite expands the folders on the way to it.** A playlist inside a collapsed
  folder would otherwise be selected invisibly and then reset by the panel's own fallback.

Cache migration **v16** (`favourite_playlists`), scoped by `library_path` — unlike the cleanup
locks and mixable templates, a playlist id only means anything inside the database it came from.

**Not done:** drag-and-drop onto a favourite. The hotkeys and the `+` button cover the same intent,
and the track table has no drag source yet.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 546, typecheck, lint, `pnpm e2e` 39 — all green.

## 2026-08-06 — Epic 6 (part 5): key leading-zero option

Small and entirely about one thing: sorted as text, an unpadded key column reads
`1A, 10A, 11A, 12A, 2A, …`. The wheel positions come out interleaved, which is exactly what a DJ
scanning that column on a CDJ cannot use. `01A` fixes it.

`SyncOptions.add_leading_zero`, off by default, with a Sync toggle whose label says *why* — an
option that reads as cosmetics gets left off.

Two decisions:

- **Applied after conversion and independently of it.** The sort problem is just as real in a
  library left in its original notation as in one converted to Open Key, so the option does not
  require `convert_keys`.
- **A value with no wheel position is returned unchanged.** `C minor` is not padded. Padding is not
  a licence to rewrite something we did not understand — and the operation is idempotent, which
  matters because Sync runs more than once and `001A` would be the tell that it is not.

**Colors → nearest is blocked, and now says so.** `SyncOptions.change_to_nearest_color` has been
accepted and ignored since it was added. The reason is real: `Track` has no colour field and no
change kind writes `ColorID`, so there is nothing to map to anything. It stays unexposed rather
than becoming a switch that does nothing — recorded in `PARITY.md` and `01-interop.md` with the
blocker named, per ADR-0008.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 530, typecheck, lint, `pnpm e2e` 38 — all green.

## 2026-08-06 — Epic 6 (part 4): share / export

`crates/share` renders five outputs — quick copy, quick copy numbered, CSV, M3U and
printer-friendly HTML — reachable from **Playlist Tools → Share**.

The spec draws a line and so does the UI: **sharing produces a file, syncing updates Rekordbox.**
Nothing here stages a change or touches `master.db`.

Rendering lives in Rust rather than the renderer, so the CLI and MCP server can reach the same
export and CSV escaping has exactly one implementation.

Decisions, each tested:

- **CSV formula injection is defused.** A field starting `=`, `+`, `-` or `@` is quoted *and*
  prefixed with `'`. Comments are free text a DJ pasted from somewhere, and a comment reading
  `=cmd|...` is a live payload the moment the export is opened in Excel. The prefix is visible
  deliberately — silently mangling the value would be worse.
- **M3U says what it could not carry.** An M3U is a list of paths; a track without one cannot be in
  it. Handing back a quietly short playlist is how a set goes missing on the night, so the pathless
  titles come back with the export and the UI names them.
- **HTML is self-contained** — inline CSS, no external references, no scripts — so it works off a
  USB stick with no network. PDF is the browser's Save to PDF over it, which is how Lexicon does it
  too. A PDF writer here would be a large dependency reimplementing a print dialog.
- **A playlist name cannot become a path.** `Friday 8/6` exports as `Friday 8-6.csv`, and a name
  that sanitises to nothing falls back to `playlist` rather than `.` or `""`.
- **The default CSV columns are title / artist / BPM / key / duration** — exactly what the
  user-level `dj-setlist-builder` skill reads, so an export drops straight into that tooling.
- **A missing artist does not leave a dangling dash.** `- Title` reads as a field the reader is
  meant to notice; a bare title reads as a bootleg, which is what it is.
- **Hours are spelled out**, so a 90-minute live set does not read as `30:00`.

Not done: dragging header columns to reorder them. The picker orders by the order columns were
ticked, which covers the same intent for a list being built rather than rearranged.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 529, typecheck, lint, `pnpm e2e` 38 — all green.

## 2026-08-06 — Epic 6 (part 3): playlist occurrence

"Which tracks appear in exactly N playlists?" — **Playlist Tools → Occurrence**. `decks` had the
N=0 case only, through the `not-in-any-playlist` filter. This is any N.

Counted with `COUNT(DISTINCT PlaylistID)`. Rekordbox allows the same track twice in one playlist,
and "appears in two playlists" must not be satisfied by appearing twice in one — the distinct is
the whole correctness of the feature, and it has a test that adds a duplicate row specifically to
catch its absence.

**A track in no playlist is absent from the query, not zero in it.** The `GROUP BY` cannot see it.
The zero has to come from the library side, which is why the command walks the track list rather
than the count map — the N=0 case is exactly the one people ask for most, and it is the one a naive
implementation silently returns nothing for.

**Addition beyond the spec:** the report ships the whole distribution — how many tracks sit in 0,
1, 2 … playlists — and each row re-runs the report for that N. A bare "how many playlists?" box
asks the user to guess a number they have no way to know; the distribution answers it first.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 519, typecheck, lint, `pnpm e2e` 36 — all green.

## 2026-08-06 — Epic 6 (part 2): playlist tools

All five, in a **Playlist Tools** view: Merge, Sort, Cross Reference, Prefix, Rewrite Order. Every
one previews first, and everything they do is a staged change that goes through review and Sync.

**Rewrite Order is the one that earns the epic.** It has no visible effect inside `decks`, exactly
as it has none inside Lexicon — its entire purpose is that a CDJ can only sort by a handful of
columns and knows nothing about Energy. Sort by Energy here, rewrite the order, and the playlist
arrives on the gear that way.

Decisions, each tested:

- **Merge creates; it does not consume.** The sources are left alone. A tool that quietly deleted
  four playlists to make a fifth would be a different and much worse tool. The preview says
  "3 tracks from 5 rows — 2 duplicates dropped", because the final count alone hides the work.
- **Sort needed a new change kind.** `ChangeKind::PlaylistReorder` writes `djmdPlaylist.Seq`. The
  parent folder is part of the applier's `WHERE`, so naming a playlist that lives elsewhere fails
  loudly rather than silently reparenting it — a reorder should only ever reorder.
- **Cross Reference over an empty selection returns nothing, not everything.** "In all zero
  playlists" is vacuously the whole library: technically right, and a terrible thing to hand
  someone who has selected nothing. The `in none` mode warns before it runs, per the spec.
- **Prefix numbers in tick order**, which is why the selection is a list and not a `Set`. `Replace
  an existing number` stops prefixes stacking on a second run, and a number that is *part* of the
  name — `7empest`, `2 Bad Mice` — is not stripped. The signal is the separator: digits running
  straight into a letter stay.
- **Rewrite Order appends rather than drops.** If a filter was active, the sorted view holds fewer
  rows than the playlist; storing only those would silently remove the rest. They go to the end,
  and the UI says how many.
- **A no-op stages nothing.** An order that already matches, or a rename set that is already
  right, produces zero changes rather than rows in the review list that change nothing.

**Divergence, stated rather than hidden:** Lexicon persists "the current visible sort" of the
browser. `decks` sorts inside the tool, on a field you pick. The browser's column sort is transient
UI state not plumbed to this view, and a button whose result depends on which column you last
clicked somewhere else is worse than one that states its input.

One real bug caught by its own test: the Rewrite Order sort negated the whole comparator for
descending, which flipped the null handling too — an un-analysed track led the set purely because
`null` compares low. Direction now lives inside the comparator, so tracks with no value sort last
in **either** direction.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 516, typecheck, lint, `pnpm e2e` 35 — all green.

## 2026-08-06 — Epic 6 (part 1): Mixable Tracks

`scoring::score_transition` has existed since long before the parity initiative with **no UI
caller** — the spec called it "partial, and stranded". This reaches it. **Right-click a track →
Find mixable tracks**, or the `Mixable` toggle in the header; the panel opens as a right-hand
inspector and stays open, because the spec's workflow is driving it live through a set.

**The rules filter; the score orders.** A candidate that fails an enabled rule is not in the list
at all. That is why the header says "12 of 4,213" rather than just listing twelve — a rule that
merely demoted would make "must have cue points" a suggestion, and the count is what tells you the
rules are too tight.

Nine of the spec's thirteen advanced options are here: BPM range, Match key, half/double BPM,
must-have-cues, genres, year, energy, rating, and the tag include/exclude lists. **Four are
deliberately absent** — `Match color` and `Recently added` need colour and date-added columns
`Track` does not carry, and Popularity / Danceability / Happiness are the Lexicon-only fields from
Epic 4. The panel says so in as many words rather than showing controls that match everything.

Decisions, each tested:

- **`Use as next track`** re-seeds the panel from the row just picked. It is the difference between
  a report and a tool you can play a set with.
- **Key Mixing Mode is global**, per the spec, and the backend overwrites whatever a template
  carried with the stored value — otherwise loading an old template would silently change a setting
  the user made in the panel.
- **Basic mode is served by the backend**, not hardcoded in the renderer. A second definition of
  "basic mode" in TypeScript would drift the first time a default changed and nothing would fail.
  The panel simply does not search until it has the rules.
- **Templates are keyed by name and overwrite**, because the workflow is "tweak, save as *Peak
  time* again". Not scoped by library: it is a statement about how someone mixes.
- **Ties break by track id**, so two identical searches give the same order. A list that reshuffles
  between previews is not usable live.
- **Half/double tolerance is a percentage of the *stretched* tempo**, so a ±3% double-time search
  round 140 accepts 286 and refuses 290, rather than applying 3% of 140 to a 280 target.
- **An unparseable key is never compatible.** Treating unknown as a wildcard floods the list with
  exactly the tracks nobody has analysed yet.
- **Archived tracks are never suggested.** Archiving says "out of rotation", and "what do I play
  next" is where that should be honoured.

Two latent bugs fixed on the way. `score_transition` carried its own Camelot-only parser, so every
spelled-out key (`C minor`, `Cm`) scored as **"Missing Key Data"** — it now routes through
`changes::key_format`, the one place that knows the 24-key table. And `key_format` itself could not
read **Open Key** input (`10m`, `8d`), which is what a library edited in Lexicon stores; it can now,
without stealing `Dm` from the musical-key parser.

Cache migration **v15** (`mixable_templates`). Agent tool `mixable_tracks`, so the chat panel, MCP
server and CLI gain it too — its `bpm_tolerance_pct` keeps "omitted" and "0" apart, since one means
"use the default 6%" and the other means "ignore tempo".

Still missing from Epic 6: the Track Timeline, playlist tools (Merge / Sort / Cross Reference /
Prefix / Rewrite Order), Playlist Occurrence, favourite playlists, the sidepanel, History
snapshots, and share/export.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 502, typecheck, lint, `pnpm e2e` 30 — all green.

## 2026-08-06 — Epic 5 (part 10): prefix rewriting

The fuzzy relocate answers "where did this one file go?". `relocate::rewrite` answers a different
question: "the drive letter changed, rewrite all four thousand of them." **Files → Rewrite Paths.**

**Nothing is inferred.** The user states both prefixes. The spec calls this the deterministic path,
and a tool that guessed the rewrite would eventually guess wrong across an entire library — so
there is no "detect" button, deliberately.

Decisions, each tested:

- **Separators and case are ignored when matching** — a user typing `D:\Music` means the folder
  stored as `D:/music/`. But **the remainder keeps its original case**: a rewrite that lower-cased
  the rest of the path would break every file on a case-sensitive filesystem. That pair is the
  whole subtlety of prefix matching.
- **All-tracks mode is off by default** and warns when switched on. The common case is repairing
  breakage; sweeping working paths into a rewrite is how a working library stops working.
- **A path already in the library is refused**, per the spec's constraint — and so is a collision
  between two rewrites in the *same plan*, which does not have to pre-exist to be a collision.
- **An empty source prefix rewrites nothing**, since it would match every path in the library.
- **Extension substitution** swaps only the last dot *after* the last separator, so
  `/Music/v1.0/track` gains an extension rather than having `0` replaced.
- The preview says **"1 of 3 would be rewritten"**, and lists collisions but not the thousands of
  paths that simply did not match — that would be noise, not information.

Missing-ness is judged **through local path mappings**, so a library opened on a second machine is
not reported as entirely missing before the user has typed anything.

Rewrites stage as `TrackRelocate`. The spec's "optional automatic backup before rewriting
locations" is already there and not optional: `WriteGuard` takes one before Sync's first write.

Still missing: the **single-track merge-with-existing** case and the **5-minute re-check cadence**.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 489, typecheck, lint, `pnpm e2e` 26 — all green.

## 2026-08-06 — Epic 5 (part 9): duplicate resolution

Detection already existed. What did not was the part that makes it safe to use.

**Playlist re-pointing** was the flagged gap and is closed. Resolving a group stages a
`PlaylistAddTrack` for the keeper and a `PlaylistRemoveTrack` for each loser, in every playlist that
held one — archiving a losing copy without that leaves a hole in every set it was in, and the user
finds out on stage. The keeper is added **before** the loser is removed: both are staged, so the
order only matters at apply time, and that is exactly when a removal-first ordering would leave the
set briefly short. A playlist already holding the keeper gets the loser removed rather than the
keeper added twice.

**The keeper heuristic puts cues above bitrate**, and that is the single most important line in it.
Losing someone's cue work is the expensive mistake; losing 64kbps is not. Ties fall through bitrate
→ playlist membership → play count, and a genuine tie resolves the same way every run — otherwise a
bulk `Prefer` over 200 groups would give a different answer each time it was previewed. An explicit
rule like `Highest bitrate` overrides the default, which is the whole point of `Prefer`: "I know,
do it my way anyway."

The heuristic lives in Rust as a pure function rather than in a click handler, so it is inspectable
and testable — and reusable if the chat panel ever wants it.

**Duration bounds** are the spec's 15 seconds to 15 minutes, and the interesting case is the third
one: a track with **no recorded duration is included**. An unknown length is not evidence of a long
one, and excluding it would silently drop everything that has never been analysed.

**The review step** names the playlists that will be re-pointed before anything happens, and says
so plainly when none will be. Losers are archived, never deleted, and the confirmation says it.

Still missing: **interruptible scans** and **manual merge**. The first needs cancellation plumbing
through a long-running command; the manual's advice to work in passes is not possible without it.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 479, typecheck, lint, `pnpm e2e` 26 — all green.

## 2026-08-06 — Epic 5 (part 8): Genre / Artist Cleanup

**Locking** is the interesting one, and it turns on where it is *not* scoped. By kind
(`genre` | `artist`), because the same string can be a good genre and a misspelt artist. **Not** by
library — a value the user has declared canonical is canonical for them, and re-locking the same
fifty genres for every library would defeat the point of locking. Cache migration **v14**.

The half that matters is `Cmd/Ctrl+A` selecting everything **unlocked**. Select-all is precisely
the gesture most likely to sweep a good value into a rename, so a lock that did not protect against
it would be decoration. Locking something already selected also deselects it — otherwise it sits
there selected and unselectable, which reads as the lock not working.

**Pinned letters** persist the same way. The letter bar only offers letters actually present, so it
never advertises a jump to nothing; non-alphabetic values group under `#`.

**Alt/Option-click filters the browser**, and the two modes differ honestly: genre has a real
filter dimension, artist does not, so it goes through the search box — which searches artist among
other fields, making the result a superset rather than an exact match. Named in the code and the
spec rather than passed off as a filter.

**Sort by track count (default) or name.** Count sorts descending with name as the tie-break, or
equal counts shuffle between loads for no reason.

**Not done: the extra artist fields** (`Remixer`, `Producer`, `Composer`, `Lyricist`). `Track` does
not model them and the core `SELECT` does not read them — the same gap that puts `label`, `mix` and
`colour` out of scope. They belong with whichever epic widens the track model, not bolted on here.

One thing worth recording about the diff: I rewrote three dialog strings that were not mine to
change and broke four existing tests doing it. The tests were right; the copy was restored.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 475, typecheck, lint, `pnpm e2e` 26 — all green.

## 2026-08-06 — Epic 5 (part 7): Database Backup

`WriteGuard` protects `master.db`. Nothing protected everything `decks` knows that Rekordbox does
not — custom tags, the archive, smartlists, staged changes, path mappings, watch folders,
conversations. All of it lived in one local cache file with no way to move it to another machine or
recover it after a mistake. **Settings → Database Backup** closes that.

**A JSON document rather than a ZIP**, which is the main divergence. Lexicon ZIPs because it
bundles several files; this is one document, and compressing a few hundred kilobytes of text buys
nothing a user can feel. What it buys instead is worth more: the backup is *inspectable*, and it
survives schema changes. Restoring a copied SQLite file into a newer schema is a gamble; restoring
named columns is not — unknown columns are dropped and **named in the report**, and a table missing
from the backup is left alone rather than emptied, so an old backup cannot silently wipe a feature
it never knew about.

Other decisions:

- **Analysis caches are excluded.** Waveform peaks, fingerprints and audio features derive from
  files still on disk; including them would multiply the backup's size to save CPU the user spends
  once.
- **Nothing is auto-deleted.** Lexicon removes its own backups after a month. A tool that deletes
  the user's backups on a timer is doing something they did not ask for, and the retention note
  says so plainly.
- **A backup from a newer build is refused**, not partially applied.
- **Restore replaces**, in a single transaction — a failure part-way leaves the cache as it was
  rather than half-swapped, which would be the worst of both.
- **The file is inspected first and its contents shown in the confirmation**, so the user sees what
  they are swapping *in* rather than only what they are losing. A file that is not a backup is
  caught on read, before anything is deleted.

Table names reach a `format!` string on restore, so they are checked against a fixed allowlist
first — with a test that feeds in `"tags; DROP TABLE tags"` and asserts the real table survives.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 467, typecheck, lint, `pnpm e2e` 26 — all green.

## 2026-08-06 — Epic 5 (part 6): Archive — the playlist rule and the selection helper

**The context-sensitive playlist rule** is the interesting half. Archiving *from inside a playlist*
removes the track from **that** playlist and leaves every other alone; archiving *from the browser*
touches no playlist at all. The asymmetry is the whole point — from a playlist you are saying "not
in this set", from the browser you are saying "not in my way" — and it is implemented where users
actually archive, on the track context menu, whose label changes to say which one you are about to
get.

Archiving is cache-only and takes effect at once; the playlist removal is a staged change and goes
through review and Sync. The two halves are reported separately because they land at different
times, and a toast claiming both had happened would be a lie about one of them.

**The selection helper** offers the spec's three criteria over the archive. Two details worth
keeping: "older than 0 days" does *not* sweep up something archived a second ago — almost certainly
a misclick on the way to picking a real threshold — and a criterion matching nothing says so rather
than silently clearing the selection, which would look identical to it having worked.

**Cleanup is where tracks finally leave every playlist**, not archiving. That is exactly what makes
archiving safe to do on a whim. It stages the playlist removals *before* the track deletes, so the
playlist rows are never left pointing at a track that no longer exists.

**Delete-from-disk is deliberately not implemented**, and this is the second time in this epic that
decision has come up (Find Broken Tracks was the first). It is the one operation with no undo, and
a program whose first rule is that `master.db` is read-only should not be the thing that deletes a
DJ's audio. The confirmation dialog says "Audio files on disk are never touched" in as many words,
and there is a test asserting that sentence is there. If it is wanted it deserves its own decision
and its own guard rails, not a quiet ride-along on a cleanup button.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 458, typecheck, lint, `pnpm e2e` 26 — all green.

## 2026-08-06 — Epic 5 (part 5): Find Broken Tracks

The existing broken-link scan asks whether a path exists. That misses the failure DJs actually
meet: the file is right there, and the deck plays two seconds of it and stops. A truncated
download, a half-copied file, a `.mp3` that is really an HTML error page — all present, all
unplayable.

`audio_analysis::playable` decodes instead of stat-ing. Reachable from **Audit → Find Broken
Tracks**, and exposed as `health_playable_scan` through `crates/agent-tools`.

**Two depths, because the honest ones cost different amounts**, and the UI names the trade rather
than picking silently. Header probes the container and builds a decoder — fast, catches
wrong-format and unsupported files, misses anything that goes wrong late. Full decodes every packet
and discards the audio — catches truncation, costs about what analysing the track costs.

**Truncation needed a second signal.** Raw PCM has no framing to fail on, so a half-downloaded WAV
decodes perfectly and simply ends early; the first version of this passed it. The check now
compares frames decoded against the frame count the header *declares*, with a 1% tolerance for
encoder padding — which as a bonus makes truncation detectable in formats where the stream itself
would not complain either.

Outcomes are named rather than boolean: `Missing`, `Unreadable`, `Undecodable`, `Truncated`,
`Damaged { bad_packets }`. Deleting a file that is absent is a different fix from replacing one
that is corrupt, and a track that plays with glitches is a third thing again.

Two deliberate divergences from the spec:

- **Nothing is deleted.** Lexicon optionally removes broken files from the library, from playlists
  and from disk. `decks` reports; removing a track is a staged change Sync applies under the write
  guard. Deleting audio from disk is not offered at all — it is the one operation with no undo,
  and this program's whole posture is that the user reviews first.
- **The report saves where the user chooses**, not to `Documents/Lexicon`. Each entry still names
  which playlists held the track, which is the entire reason the report exists.

Paths resolve through local path mappings first, so a library restored on a second machine is not
reported as four thousand missing files.

The WAV builder in the tests is worth noting: `fixtures/audio/` is gitignored by design, so a
decode check that only ran against real files could not be tested at all. A hand-built 44-byte
header plus PCM gives a genuine pass, a genuine empty-audio case, and a genuine truncation.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 454, typecheck, lint, `pnpm e2e` 26 — all green.

## 2026-08-06 — Epic 5 (part 4): the manual multi-track editor

`E` over a selection opens a field editor; `changes::multi_edit` collapses the fields and plans the
writes.

**The whole feature turns on one rule: a field the user did not touch is not written.** Open the
editor on forty tracks, change the genre, press Save — and the other nine fields must come out
exactly as they went in, even though the form had to show *something* in each of them. Get that
wrong and the editor silently flattens a library to whatever the first track happened to hold.

So the form's state is not "the values", it is "the values plus which ones were edited".
`FieldValue::Multiple` is what a field shows when the selection disagrees, and it is a value the
caller can never accidentally write, because **it is not a value at all** — in the UI it is a
placeholder, not text. The apply command takes only the *edited* fields, never the whole form:
sending the form back would mean `<multiple values>` had to be represented somehow, and any
representation of it is one save away from flattening the selection.

Other decisions, each tested:

- **A missing value and an empty string are the same field state.** A form cannot tell them apart,
  and "clear this field" must not behave differently depending on how the field became empty.
- **One track missing the field is a disagreement**, not agreement on what the others hold —
  otherwise the editor shows "House" while half the selection is empty, and Save is
  indistinguishable from doing nothing.
- **A track already holding the value produces no change.** Most of a large selection is usually
  already right; staging forty no-ops would bury the two that matter.
- **Clearing a field is a real edit**, distinct from not touching it. Whitespace is a value the
  user typed; only empty clears.

`Enter` saves, `Esc` and Cancel discard. The selection is frozen when the editor opens — editing
forty tracks while the table's selection changes underneath would save to the wrong set.

Deliberately not done: `←`/`→` auto-saving navigation between tracks, `Tab` to the Recipes page,
and album art (replace / remove / Reload). `decks` has no album art anywhere, which makes it a
separate feature rather than part of this one.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 443, typecheck, lint, `pnpm e2e` 26 — all green.

## 2026-08-06 — Epic 5 (part 3): CSV import and the common-text blocklist

**Import Tags From CSV** lands in `crates/track-matcher::csv_import`, alongside the existing
`csv_input`. The two are close cousins with opposite jobs: `csv_input` parses a CSV to *find*
tracks, this one parses a CSV to *write fields onto* them, and the match is only how it decides
which track the values belong to.

Rows match on a Location column or on Artist + Title together. Decisions, each tested:

- **A mapping with no matching strategy is refused** rather than run. It would match nothing, and
  "0 rows matched" is indistinguishable from a broken file.
- **Location wins when both are configured.** A path is an identity; a name is a description two
  mixes can share.
- **Path comparison ignores separators and case** — a CSV exported on Windows and a library indexed
  on macOS describe the same file, and refusing to match them would kill the strategy in exactly
  the case it exists for.
- **An empty cell leaves the field alone.** Spreadsheets are full of blanks; treating them as
  deletions would wipe metadata on every partial import.
- **A column the file does not have is an error**, not a blank column — a mistake the user can fix,
  where importing blanks over good metadata is not recoverable.
- **Two tracks matching one row is `Ambiguous`, not arbitrary.** Rows that matched nothing or
  matched several are shown with their reason, not dropped.

**The Excel caveat that actually bites:** "CSV UTF-8" export writes a byte-order mark, and it lands
*inside the first header name* — so a mapping naming the first column silently stops matching, and
the failure reads as a typo in the user's own mapping. Stripped on both the header read and the
parse, with a test.

**The common-text blocklist has a UI** at Settings → Remove Common Text, closing a gap where the
IPC existed and nothing consumed it. Both presets the manual names — `(Original Mix)` and the 24
Camelot keys — are one-click buttons rather than seeded entries: a blocklist that arrives
pre-populated will eventually strip something the user wanted, and they will not know why.

**A real bug fixed on the way.** `remove_common_text` lower-cased the value, searched the copy, and
spliced the original using the *pattern's* byte length. Lower-casing can change a string's byte
length (`İ` → `i̇`), so that is a panic on a char boundary, not a wrong answer. It now goes through
`recipes::text::remove_text`, which already got this right — one correct implementation instead of
two, and `smart-fixes` gains a dependency on `recipes` to say so.

**`Blob.text()` needs Safari 14+,** and the desktop shell runs on WKWebView. `readTextFile` in
`lib/read-file.ts` falls back to `FileReader`; both the CSV import and the Track Matcher use it now.
jsdom is missing `Blob.text()` too, which is how it surfaced.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 430, typecheck, lint, `pnpm e2e` 24 — all green.

## 2026-08-06 — Epic 5 (part 2): Undo History

`decks` gates hard *before* a write — every change reviewed, Sync opt-in, `WriteGuard` taking a
timestamped backup first. It had no answer at all for the change you accept and then regret, and
restoring the backup is a sledgehammer that throws away the rest of the session with it.

**Every Sync run now records the inverse of each change it applied**, and undoing stages those
inverses as ordinary proposed changes. So an undo goes back through review and the same guarded
Sync: two steps rather than one, no second write path into `master.db`, and nothing reaches the
library without the user seeing it. That is the whole design decision — an undo that wrote directly
would have been one click, and would have broken the program's first rule.

Inverses are computed **at apply time**, not derived on demand: `staged_changes` rows get cleared,
and a run has to stay undoable after its originals are gone. Migration **v13** adds `undo_runs` and
`undo_entries`.

**Not every change can be inverted, and the UI says which.** Per ADR-0008 a blocked entry carries a
named reason rather than being quietly dropped, and a run shows its reversible/blocked split before
the user commits to anything. An undo that silently restored eight of twelve edits would be worse
than one that restored none.

- Metadata, cue, relocate edits, playlist rename and reorder invert by swapping the change's two ends.
- Playlist add ↔ remove is the same payload under the opposite verb; a re-added track lands at the end.
- Cue deletions invert **when the deletion recorded the cue** — the cue recipes now snapshot it into
  `old_value`, so the restored cue comes back at the same position, name and colour with a new row id.
- Adding a cue and creating a playlist are blocked: the new row's id is minted inside the apply
  transaction and nothing carries it back out.
- Deleting a playlist or a track is blocked, and points at the backup Sync already took.

**One distinction does real work throughout:** `old_value: Some(Null)` means the field was
genuinely empty and restoring it means clearing the field; `old_value: None` means nothing was
recorded and the change cannot be reversed at all. Collapsing the two would blank fields the user
never asked to blank. There is a test named after it.

**Retention diverges from the spec on purpose.** Lexicon drops undo history after 60 minutes or on
restart; `decks` keeps the last 50 runs per library. The cache is already persistent, and noticing
a bad sync the next morning is at least as common as noticing it within the hour — and "the last
fifty syncs" is something a user can reason about where "anything since 09:14" is not.

Reachable from **Changes → Undo History**, and exposed as `undo_list` / `undo_entries` /
`undo_run` through `crates/agent-tools`, so the chat panel, the MCP server and the CLI all gain it
from one implementation. The staging loop — including the once-only guard — lives in
`CacheDb::stage_undo_run` rather than in the Tauri command, so there is exactly one copy of it.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 411, typecheck, lint, `pnpm e2e` 23 — all green.

## 2026-08-06 — Epic 5 (part 1): the Recipes engine

New `crates/recipes` — the *other* bulk-editing system. `crates/smart-fixes` is ten fixed,
zero-parameter cleanups; a recipe takes parameters, and the user assembles the one they need. The
casing, field, text and number categories are done: **18 operations**, every one a pure function of
(recipe, fields) → fields, with no database, no filesystem and no notion of what a track is. That
keeps the whole vocabulary testable in isolation and lets one engine back the preview, the apply
pass and, later, an agent tool.

**The casing recipes exist because of one parameter.** `fix_casing` hardcodes its
article/preposition list; a recipe takes the user's own words-to-ignore. A library full of `EDM`,
`NYC` and `DJ` needs those protected, and no hardcoded list will ever contain them.

Three rules the spec leaves open, decided and tested:

- A recipe whose source is empty **reports why** — `SourceEmpty`, `NoMatch`, `NotANumber`,
  `Misconfigured`. "340 of 400 changed" needs an explanation attached; silence reads as a bug.
- `Merge Fields` with one half missing yields the other half, not a stray separator.
- `Extract Text` with no match leaves the target **untouched**. Writing an empty string would blank
  a good remixer field, which is worse than not running at all.

Smaller decisions worth recording: `AdjustNumber` keeps an integer looking like an integer (bumping
a track number from 3 must give `4`, not `4`); casing preserves original spacing rather than
round-tripping through `split_whitespace`; and `RemoveBetween` collapses the gap it leaves, since
`"Track  Live"` with a double space is exactly what a cleanup recipe should not produce.

The field vocabulary offered by the UI is deliberately the intersection of what `decks` models and
what the applier's allowlist will actually write — a test enforces it. Offering a field that cannot
be persisted would produce a preview full of changes that silently vanish at sync time.

`RecipesPanel` (sidebar → **Recipes**) builds an ordered list, previews every change as a
deselectable before/after row, and stages what survives. Recipes serialise, so one built today can
be saved and replayed on next month's downloads — the point of the feature.

**The tag recipes** ship alongside, in `crates/recipes::tags`. They are modelled as a *delta* to
the track's tag set rather than a new value — which is what the cache's add/remove accessors want,
and what lets a preview say "adds 3, removes 1" instead of showing two lists.

`Import Tags from Text` is the one the spec singles out, and it is idempotent in two senses: a tag
the track already has is not re-added, and nothing existing is ever removed, so a hand-added tag
survives a re-run. Matching is case-insensitive, because a library holding both `#techno` and
`#Techno` is exactly the mess the feature exists to clean up. A tag runs from the marker to the
next whitespace, matching how the convention is written (`#PeakTime`, not `#Peak time`).

Two more rules the spec leaves open: replacing a tag with one the track already has is a removal
*only*, or it ends up holding it twice; and replacing with an empty tag is refused rather than
silently becoming a delete.

Tag recipes apply directly rather than staging — tags live in the local cache, so there is no sync
step to carry them. A tag name with no existing tag is created in the first category, and the result
reports which were invented.

**The three "other" recipes** close the category. Each reaches into a different subsystem — Incoming
state, playlists, the filesystem — so they run one at a time rather than joining the ordered recipe
list, and the UI states what each does *before* it runs. `Remove from All Playlists` leaving
smartlists alone is exactly the sort of thing that otherwise reads as the recipe having missed some.

`Import Date from Filesystem` takes the file's **modification** time, not creation time: creation
time is not portable (Linux has no reliable `birthtime`), and a file copied between drives keeps its
mtime while its ctime becomes the copy date — worse than useless as a release year. `Mark as
Incoming` is the exact inverse of `Selected done`, clearing the per-track reviewed flag added in
migration v12.

**The cue recipes** land in `crates/recipes::cues` — nine of the spec's eleven. The spec calls this
category the most valuable and the furthest from anything `decks` had, and it needed a different
shape: one operation can delete, reorder, rename and recolour in a single pass, so the engine
returns a whole new cue list plus the ids it removed, rather than a per-field diff.

The beat grid is passed **in** rather than read, which keeps the category a pure function and means
`QuantizeCues` is testable without an ANLZ file on disk.

Decisions worth recording, all tested:

- **"First" and "last" mean first and last in the track.** `djmdCue` rows come back in insertion
  order; a user means the timeline. Every mode that says first/last sorts by position before
  picking.
- **`Sort Cues` reassigns hot-cue slots 1–8** in the new order, because `djmdCue` has no cue
  ordering of its own — the slot *is* the order. Memory cues have no slot and stay put; a ninth hot
  cue keeps the slot it had. A sort that changes nothing stages nothing.
- **Quantizing preserves loop length** rather than snapping both ends independently, which would
  stretch the loop. Shifting takes loops whole, for the same reason, and clamps at zero.
- **`QuantizeCues` on an unanalysed track says "this track has no beat grid"** instead of reporting
  no changes (ADR-0008). The UI lists the track with its reason and excludes it from the count.
- **"Random" is `Cycle` and is deterministic.** A preview showing different colours from the apply
  would be worse than no preview at all.
- **Colour edits stage `-1`, not null** — Rekordbox's spelling of "no colour" — and position edits
  stage a JSON *number*, so `json_to_sql` lands them as integers rather than text.

`Change Active Loops` and `Half/Double BPM` are deliberately out: the first needs a `djmdCue`
column `decks` does not model, the second has to move beatgrid markers, which is an ANLZ write and
belongs with the beatgrid recipes.

**Not done:** the beatgrid recipes (3), which all write a grid.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 401, typecheck, lint, `pnpm e2e` 22 — all green.

## Blockers — verified, not assumed (2026-08-06)

The three Epic 4 items still open cannot be built *in this environment*, for reasons that are about
the environment rather than the design:

| Item | Blocker | Evidence |
|---|---|---|
| Enrichment (Find Tags, album art) | Outbound egress to metadata APIs is blocked. | `curl https://musicbrainz.org/ws/2/...` returns `CONNECT tunnel failed, response 403`. Writing response parsers against invented JSON shapes would be untestable code in a production path. |
| Energy / Danceability | No audio to calibrate against. `fixtures/audio/` holds only `.gitkeep`; real fixtures are gitignored. | An uncalibrated 1–10 scale would be a number we could not defend, which is exactly what ADR-0008 exists to prevent. |
| Beatshift Fixer | Both of the above, plus a re-encoding dependency. | Detecting beatshift needs real encoder-padded MP3s; fixing it needs a re-encoder. |

None of these is deferred by preference. Each needs either network access or a real (gitignored)
fixture library, both of which are available on a developer machine and not here. The design work
for all three is recorded in `docs/lexicon/` and `docs/ROADMAP.md`.

## 2026-08-06 — Epic 4 (part 2): Field Mappings

`crates/changes::field_mappings` — the projection engine for fields the target does not have.
Energy, Danceability and Custom Tags have no Rekordbox column and no standard ID3 frame; a mapping
writes them somewhere that does. It lives in `changes` rather than beside Write Tags because the
same `Energy → Comment` rule must produce the same string whether it lands in `master.db` or in an
ID3 frame — one implementation, two call sites.

Spec semantics: source → target, overwrite replaces while off appends, several sources on one
target combine with `, `, custom tags write hashtag form, and a colour source writes the colour
*name* since a text target cannot use a hex value.

Three rules the spec leaves open, decided and tested:

- A track with no value for a source contributes **nothing** — not `Energy` with no number after
  it, and not a blanked target.
- Numbers are zero-padded to two digits, so a text target sorts them correctly. Same reason Key
  Conversion has a leading-zero option.
- Where several mappings share a target, the **first** decides overwrite-vs-append. Mixing the two
  on one target is a configuration mistake; first-wins is predictable and matches reading order.

**Cache migration v11 drops the dead `field_mappings` table from v5.** Nothing ever read or wrote
it, and its `(library_path, source_field)` primary key allowed exactly one target per source —
which cannot express combining, the feature's most useful half. The replacement is scoped by
*profile* rather than library path, because mappings are configured per destination.

Write Tags honours them, with two guards: mappings only fill targets the per-field selection did
**not** claim (quietly replacing a field the user explicitly ticked would be a nasty surprise), and
a mapping onto a field audio files do not have produces a warning rather than silently vanishing.

`FieldMappingsSection` in Settings configures the ID3 profile. Per-DJ-app profiles and applying
mappings during sync are outstanding; the schema is ready for both.

**Incoming `Selected done`** ships alongside (cache migration **v12**). The manual is right that
auto-advance is the detail that makes triage fast: marking the selection reviewed immediately
selects the next track, so an inbox clears with one repeated `D` instead of a click-and-reach cycle
per track. Two details that had to be got right — the next track is chosen from the list as it stood
*before* removal, so it is the one that visually followed what the user was looking at; and
advancing is skipped entirely when marking failed, because advancing past a track that is still in
the inbox would lose it.

The existing `incoming_watermark` could not express this: it answers "what arrived since I last
cleared", which is all-or-nothing. Per-track review state is a separate table, filtered out
alongside archived tracks.

**Send to → Move files…** is now on the track context menu, scoped to the current multi-selection
when the right-clicked track is part of it, so "send these twelve" works as well as "send this one".
It carries a `Moves on disk` hint — it is the only context-menu entry that touches the filesystem,
and that should not be a surprise.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 354, typecheck, lint, `pnpm e2e` 17 — all green.

## 2026-08-06 — Epic 4 (part 1): Move & Rename

New `crates/file-organizer`, three pure layers with no filesystem access anywhere in the crate:

**`pattern`** — the `%field%` template language. `%field%` interpolates, literal text passes
through, and `{ … }` marks an optional segment that is emitted only when *every* field inside it
has a value. That last construct is the whole point: `%artist% - %title% (%key%)` on a keyless
track leaves `Daft Punk - Get Lucky ()`, while `{(%key%)}` leaves nothing behind. Renders are
trimmed, because a dropped segment nearly always strands the space that separated it. Optional
segments deliberately do not nest — one level covers every documented use, and rejecting nesting
gives a clear error rather than a surprise. Every worked example from the manual is a test.

**`subfolder`** — up to three nested levels, each independently optional, plus the computed
patterns: bitrate as `320+`/`320-` buckets rather than raw numbers, first tag, current year, current
month zero-padded, current decade as a range. **An empty field drops its level, not the move** — a
track with no genre still lands in the target folder, one level shallower. Anything else orphans
files. A missing bitrate drops the level rather than defaulting into `320-` and mislabelling a
lossless file. Field values are sanitised per component, so `Drum & Bass / Jungle` is one folder
and cannot invent a second level.

**`plan`** — combines the two into destination paths, taking an existence oracle as an argument so
collisions, no-op moves and empty renders are all unit-testable. Collisions suffix ` (2)`, both
against the filesystem and against destinations claimed earlier in the same batch: two tracks can
legitimately render to the same name, and overwriting one with the other destroys audio. A render
containing nothing but punctuation falls back to the existing filename, so an untagged track never
becomes `-.mp3`.

**New `ChangeKind::TrackRelocate`.** Moving files is a filesystem operation; telling Rekordbox
where they went is a staged change like everything else. The applier writes `FolderPath`, and
writes `FileNameL`/`FileNameS` **only if the database has them** — detected per-database with
`PRAGMA table_info` rather than assumed, since `decks` does not model those columns.

This also fixes a live bug: `RelocateBanner` staged path updates as
`TrackMetadataEdit { field: "folder_path" }`, which is not a `djmdContent` column and was rejected
by the applier's allowlist — so relocations staged from the UI never actually applied. It now
stages `TrackRelocate`. The frontend `ChangeKind` union was also missing `TrackDelete`,
`TrackAddCue` and `TrackDeleteCue`; it now mirrors the Rust enum.

Four Tauri commands in `src-tauri/src/organizer.rs`. `pattern_fields` returns the manual's full
28-field vocabulary with a `supported` flag, so the editor can say "decks cannot fill remixer yet"
instead of quietly rendering blanks. `preview_organize` plans without touching anything;
`apply_organize` executes exactly the rows it is handed back, so what runs is what the user read.
A file that cannot be moved fails alone — one locked file must not abandon the other 500.
`fs::rename` falls back to copy-then-remove for cross-filesystem moves, which is the common case
(downloads on the internal disk, library on an external one).

New `OrganizeFilesView`, reachable from the sidebar as **Move & Rename**, acting on the selection
or the whole library. Preview lists rows that would *not* change too, and the success toast says a
sync is still needed — per the manual, a partial sync leaves the old locations behind.

**Find Unused Files** ships in the same crate, because it is the same concern from the other side:
which files on disk the library does not account for. Include/exclude extension filtering (an empty
list means "no filter" in either mode, since an empty include list would report nothing and look
broken), the DJ-folder skip list matched case-insensitively, `Copy paths` to export without
deleting, and a timestamped record of every deletion under the app data folder.

Its output is a list of deletion candidates, so it carries three guards the manual does not
specify. A scan against an **empty library refuses to run** — everything would look unused. Library
membership is **re-checked at delete time**, because the library can gain a track between the scan
and the click. And path comparison is case- and separator-insensitive, since Rekordbox and the
filesystem do not reliably agree and a case-only mismatch would offer a real track for deletion.
Nothing is pre-selected and deletion needs an explicit second click.

**Bulk Write Tags** rounds out the slice: `write_tags_bulk` projects the library's values into the
files' own tags, with per-field selection, over the selection or the whole library. The rule that
matters is the one the manual does not state — **a selected field whose library value is empty is
not written**, because ticking "Artist" on a library that happens not to know one would blank a
perfectly good tag in the file. Those tracks come back as `skipped` and the panel says how many.
Nothing is ticked by default: this writes to files and cannot be rolled back through the staged-
change pipeline.

The sidebar entry is now **Files** rather than Move & Rename, with Move & Rename, Write Tags and
Find Unused Files as sections — they are one domain (things that write to disk rather than to
Rekordbox's database) and `docs/lexicon/06-files.md` treats them as one.

**Local Path Mappings** (cache migration **v8**) close the slice. Per-computer prefix rewrites so a
library restored on a second machine finds its music without a bulk relocate. Longest matching
prefix wins; matching is on whole path components, so `/Music` cannot swallow `/MusicVideos`;
separators are interchangeable because these databases cross platforms; and matching is
case-insensitive while the remainder keeps its original case — the comparison has to be lenient,
the filesystem may not be.

Read-side only. The library keeps saying `D:\Music\…`, which is exactly what lets one database
work on two machines at once. The table is deliberately **not** keyed by `library_path`: a mapping
describes where this *computer* keeps its music and has to apply the moment any library is opened.
Recorded as **ADR-0014**, since it breaks the pattern every other post-v5 table follows.

Applied at every point that turns a stored path into a real one: the missing-file scan (a mapped
track is not missing), Move & Rename's sources, Write Tags, and — the one that matters most — the
unused-file sweep's known-path set, since without it every mapped track would look unused and land
on the delete list.

**Quick move** (cache migration **v9**) closes the slice: remembered destinations, favourites first,
hotkeys 1–9. Recording a folder is an upsert, so using the same one twice moves it up the list
rather than duplicating it, and the hotkeys are ignored while a text field has focus — otherwise
typing a path fires a move on every digit. The move itself reuses the Move & Rename planner, so
collisions and `TrackRelocate` staging behave identically, and the success message repeats the
full-sync warning the manual is emphatic about.

**Watch folders** (cache migration **v10**) complete the automation story, with one deliberate
substitution: arrivals are found by a **debounced scan** every 15 seconds rather than a native
filesystem watcher. That makes the arrival set a pure function of (files on disk, library,
dismissed) — testable without an event loop, unable to miss something that happened while the app
was closed, and free of a platform-specific dependency. A push-based watcher would sit behind the
same function and change nothing the user sees.

Two rules the manual does not state, both earned rather than assumed. A file whose modification
time is under **10 seconds** old is held back and reported separately: a large FLAC copied over a
network share exists on disk long before it is complete, and importing mid-copy reads truncated
tags and a wrong duration. And dismissals are recorded, so a file the user chose not to import is
not offered again on every scan.

**New `ChangeKind::TrackCreate`, and it is export-only.** Inserting a row into `djmdContent` needs
columns `decks` does not model and cannot verify against a real fixture, and a half-populated row
in a performing library is worse than no row. So the applier **refuses** it — with a message naming
the file and pointing at the XML route — and the exporter emits the new tracks into the collection,
continuing IDs past the highest existing one so an import can never silently replace a real track.

**Automatic Actions** closes the epic's automation story, honestly. The settings group exists and
**Auto Analyze New Tracks** works: importing an arrival detects BPM and key on the way in, and a
failed analysis does not undo an import that already succeeded. The other four are shown as
**disabled toggles that state what they need** — automatic drop detection, the Beatshift Fixer,
field mappings, the enrichment providers — rather than hidden or offered as switches that quietly
do nothing. A switch that does not switch anything is worse than one that says why it is off, and
hiding them would make the gap invisible. An unavailable action also reads as off at the point of
use regardless of what is stored, so a setting enabled before its feature regressed cannot silently
take effect.

**What is NOT done in Epic 4:** incoming auto-advance and its hotkey, delete-from-disk, quick
move's context-menu entry point, field mappings, auto-write-on-change, enrichment (Find Tags &
album art), Energy/Danceability, and the Beatshift Fixer.

Verification: `cargo test --workspace` clean, clippy `-D warnings` clean, `cargo fmt --check`
clean, `pnpm test` 339, typecheck, lint, `pnpm e2e` 17 — all green. One CI-only clippy lint
(`unnecessary_sort_by`, 1.97) had to be fixed after the fact: the container's toolchain is 1.94, so
`cargo clippy` passing locally does not guarantee CI.

## 2026-08-06 — Epic 3 (part 1): cue templates and custom cue anchors
New `crates/cue-generator`, split in two so detection can land later without touching the placement logic:

**`anchor`** — the structural landmarks a template hangs off (`Start`, `Drop{ordinal}`, `Breakdown{ordinal}`, `FadeOut`, `End`) and `resolve_custom_anchors`, which maps the user's *existing* cues onto them. Lexicon's matching rules exactly: name+colour requires both, name-only takes the first cue with that name regardless of colour, colour-only takes the first with that colour regardless of name, and a rule with neither matches nothing rather than acting as a wildcard. Name matching is trimmed and case-insensitive. Anchors carry `Confidence`; a user-placed one is `Certain` because a human put it there.

**`template`** — declarative placement in beats relative to anchors, with name, colour, enabled and order. Offsets walk the beat grid rather than multiplying by a constant tempo, so they stay correct across tempo changes, falling back to arithmetic only past the end of the grid. Also: `keep_cue_position` (slot = template row, so "drop is always cue 1" survives a skipped row), overflow trimming that drops the **least confident first** then the latest in the track, and the **Rekordbox duplicate-memory-cue guard** — Rekordbox silently discards the second of two memory cues at one position, so we discard it ourselves and say which.

Nothing is silently dropped: every omission comes back as a typed `SkippedCue` (`AnchorMissing`, `OutOfRange`, `Overflow`, `DuplicateMemoryCue`) that the UI renders as a sentence.

Three Tauri commands in `src-tauri/src/cue_generator.rs`; preview and apply share one `generate` function so what the user reviews is exactly what gets staged. `suggest_anchor_rules` guesses anchors from cue names ("Drop", "Break", "Outro"/"Fade") so the panel opens with something to edit. `CueGeneratorPanel` is mounted in `TrackDetailPanel`.

**Honest labelling, per ADR-0008.** `Confidence` rides from anchor → cue → UI. Low-confidence cues render as `provisional NN%`, and the panel states plainly that automatic drop detection is not implemented rather than implying an analyser is running.

**What is NOT done:** structural segmentation — the actual drop/breakdown/fade-out detection — is the remaining half of Epic 3. Everything above is the placement machinery it will feed. Also outstanding: breakdown-min-beats, drop-at-start and auto-generate-on-play (all detection inputs), and finding a good emergency-loop spot.

One robustness fix found by the E2E suite rather than by unit tests: `CueGeneratorPanel` assumed `suggest_anchor_rules` returns an array. Two existing specs whose IPC mock returns `null` for unknown commands took the whole track panel down. Now guarded.

Verification: `cargo test --workspace` 725 passing, clippy `-D warnings` clean, `cargo fmt --check` clean, `pnpm test` 279, typecheck, lint, `pnpm e2e` 11 — all green.

## 2026-08-06 — Epic 2 (part 1): action registry, cue editing, quantize, beat jump
**Action registry** (`apps/desktop/src/lib/actions.ts`) — the substrate the rest of Epic 2 registers into. Every global capability is a named, bindable command; the Action Center searches that list, hotkeys bind to it, and inline hints read from it. Pure module covering binding serialisation, display formatting, matching (Cmd and Ctrl interchangeable), persisted user rebinding, conflict detection and fuzzy search. `ActionProvider` owns a single global key listener reading the action list through a ref so it installs once. App's four global shortcuts migrated onto it; component-local key handling (track-table arrows) deliberately stays in `useKeyboardShortcuts`. `ActionCenter` is the `Cmd/Ctrl+Space` palette.

**Cue editing.** New `ChangeKind::TrackDeleteCue` + applier (deleting a missing cue errors rather than silently succeeding — it means the staged change was built against a stale view). New `crates/rekordbox-db/src/quantize.rs`: pure beat-grid arithmetic — nearest beat, snap at 1/2/4/16/64-beat resolutions measured from the first grid marker, `is_on_grid`, beat jump clamped at both ends, and `cues_following_grid`, which returns only the cues that were already on the grid. That last one is the subtle bit: a grid nudge must move on-grid cues and leave deliberately off-grid cues alone. Seven Tauri commands in `src-tauri/src/cues.rs`, all staging changes rather than writing `master.db`. `CueEditor` is mounted in `TrackDetailPanel`, so cues are editable from the UI: 1–8 set or play, Cmd/Ctrl+1–8 delete, Q toggles quantize, plus colour, loop length in beats, move-to-playhead, beat jump and grid nudge.

Quantising an existing loop shifts its out-point by the same delta as its in-point, so loop *length* is preserved rather than stretched.

**Not done in this slice, and why** — recorded in ROADMAP and PARITY rather than left implicit:
- **Active loops are blocked**, not deferred by choice: they need a `djmdCue` column our schema does not model. Loop length works; auto-engaging loops do not.
- **Beatgrid writing** (ANLZ) and half/double BPM: the grid nudge stages the cue moves that *follow* a grid change, but nothing writes the grid itself yet.
- Cue placement is on the cue list and slot grid; dragging cues on the waveform is not implemented.
- Find Popup, play queue, cue templates, and the Cue Destination round-trip remain.
- Hotkey rebinding exists in the registry (with persistence and conflict detection) but has no settings UI, and there are no system-wide hotkeys.

Verification: `cargo test --workspace` 698 passing, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `pnpm test` 269, `pnpm typecheck`, `pnpm lint`, `pnpm e2e` 11 — all green.

## 2026-08-05 — Epic 1: Smartlists engine
New `crates/smartlists` implementing the rule model from ADR-0013: a **two-level** structure (AND of OR-clauses) rather than a general boolean tree, matching what Lexicon actually exposes — OR grouping is only offered in "All Rules" mode. Evaluation is pure and in-memory over `&[Track] + EvalContext`, where `EvalContext` mirrors the frontend `FilterContext` so both sides share semantics. Key equality canonicalises through `changes::key_format` plus a new Open Key parser, so `8A`, `8a`, `8m`, `Am` and `A minor` all match the same track. Archived tracks are excluded unless a rule mentions `IsArchived`; an empty rule set matches nothing rather than the whole library. Also ships the Smartlist Generator (by field / tag category / decade / BPM range / play count) with `only_missing` as the idempotency guard, and a 30-second recompute throttle with injected time.

Cache migration **v7** adds a `smartlists` table storing rules as a JSON document. Normalised clause/rule tables were dropped from the plan: evaluation is in-memory so nothing ever queries an individual rule, and `staged_changes` already stores JSON in this DB. `parent_folder_id` doubles as the generator's ledger — generated smartlists sit in the reserved `Lexicon` folder and moving one out is what makes the generator recreate it, so no extra bookkeeping state exists.

Sync: `SyncOptions.all_smartlists_to_playlists` is now honoured (it had been accepted and ignored since Sub-Plan 6). `sync_execute` stages `PlaylistCreate` + `PlaylistAddTrack` for each smartlist before collecting the change set. Playlist IDs derive from the smartlist ID rather than being freshly generated, because the applier requires `target_id` to be stable between preview and apply. `Excluded From Sync` conventions implemented for both names (case-insensitive prefix) and the custom tag. `rekordbox_compatibility` encodes what Rekordbox 6/7 can hold — MyTag only, `has all`/`has none` only, ≤2 rules and ≤4 categories — and drives the editor's indicator.

Nine Tauri commands in `apps/desktop/src-tauri/src/smartlists.rs`; `SmartlistList` / `SmartlistEvaluate` added to `agent-tools::ToolRequest` and MCP. Frontend: `SmartlistsView.tsx` (list + rule editor + generator) with live match count, wired into `SidebarNav`.

Deviations from the plan, all deliberate: (1) evaluation is in-memory rather than the SQL/in-memory hybrid ADR-0013 proposed — an **Implementation note** amending the ADR explains why; (2) smartlists ship as a dedicated view rather than inside the playlist tree, since Rekordbox playlists come from `master.db` and smartlists from the cache DB — merging the trees is deferred to Epic 6; (3) the tag query language for the *search box* and the Tags-page OR/AND selection are deferred to Epic 5 — the rule engine already expresses both.

Also found: `field_mappings` and `sync_runs` were created in migration v5 and are **read and written by nothing**. `field_mappings` is additionally the wrong shape — its `(library_path, source_field)` PK cannot express multi-source combining or per-app mappings, so Epic 4 should replace rather than extend it. Recorded in `docs/data-model.md` and `PARITY.md`.

Verification: `cargo test --workspace` (incl. `decks-desktop`), `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm test` (224), `pnpm typecheck`, `pnpm lint`, `pnpm e2e` (11) — all green. GTK/webkit/ALSA dev packages were installed in the dev container, so the Tauri shell and Playwright now run locally instead of only in CI.

## 2026-08-05 — Epic 0: Lexicon parity spec, parity matrix, roadmap, GPL-3 relicense
Kicked off the Lexicon parity initiative. Wrote `docs/lexicon/` — twelve domain specs (overview, interop, library, smartlists, analysis, cues/player, files, health, streaming, history/backup, recipes, extensibility) derived from the official 95-section Lexicon manual supplied by the project owner, plus `PARITY.md` (a 102-row feature matrix: 4 done, 31 partial, 51 missing, 16 deferred) and `GAPS.md`. Added `docs/ROADMAP.md` with eight epics, one branch and draft PR each, Rekordbox-first scope. Appended ADR-0011 (relicense MIT -> GPL-3.0-or-later so libKeyFinder/aubio/audiowaveform become usable), ADR-0012 (analysis stack: beat_this_cpp MIT, libKeyFinder GPL-3, libebur128 MIT, Chromaprint non-FFTW; excludes madmom and Essentia TF models as non-free CC-BY-NC weights, and Spotify audio-features as deprecated), and ADR-0013 (two-level smartlist rule model, not a general boolean tree). Added a root `CLAUDE.md`, which the repo previously lacked. Relicensed: `LICENSE` is now GPL-3 with the reklawdbox MIT grant preserved in `NOTICE`; `Cargo.toml` and `package.json` declare `GPL-3.0-or-later`; `crates/stratum-dsp` keeps its own MIT OR Apache-2.0 and is noted in `NOTICE`. The Lexicon manual itself is gitignored at `docs/lexicon/source/` — it is copyrighted third-party documentation and this repo is public.

Three findings worth flagging from the manual that were not previously known:
1. **Lexicon deliberately does not ship Camelot notation** ("due to licensing restrictions") and uses Open Key instead. `decks` uses Camelot throughout, including a palette explicitly labelled Mixed In Key's. Decision needed before the repo goes public — logged in `docs/lexicon/GAPS.md`.
2. **Recipes is a second, much larger bulk-edit system** (~44 parameterized operations across 8 categories) distinct from the ten fixed Smart Fixes. `decks` has parity on Smart Fixes and none of Recipes.
3. **`crates/scoring::score_transition` and the `suggest_next_tracks` Tauri command have no frontend caller** — Mixable Tracks is half-built and stranded. Epic 6 picks it up.

Verification: `cargo test --workspace --exclude decks-desktop` all green (0 failures). The Tauri shell cannot build in the Linux dev container (`gdk-3.0` missing; project targets macOS/Windows), so `decks-desktop`, `pnpm test/typecheck/lint` and `pnpm e2e` were not run — this epic changes no code, only docs and licence metadata.

## 2026-05-25 — Sub-Plan 8c: Library-wide duplicate detection + DuplicatesView
Extended `rekordbox_db::DuplicateGroup` with `kind: DuplicateKind` (ExactTitleArtist | FuzzyTitle | AudioFingerprint) and `confidence: f32`; both `#[serde(default …)]` so legacy responses still deserialize. Rewrote `audio_fingerprint_duplicates` to bucket by the first 4 chromagram bytes then pairwise within buckets (turns 50k-track O(n²) into a sum of O(k²) per bucket), with `FINGERPRINT_HAMMING_MAX_BITS = 10` constant and per-bit-count Hamming (`hamming_bits`) so the threshold is on bits, not bytes. Added `library_duplicate_groups` (queries + `RekordboxDb` method) that runs all three strategies in one call, and a new `library_duplicate_groups` Tauri command in `apps/desktop/src-tauri/src/lib.rs` that pulls fingerprints from `cache.sqlite3.audio_fingerprints` via `CacheDb::get_all_fingerprints` (degrades to empty map on cache miss/error). Frontend: new `DuplicatesView.tsx` with per-group radio "keep one" picker → `archive_tracks`, "Open in inspector" per row, and grouped header chips by kind; wired into `SidebarNav` (`duplicates` entry between Smart Fixes and Track Matcher) and `App.tsx` (`onOpenInspector` swaps inspector to `details`). New IPC: `listLibraryDuplicateGroups`. Tests: 4 new Rust unit tests in `queries::health` (`groups_exact_title_artist`, `groups_fuzzy_title_above_threshold`, `groups_fingerprints_within_hamming_threshold` covering both pair-match and >10-bit reject, `library_groups_combines_all_three_strategies`) + 4 new vitest cases in `DuplicatesView.test.tsx` (renders three kinds, keep-one archives the others with correct IDs, empty state, open-inspector callback). Verification: `cargo test --workspace` ok, `cargo clippy --workspace --all-targets -- -D warnings` clean, `pnpm test` 208/208, `pnpm typecheck` + `pnpm lint` clean, `pnpm e2e` 8/8.

## 2026-05-24 — Sub-Plan 8b: Waveform peaks persisted across restarts
Added migration v6 (`crates/cache/src/migrations.rs`) creating `waveform_peaks(track_uri PK, peaks BLOB, sample_count, generated_at)` plus `CacheDb::set_waveform_peaks` / `get_waveform_peaks` accessors storing little-endian `f32` blobs. The `get_audio_waveform` Tauri command now checks the cache first (honoring the requested `bars` count via `sample_count` match), only decoding via symphonia on miss, and persisting non-empty results. 3 new Rust tests (`waveform_peaks_round_trips`, `_returns_none_for_unknown`, `_overwrite_replaces_data`).

## 2026-05-24 — Sub-Plan 8a: Filter persistence per library
`loadPersistedFilters` / `persistFilters` now key localStorage by `libraryPath` (`decks.filters.v1::<path>`, with the un-keyed `decks.filters.v1` retained as a legacy fallback for `null`). `App.tsx` re-loads filters when the active library changes and writes persist scoped to the current library. Added 6 vitest cases in `apps/desktop/src/lib/filters.test.ts` covering round-trip, library scoping, `query`/`missingFiles` reset on reload, legacy null-key behaviour, quota-exceeded silent-fail, and malformed-JSON recovery.

## 2026-05-24 — Sub-Plan 6: Sync options wired end-to-end
`SyncPanel`'s previously-stubbed option controls (`cue_destination`, `keep_grids`, `convert_keys`) are now live. Added `changes::applier::SyncOptions` + `CueDestination` + `KeyFormat` (snake_case serde), `apply_with_options(tx, &[changes], &opts)`, new `crates/changes/src/key_format.rs` with `to_camelot` / `to_open_key` (24-key table + enharmonics, parse-failure-returns-original semantics), and writer-side honoring in `applier/tracks.rs` (Key field conversion) and `applier/cues.rs` (`Kind` slot selection for hot/memory/both). The `SyncOptions` struct in `apps/desktop/src-tauri/src/lib.rs` now forwards into `apply_with_options`; `ipc.ts` `SyncOptions` gained `cue_destination` / `keep_grids` / `convert_keys` / `change_to_nearest_color` / `all_smartlists_to_playlists`. UI controls are no longer disabled. Per ADR-0010, the "never mutate master.db" invariant is formally relaxed for the opt-in Sync feature under `WriteGuard` (lock probe + timestamped backup + transactional writes). New tests: 4 in `key_format`, 4 in `applier::tracks` (`key_conversion_camelot_rewrites_value`, `_open_key_`, `_original_passthrough`, `_invalid_falls_back_to_original`), 3 in `applier::cues` (`add_cue_memory_destination_forces_kind_zero`, `_both_inserts_two_rows`, `_hot_preserves_staged_slot`), 2 in `applier` (`keep_grids_skips_bpm_edit`, `_false_writes_bpm_edit`), plus 1 vitest in `SyncPanel.test.tsx` (`forwards non-default … to syncExecute`). Partial / deferred (documented in `docs/MANUAL_TEST_PLAN.md`): `keep_grids` only covers BPM `TrackMetadataEdit` — beat-grid ANLZ writes are not staged anywhere today, so there is nothing else to skip; `cue_destination` only governs the `Kind` value of newly inserted cue rows, it does not retroactively re-slot existing cues; `change_to_nearest_color` / `all_smartlists_to_playlists` are plumbed through the option struct but not yet honored. Real-library disposable-DB smoke is queued for manual verification — synthetic-fixture tests pass on this branch.

## 2026-05-24 — Sub-Plan 7: enhanced track columns
TrackTable now exposes an **Energy** column (bar visual; hydrated from `cache.audio_features.energy` per `Track.folder_path`), tints **Key** values with the Mixed In Key Camelot palette (new `lib/camelot.ts`), and conditionally renders an **inline Tags** column with up to three chips + overflow when any tag bindings exist for the active library. Rust `Track` gained `energy: Option<f32>`; `list_tracks` / `library_search` / `list_incoming_tracks` / `list_archived_tracks` call a new `hydrate_energy` helper backed by a batched `CacheDb::get_energy_by_uris` lookup (no N+1).

## Current phase
QA-pass remediation (2026-05-17): fixed six functional bugs that evaded the green test suites — missing `stream_claude_code_chat` Tauri command (re-implemented), ANLZ path-join bug in intro-cue staging (two sites, now share `anlz::resolve_anlz_path` helper), wrong Claude model id (`claude-opus-4-5` → settings-driven, defaults to `claude-sonnet-4-6`), global spacebar handler that swallowed button activation (moved to shared `useKeyboardShortcuts` with button/link/role=button exclusions), `is_playing` never clearing at end of track (audio thread now emits `playback-ended` and clears state), and Relocate flow staging `old_value: null` instead of the original path (now stages real old path + invalidates library/missing-files queries). Manual real-library verification still the v0.1.0 blocker.

## Current task
Manual real-library verification — the data-layer half is now automated via `scripts/real-library-smoke.sh` (13/13 against `~/Library/Pioneer/rekordbox/master.db`, sha256 unchanged). What still requires a human at the UI: first-run wizard walkthrough, track-table scroll smoothness, column sort interaction, theme persistence across restarts, spacebar focus rules, Anthropic key keychain prompt, chat panel mount/unmount, and a fresh `pnpm --filter desktop tauri build` artefact. `master.db` writes are still prohibited.

## Verification baseline
- `cargo test --workspace`: passing as of 2026-05-17 (test count up after adding `claude_agent::parse_stream_line` parser tests and `anlz::resolve_anlz_path` regression tests)
- `cargo clippy --workspace --all-targets -- -D warnings`: clean as of 2026-05-17
- `pnpm test`: passing as of 2026-05-17 (139 tests — +5 for new `useKeyboardShortcuts` test, +1 for SettingsPanel model select, -1 stale spacebar test in `useAudioPlayer`)
- `pnpm typecheck`: passing as of 2026-05-17
- `pnpm lint`: passing as of 2026-05-17
- `pnpm build`: passing as of 2026-05-15
- `pnpm e2e`: passing as of 2026-05-15 (4 Playwright tests)
- `pnpm --filter desktop tauri build`: passing as of 2026-05-16 — fresh `target/release/bundle/dmg/decks_0.1.0_aarch64.dmg` (9.1 MB) and `target/release/bundle/macos/decks.app/Contents/MacOS/decks-desktop` (arm64 Mach-O). Info.plist reports CFBundleShortVersionString=0.1.0, CFBundleIdentifier=app.decks.desktop. Launch verification still pending.

## Current true implementation state
- [x] Repo scaffold, Cargo workspace, pnpm workspace, CI workflow.
- [x] `crates/rekordbox-db`: read-only SQLCipher connection, tracks, playlists, playlist entries, cues, ANLZ beat grid parser.
- [x] `crates/rekordbox-xml`: parse and emit Rekordbox XML with round-trip tests.
- [x] `crates/cache`: SQLite WAL cache with schema migrations and audio-feature cache.
- [x] Desktop app: Tauri 2, React, Vite, Tailwind, first-run library selection and validation.
- [x] Library UI: virtualized track table with filter and sort.
- [x] Track detail UI: metadata and cue display, with visible cue-load failures.
- [x] Audio preview: native rodio play/pause for selected track.
- [x] Waveform rendering and scrub controls: high-fidelity native Pioneer color waveform (Phase 17) plus interactive seek/playhead (Phase 21).
- [x] Settings: theme, library path change, Anthropic API key in OS keychain, and Claude Code install/login/subscription detection.
- [x] Agent read-only MVP tools: search, get track, list playlists, get playlist, list cues, orphan scan, duplicate scan, broken metadata scan.
- [x] Playlist support: backend playlist detail tool and basic playlist panel UI.
- [x] Conversation persistence.
- [x] Safe staged changes and diff review.
- [x] Export accepted changes to Rekordbox XML.
- [x] One-click audit workflow entry point in the agent panel.
- [x] Playwright E2E tests.
- [ ] Real Rekordbox library manual verification.
- [x] macOS release build artifacts generated.
- [x] Final UI audit and redesign recommendations documented.
- [x] Implemented phase 11 UI polish (empty states, panel layout, zero values, placeholder waveform).
- [x] Deterministic synthetic fixture generator: `scripts/seed-test-library.sh`.
- [x] Playlist view fills available workspace height instead of a fixed short band.
- [x] Cue query supports additional real-library `djmdCue` column variants.
- [x] Phase 12 — second UI polish pass: denser track table (28px rows, IBM Plex Mono numerics), labeled sidebar (168px, amber active rule), structured filter system with drawer + chips (BPM/year ranges, key/genre multi-select, missing-metadata toggles, has-cues, not-in-any-playlist, comment-contains), playlist duplicate badges + duplicate count, expanded playlist columns (health dot, genre, time, year), inspector empty state, always-visible Details toggle.
- [x] Two new read-only IPC commands: `list_tracks_with_cues`, `list_tracks_in_any_playlist`.
- [x] Confirmed playlist duplicates are real Rekordbox `djmdSongPlaylist` entries — surfaced via DUP badge, not deleted.
- [x] Shared `agent-tools` Rust service for provider-neutral tool execution.
- [x] `decks mcp` local stdio MCP server for Claude Code, Gemini CLI, and other local MCP hosts.
- [x] `decks tools call` diagnostic CLI for direct tool invocation.
- [x] HTTP MCP transport for OpenAI Responses API remote MCP usage (`decks mcp-http --bind <addr>`).
- [x] `crates/stratum-dsp`: vendored DSP engine (BPM detection, key detection, beat-grid HMM) from reklawdbox.
- [x] `crates/audio-tags`: lofty-based tag read/write for MP3/FLAC/M4A/WAV (title, artist, album, genre, BPM, key, comment, year, duration).
- [x] `crates/audio-analysis`: Symphonia decode + stratum-dsp analyze + Camelot key conversion + `audio_features` cache integration.
- [x] Tauri commands: `read_audio_tags`, `analyze_track`, `write_audio_tags`.
- [x] Agent MCP tools: `library.read_file_tags`, `library.analyze_track`, `library.scan_and_propose_missing`.
- [x] Track inspector: "Analyze" button → analysis result section with BPM/key/confidence + "Propose correction" buttons.
- [x] Phase 14 — ElevenLabs UI integration: synthetic StaticWaveform behind cue markers (clearly labeled "preview" — real audio analysis still deferred), Message + Response components for chat bubbles, ShimmeringText for agent thinking state, Conversation + scroll button for the message list. Added `@/*` path alias, shadcn-compatible Tailwind/CSS-var aliases, and shadcn Button/Avatar primitives.
- [x] Phase 15 — Fixed Claude Code `stream-json` output format parsing bug in Rust backend. Sub-process text is now properly emitted and integrated via `useAgent` progressive chunking, making the local subscription-backed Claude Code chat fully operational.
- [x] Phase 16 — UI/UX Layout & Filtering Enhancements (implemented by Gemini agent): Collapsible sidebar nav, resizable inspector panel (Chat/Details), resizable table columns, inline column search filters, non-blocking filter drawer, searchable multi-select Radix UI dropdowns for Key/Genre, Cmd/Shift multi-select track selection with summary action bar, and a toggle to hide the playlist sidebar for more space.
- [x] Phase 17 — Native Pioneer Waveform Rendering (implemented by Gemini agent): Reverse-engineered ANLZ parser in Rust (`PWAV`, `PWV3`, `PWV4`, `PWV5`) and high-fidelity `<ColorWaveform>` Canvas component replacing the synthetic placeholder.
- [x] Phase 18 — Smart Missing File Relocation (implemented by Gemini agent): `relocate` crate with fuzzy filename matching (Levenshtein), exposed via `relocate.scan` agent tool and `<RelocateBanner>` bulk-fix UI.
- [x] Phase 19 — Analytics Dashboard (implemented by Gemini agent): Efficient SQLite backend aggregation and `recharts` frontend UI (`<AnalyticsView>`) for genre, BPM, and key distributions.
- [x] Phase 20 — Audio-Fingerprint Duplicates (implemented by Gemini agent): Chromagram 128-byte hash extraction via `stratum-dsp`, cached persistently, with Hamming-distance grouping for detecting true duplicate audio files.
- [x] Phase 21 — Audio Playback Scrubbing (implemented by Gemini agent): `rodio` seek wiring and active polling for interactive waveform clicking and playhead tracking.
- [x] Phase 22 — The Inbox Workflow & Bulk Cues (implemented by Gemini agent): Dedicated `InboxView` for tracks missing metadata/cues/playlists, and a bulk "Add Intro Cues" tool that parses ANLZ beat grids to calculate mathematically perfect 1.1 downbeats and 4-bar loops.
- [x] Post-Gemini remediation (2026-05-15): finished the missing `library_stage_intro_cues` Tauri command + `Relocate*` enum variants the Gemini sessions left dangling, added unit tests for intro-cue logic and synthetic ANLZ PWAV/PWV3/PWV4/PWV5 parsers, added `PlaylistRemoveTrack` / `PlaylistDelete` export tests, added `audio_fingerprints` migration test, and removed dead `health__audio_fingerprint_scan` UI plumbing that called an unimplemented Tauri command. Removed the unfinished `SetBuilderView.tsx` Phase 3 stub (out-of-scope, didn't typecheck).
- [x] Custom-tags hardening (2026-05-24): `usage_count` badge surfaced from a new `list_tags` projection, `list_track_tags_map` IPC + `FilterContext.tagsByTrack` hydration, `tagIds`/`tagMatchAll` filter dimension, FilterDrawer Tags section + FilterChips chips, global `T` shortcut + right-click "Edit tags…" action wired to `TagPickerModal`, and a "Show N tags in library" jump-to-filter button on `CustomTagsPanel`.

## MVP phase checklist
- [x] Phase 0 — Repo familiarization and status reconciliation.
- [ ] Phase 1 — Stabilize current foundation and tag `v0.1.0`. (Ready for manual verification)
- [x] Phase 2 — Define MVP agent and playlist scope.
- [x] Phase 3 — Implement missing read-only agent tools and playlist view.
- [x] Phase 4 — Persist conversations.
- [x] Phase 5 — Safe staged changes system.
- [x] Phase 6 — Inline diff review UI.
- [x] Phase 7 — XML export.
- [x] Phase 8 — One complete MVP workflow.
- [x] Phase 9 — Playwright E2E.
- [x] Phase 10 — Local macOS release build.
- [x] Phase 11 — Full UI audit and redesign suggestions.

## Blockers
- Real Rekordbox 7 `master.db` manual testing requires access to a local user library. **Data-layer portion is now automated** — see `scripts/real-library-smoke.sh` (13/13 against the real library at `~/Library/Pioneer/rekordbox/master.db` on 2026-05-16, sha256 unchanged; covers `library_search`, `library_get_track`, `library_list_playlists`, `library_get_playlist`, `library_list_cues`, all four health scans, `staging_list_changes`, `library_read_file_tags` and (opt-in) `library_analyze_track`). UI-only items still need a human.
- Packaged app artifacts rebuilt fresh on 2026-05-16 at `target/release/bundle/macos/decks.app` and `target/release/bundle/dmg/decks_0.1.0_aarch64.dmg`; bundle structure verified (Info.plist OK, arm64 Mach-O binary present). Manual launch verification against a real/disposable library is still required.
- Claude Code is detected locally. Subscription-backed Claude use is now supported through Claude Code as the MCP host; the embedded Tauri chat still uses Anthropic API keys.
- Custom Tags drag-to-reorder/move (Sub-Plan 1 Step 9) is deferred. The backend `move_tag` IPC exists, but wiring `@dnd-kit` for chip drag is blocked on a new `reorder_tags` command for within-category ordering; see comment in `apps/desktop/src/components/CustomTagsPanel.tsx`.

## Sub-Plan 2 — Genre/Artist Cleanup test coverage (2026-05-24)
Added `apps/desktop/src/components/CleanupPanel.test.tsx` (7 tests covering list/rename/delete for both `mode="genre"` and `mode="artist"`, shift-click multi-select, and empty/disabled states) plus Playwright `apps/desktop/e2e/cleanup.spec.ts` exercising the full rename → stage → accept → export round-trip. Smoke-script `list_genres` block deferred: no MCP/CLI tool exists for it (Tauri-only command), and adding one would be new feature code beyond this sub-plan's scope.

## Sub-Plan 3 — Smart Fixes E2E + rigor (2026-05-24)
Added `apps/desktop/e2e/smart-fixes.spec.ts` — full preview → deselect-one → stage → "Changes" round-trip against `smart_fix_preview` / `smart_fix_apply` / `list_changes` IPC mocks (fix_casing, 3 proposals, deselect middle, assert staged count = 2 and right field/old/new values surface in DiffReviewPanel). Added two Rust edge-case tests to `crates/smart-fixes/src/fixes/add_mix_parens.rs`: bare-suffix-only titles ("Original Mix", "Remix") yield no proposal, and a title already ending in a parens group is never double-wrapped even when an earlier suffix word matches.

Deferred (documented gaps, not fixed — they are feature work beyond this rigor sub-plan):
- Smoke-script `smart_fix_preview` block: deferred because `smart_fix_preview` / `smart_fix_apply` / `common_text_blocklist_*` are Tauri-only commands and have no `decks tools call` (MCP) exposure (`grep smart_fix crates/agent-tools/src/mcp.rs` → no matches).
- Common-text blocklist UI: only the IPC wrappers exist (`commonTextBlocklistList` / `Add` / `Remove` in `apps/desktop/src/ipc.ts:603-613`). No UI in `SmartFixesPanel.tsx` consumes them, so there is nothing to vitest. Surfacing the blocklist as a settings sub-panel is a follow-up feature.

## Sub-Plan 5 — Track Matcher CSV + round-trip (2026-05-24)
Moved CSV parsing for the Track Matcher off the UI thread and onto the Rust backend. New `track_matcher::csv_input::parse_csv(input, title_col, artist_col)` powers a Tauri command `parse_csv_for_matcher` (IPC: `parseCsvForMatcher`), so the renderer no longer carries a hand-rolled CSV tokeniser. `TrackMatcherView` keeps the existing column-mapping UI (title required, artist optional) but now delegates the parse step, and the headers used by the dropdowns come from the file's first line on upload. Added 6 integration tests in `crates/track-matcher/tests/csv_roundtrip.rs` covering happy-path, extra-column ignore, missing-title error, empty-row skip, artist-omitted, and a full CSV → `match_all` → assert-results pipeline against an in-memory library. Vitest in `TrackMatcherView.test.tsx` now mocks `parseCsvForMatcher` and asserts the column-mapping UI surfaces. External APIs (Spotify, YouTube, etc.) remain explicitly out of scope — paste/.txt/.csv only.

## Sub-Plan 4 — Incoming/Archive verification (2026-05-24)
Confirmed `IncomingView` and `ArchiveView` ship the documented header actions (`Mark all reviewed`, `Archive selected`, `Unarchive`, `Delete from library`) with vitest coverage already in place (`IncomingView.test.tsx` 3 tests, `ArchiveView.test.tsx` 3 tests). Added `apps/desktop/e2e/incoming-archive.spec.ts` (2 tests): inbox "mark all reviewed" round-trip (`1 new track` → confirm dialog → `0 new tracks`) and archive unarchive round-trip (`1 archived track` → row click selects → Unarchive → `0 archived tracks`), both driven via mocked `list_incoming_tracks` / `list_archived_tracks` / `clear_incoming` / `unarchive_tracks` IPC.

Deferred (documented gaps, not fixed — feature work beyond this rigor sub-plan):
- **Track context-menu actions for Incoming/Archive.** Per spec, right-click on a row in Incoming should offer "Mark as reviewed" + "Archive", and Archive should offer "Unarchive" + "Delete from library". `useTrackContextActions` (`apps/desktop/src/hooks/useTrackContextActions.tsx`) does NOT include any of these actions — its menu only exposes Show details / Play / Analyse / Edit tags / Stage intro cue / Reveal / Copy path / Copy ID / Remove from playlist (when playlistId is set). The views still surface the same operations via header buttons (which the new E2E spec covers), so the user-visible workflow is intact, but the right-click parity is missing. Wiring this requires plumbing a per-view "extra actions" param through `App.tsx#handleTrackContextMenu` → `useTrackContextActions` to inject view-specific items keyed on the current view, plus 4 new menu entries; >20 lines + multiple tests, so deferred as feature work.
- **Smoke-script `list_incoming_tracks` / `list_archived_tracks` blocks.** Deferred — `grep -nE 'list_incoming|list_archived|incoming|archived|archive_tracks|unarchive|clear_incoming|stage_track_delete' crates/agent-tools/src/mcp.rs` returns zero matches, so neither command is reachable via `decks tools call`. Adding the MCP exposure is new feature code.
