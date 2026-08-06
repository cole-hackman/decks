# 05 — Player, Cues, Beatgrid, and the Cue Point Generator

Owned by **Epic 2** (player and editing) and **Epic 3** (generator).

Lexicon's framing is worth internalising: *"The music player is intended to be your primary music
player, connected directly to your DJ library."* It is not a preview widget bolted onto a browser —
it is the editing surface, and every cue/loop/grid operation happens there.

---

## Music player

*What it does* — Full playback engine over the library. Waveform with double-click-to-seek, a play
queue with autoplay, beat jump, and active loops.

*Queue* — right-click → `Add to queue`, or drag tracks onto the queue. Right-click a queued track to
delete, play immediately, or `View` it in the collection. Edit menu offers `Shuffle Queue` and
`Clear Queue`.

*decks status* — **partial.** `rodio` transport with play/pause, seek, interactive waveform
scrubbing and a `playback-ended` event in `apps/desktop/src-tauri/src/audio.rs`, plus a **play
queue with autoplay**. No beat jump.

**The queue** is `lib/play-queue.ts` (pure list arithmetic), `usePlayQueue` (resolving ids to
tracks and driving the transport) and `PlayQueuePanel`. Right-click offers `Add to queue` and
`Play next`; the panel reorders, removes, jumps, shuffles and clears; `Cmd+K` reaches
`Toggle Play Queue`, `Next in queue` and `Previous in queue`.

Four decisions worth recording:

- **Track ids, not tracks.** The queue survives a library refresh, a filter change and a re-sort.
  Holding whole `Track` objects would pin stale copies of rows the user has since edited.
- **One list with a marker in it**, not a "now playing" box above a separate list. The queue is the
  thing the user reasons about; splitting it in two makes "what comes after this" harder to read.
- **`Clear Queue` keeps the playing track**, leaving it as the sole entry. Clear means "nothing
  after this", not "stop the music" — stopping is the transport's job and has its own control.
  `Shuffle` likewise only permutes what has *not* played, since shuffling history would move the
  marker under the playing track and read as the queue losing its place mid-set.
- **Advance is driven by a timestamp, not a flag.** `playback-ended` sets `endedAt = Date.now()`;
  a boolean would stay `true` after the first end and the second track would never advance.

**The queue is per-session and in-memory.** A queue is what you are about to play right now;
persisting it would mean opening the app tomorrow to last night's leftovers. Playlists are the
thing that persists.

Reordering is up/down buttons rather than drag-and-drop: the track table has no drag *source* yet,
so a drop target here would be half a feature. The buttons work from the keyboard, and dnd-kit can
replace them later without changing the queue's semantics.

*Epic* — **2**.

---

## Cue points

*What it does* — Add, delete, name, colour, and reposition cues directly on the player waveform.

*Interaction model* — this is unusually well specified and worth matching exactly:

| Gesture | Effect |
|---|---|
| `1`…`8` | Set cue N if empty, play cue N if set |
| `Ctrl/Cmd` + `1`…`8` | Delete cue N |
| Double-click waveform | Seek there (the documented way to place cues precisely) |
| `Shift` + click a cue | Move that cue to the current, quantised position |
| `Ctrl` + click a cue | Delete it |
| Hold `Shift` while dragging waveform | Slow-scrub for precision |
| Drag a cue | Re-arrange cue order |
| Right-click a cue | Colour, name, convert to loop |

*decks status* — **read-only.** `decks` displays cues (including in the native ANLZ waveform) and
can *stage* a bulk "add intro cues" operation from the beat grid, but there is no interactive
cue editing at all. `ChangeKind` has `TrackAddCue` and `CueMetadataEdit` but no delete.

*Epic* — **2**.

---

## Loops

*What it does* — Any cue becomes a loop by setting a loop duration. A loop can be marked **Active**,
meaning it auto-engages when the playhead reaches it.

*Portability* — active loops are transferred only to Rekordbox, Engine DJ, VirtualDJ and djay Pro.

*Nice detail* — active loops can be globally suppressed in the player, because auto-triggering
loops make the app useless as a listening tool.

*decks status* — **missing.**

*Epic* — **2**.

---

## Quantize

*What it does* — Snaps cue placement to the beatgrid. Toggled from the Edit menu, a player button,
or the `Q` hotkey.

*Interaction with grid editing* — when quantize is on and you move the beatgrid, **cues that were
already on the grid move with it**; cues that were off-grid are left alone. That selective
behaviour is the correct semantic and is easy to get wrong.

*decks status* — **missing.**

*Epic* — **2**.

---

## Cue templates

*What it does* — Saved name+colour presets so the same cue kinds don't need re-entering. Created by
right-clicking an existing cue and promoting it into an empty template slot. Templates are
immutable — to change one you delete and re-create it. Unlimited count; a fresh empty slot appears
once existing ones are filled. Hotkeys bind the first eight.

*Applying* — press the cue button, or bind a per-template hotkey; if the playhead sits exactly on a
cue, the hotkey applies that template to it.

*decks status* — **done, under a different name.** These ship as **cue presets**, not cue
templates: `crates/cue-generator` already owns `CueTemplate` for its bulk-generation rule sets
(place one at the first downbeat, one at the drop…), and two things called "template" in one player
would be unreadable. Cache migration **v18**, `cache::CuePreset`, five IPC commands, and a preset
bar in `CueEditor`.

- **Immutable, per the spec.** There is no update path — changing a preset means deleting it and
  creating it again. That is what keeps the hotkey a stable promise: `2` applies what `2` applied
  last set, rather than whatever someone silently edited it into.
- **Created by promoting a cue**, per the spec. `Save preset` on a cue lifts its name and colour.
  An unnamed cue is refused rather than saved blank — a preset is a name *and* a colour, and there
  is nothing to save yet.
- **Applying stages; it never writes.** One `CueMetadataEdit` per field that actually changes, so a
  preset with no colour stages only the name (which is what "leave the colour alone" has to mean)
  and re-applying the same preset stages nothing at all.
- **Deleting closes the gap.** The first eight presets carry hotkeys 1–8; leaving a hole would
  retire a key while the ones after it kept their old numbers, so deleting the second preset would
  leave `2` dead and `3` still on the third. Reordering is therefore how a preset's hotkey changes.
- **Not scoped by library.** A preset describes how *this DJ* labels cues, not anything inside a
  particular database, so it survives opening a different library. That differs from
  `favourite_playlists`, which is scoped, and the migration says why.

Two divergences: duplicate names are allowed ("Drop" in red and "Drop" in orange is a reasonable
thing to want, and rejecting it is a rule the spec does not ask for), and applying goes through an
explicit **target** cue rather than the playhead's position. Position-based targeting reads well in
prose and badly in practice — "exactly on a cue" is a millisecond comparison the user cannot see,
and getting it wrong stamps a preset onto the wrong cue.

*Epic* — **2**.

---

## Beatgrid editing

*What it does* — `Edit Grid` in the player. Half/double BPM buttons for the classic 87-vs-174
error. Auto-analyses to create a grid if none exists.

*decks status* — **read-only.** The ANLZ PQTZ beat-grid parser exists in
`crates/rekordbox-db/src/anlz.rs` and drives intro-cue staging, but nothing writes grids.

*Epic* — **2**.

---

## Beat jump

*What it does* — Jump forward/backward by a beat count to scan a track fast. Defaults to
`Ctrl+←` / `Ctrl+→` for 16 beats. Designed to be hotkey-driven, and explicitly useful *without
looking at the waveform* — which is why it is also exposed as a global hotkey.

*decks status* — **missing.**

*Epic* — **2**.

---

## Hotkeys

*What it does* — Hotkeys for nearly every action, all rebindable. Defaults include `Space`
play/pause, `Z` previous, `X` next, `C` cue, `Q` quantize, `T` tag popup.

*Global hotkeys* — any hotkey can be promoted to system-wide so it works while Lexicon is in the
background. The manual notes the obvious tradeoff: a global hotkey stops doing its normal job in
other apps, so pick modifier combinations.

*Discoverability* — a `Show keyboard shortcuts` setting surfaces the binding inline wherever an
action appears.

*decks status* — **partial.** A shared `useKeyboardShortcuts` hook exists with spacebar
play/pause and a `T` tag-picker binding, plus button/link/`role=button` exclusions. No rebinding,
no global hotkeys, no inline hint display.

*Epic* — **2**.

---

## Cue Point Generator

The flagship. Right-click → Track tools → Generate Cue Points.

*What it does* — Machine-learning detection of **drops, breakdowns, and fade-out**, then places
cues relative to those anchors according to a user-defined template, with names and colours.

### Preconditions

- **Tracks must have a beatgrid**; Lexicon analyses automatically if missing.
- **A wrong BPM wrecks detection.** The manual calls out Rekordbox's habit of analysing 174 BPM
  tracks as 87 — half-time grids materially degrade drop detection.
- **Genre is an input.** Lexicon feeds the Genre field into the model because audio structure
  differs by genre. Not required, but improves results.

### Honest accuracy posture

Lexicon is refreshingly blunt, and we should match this tone rather than overclaim:

- Accuracy depends heavily on genre. Techno, House and Drum & Bass do well; Reggae and Dancehall
  do badly. Tracks with bass throughout are hardest.
- It is **"not a total replacement for manual cue points."**
- Documented failure modes: drop offset by exactly 1–2 bars (producers inserting odd bar counts —
  Lexicon quantises anchors on a 4-bar assumption and cannot detect this); drop not found on
  low-energy tracks; no second drop found when the first breakdown was missed.

### Cue point template

Declarative placement relative to detected anchors — "a cue 64 beats before the drop". Each entry
carries name, colour, and enabled state. Order in the template is the order cues are written.
A cue can depend on an anchor even when that anchor's own cue is disabled.

*Overflow handling* — if the template yields more cues than the target app supports, Lexicon
"intelligently removes the least interesting ones first". Recommends staying under 10.

*Rekordbox-specific* — can emit memory cues instead of hot cues when that app-specific option is
on. Note the constraint: **Rekordbox refuses two memory cues at the same position**, so if fade-out
and second breakdown coincide, one silently disappears.

### Custom cue anchors

*What it does* — Skips detection entirely. Instead of running the model, point the generator at
cues you already placed and declare which is the Drop, the Breakdown, and so on. The template is
then applied to those anchors.

*Matching rules*, precisely:

- Name **and** colour supplied → both must match exactly.
- Name only → first cue with that name, any colour.
- Colour only → first cue with that colour, any name.

A hotkey applies the last-used template to the selection, but only when
`Enable custom cue anchors` is on for that template.

### Generator settings

| Setting | Options / behaviour |
|---|---|
| Start cue behavior | `At first beat` · `At existing cue` (falls back to first beat) · `At zero` |
| Breakdown min. beats | Minimum length to count as a breakdown. Default 64; 32 suits some genres. Lowering it often unlocks second-drop and second-breakdown detection. |
| Drop at start | `Never` (ignore, keep looking) · `High energy only` (accept an early drop only if high energy — skips intros; suits techno) |
| Keep cue position | Cues land at their template slot index rather than being packed, so "drop is always cue 1" or "emergency loop is always cue 8" holds. Unchecking a custom cue leaves an empty slot. |
| Emergency loop | Generator finds a spot for an active loop of a requested length, choosing a point where the loop end connects musically back to its start. |
| Auto analyze | Any loaded track without cues automatically gets the last-used template applied. |

### End / fade-out detection

Uses **only low frequencies** to find the fade-out, which means the marker lands *before* quiet
outros — deliberately, since you want to mix out before that point.

*decks status* — **missing.** The nearest thing is `library_stage_intro_cues`, which reads the ANLZ
beat grid to place a 1.1 downbeat memory cue plus a 4-bar loop. That is beatgrid arithmetic, not
structural analysis.

*Epic* — **3**.

### Implementation notes for Epic 3

- The anchors needed are drop, breakdown (with an ordinal — first/second), and fade-out. Everything
  else in the template is arithmetic relative to those, in beats, resolved through the beatgrid.
- `crates/stratum-dsp` already computes a novelty curve and chroma. Foote self-similarity over
  beat-synchronous features plus novelty peak-picking is the classical route to segment boundaries
  and needs no new non-free dependency. Energy contrast across segments distinguishes drop from
  breakdown.
- Build **custom cue anchors first.** It is pure matching logic with no ML, delivers the whole
  template system on its own, and gives us a way to evaluate generated anchors against
  human-placed ones.
- Carry the honest-labelling posture from ADR-0008 (the synthetic-waveform precedent): ship
  detection with visible confidence and never present a guess as fact.
