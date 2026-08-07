# 01 — Interoperability: Import, Sync, Field Mappings

Lexicon's core value proposition. Per the scope decision in `docs/ROADMAP.md`, `decks` targets
**Rekordbox only** for now, so this file records the full picture but marks non-Rekordbox work as
deferred.

---

## Supported apps

| App | Import | Sync | Mechanism |
|---|---|---|---|
| Rekordbox 6 / 7 | ✅ | ✅ | Direct database |
| Rekordbox 5 | ✅ | ✅ | XML only |
| Serato DJ Pro / Lite | ✅ | ✅ | Direct database |
| Traktor Pro 3 / 4 | ✅ | ✅ | Direct database |
| VirtualDJ | ✅ | ✅ | Direct database |
| Engine DJ | ✅ | ✅ | Desktop DB or direct-to-hardware/USB |
| djay Pro | ✅ | ✅ | Dedicated sync page |
| Apple Music / iTunes | ✅ (XML) | ✅ | XML |
| Pioneer USB | — | ✅ | OneLibrary + legacy DeviceLibrary |
| CDJ/XDJ over network | — | ✅ | DIRECT2CDJ via Pro DJ Link |
| M3U/M3U8 | ✅ | ✅ (export) | Playlist files |

*decks status* — Rekordbox 6/7 direct (read + gated write) and Rekordbox XML emit. Everything else
**deferred past this initiative**.

---

## Sync modes

| Mode | Behaviour |
|---|---|
| **Full Sync** | The DJ app becomes a mirror of Lexicon. **Anything not in Lexicon is removed from the app.** |
| **Playlist Sync** | Additive/update only. Nothing is removed; the existing app library is preserved. |
| **Modified Sync** | Like Playlist Sync, but Lexicon auto-selects what changed since the last sync. Unlocked only after a first Full or Playlist sync, and **tracked per DJ app**. |

*The Rekordbox XML exception* — XML import in Rekordbox never deletes anything, so a Full Sync over
XML cannot actually mirror. Worth stating in our UI, since XML is `decks`'s default egress.

*decks status* — **partial.** `SyncPanel` has mode plumbing and a staged diff, but no per-app
modified-sync watermark and no delete semantics for Full Sync.

*Epic* — deferred / **6**.

---

## Sync options

| Option | Behaviour | decks status |
|---|---|---|
| **Cue Destination** | See below | partial — governs `Kind` on newly inserted cue rows only |
| **Key Conversion** | Original / Open Key / musical notation | done (`key_format.rs`), plus Camelot |
| **Don't Touch My Grids** | Never modify existing beatgrids in the app. **New tracks still receive Lexicon's grid.** | partial — only skips BPM `TrackMetadataEdit`; no grid writes exist to skip |
| **Colors → nearest** | Map Lexicon's larger palette to the app's nearest supported colour. **Off means no colour is written when there's no exact match** | **plumbed but ignored** |
| **Field Mappings** | See below | missing |
| **All smartlists to playlists** | Materialise smartlists | **plumbed but ignored** |

### Cue Destination — the Rekordbox memory-cue problem

Rekordbox is the only app with memory cues; Lexicon's internal model has hot cues only. The
round-trip is carefully designed and worth copying exactly:

- **On import**, `Both` merges memory cues into hot cues. Duplicates from the merge are **hidden,
  not deleted**.
- **On sync back**, choosing `Default` restores those hidden memory cues to their original
  positions. Cues *created* in Lexicon go back as hot cues only.
- Import also offers `Only hot cues` / `Only memory cues`.
- Sync additionally offers `All to hot cue`, `All to memory cue`, `All to hot and memory cue` —
  which is how you copy hot cues into memory cues wholesale.
- A per-cue `M` toggle (behind a DJ-app-specific setting) marks individual Lexicon cues as
  destined to become memory cues — but only honoured when Cue Destination is `Default`.

*decks status* — **partial.** `CueDestination` (memory/hot/both) exists in
`changes::applier::SyncOptions` and selects the `Kind` on inserted cues. There is no
hidden-duplicate model, so the round-trip guarantee does not hold.

*Epic* — **2**.

---

## Excluded From Sync

*What it does* — Two opt-out mechanisms, both requiring no new UI:

- Any playlist, smartlist or folder **whose name starts with `Excluded From Sync`** is skipped.
- A Custom Tag named **`Excluded From Sync`** excludes individual tracks.

Convention over configuration. Cheap to implement, easy to explain.

*decks status* — **missing.**

*Epic* — **1** (rides along with smartlists and tags).

---

## Field Mappings

*What it does* — Projects Lexicon-only fields (Energy, Danceability, Popularity, Happiness, Custom
Tags) into fields that actually exist in the target. Configured **per DJ app**, and separately for
ID3 tag writing.

*Semantics*

- A mapping is source → target. `Energy → Comment` yields `"Energy 08"`.
- **Overwrite** on replaces the target; off **appends** to the existing value.
- **Combining** — map several sources to the same target and results join with `, `:
  `"Energy 08, Pop 05"`. Unlimited sources per target.
- `All Custom Tags → Comment` writes the hashtag form `#Techno #Vocals`; a single tag category can
  be the source instead.
- Engine DJ has no track colours, so a colour → text mapping writes the colour *name*
  (`Red_Dark`).

*decks status* — **partial.** `crates/changes::field_mappings` implements the projection engine —
source → target, overwrite-replaces / off-appends, several sources combining with `, `, hashtag
form for custom tags, colour by name. Cache migration v11 stores rules per *profile*, and
`FieldMappingsSection` (Settings) configures the ID3 profile, which Write Tags honours.

Three rules the manual leaves open, decided and tested: a track with no value for a source
contributes nothing (not `Energy` with no number, and not a blanked target); numbers are zero-padded
to two digits so a text target sorts them correctly — the same reason Key Conversion has a
leading-zero option; and where several mappings share a target, the **first** decides
overwrite-vs-append, since mixing the two on one target is a configuration mistake and first-wins is
predictable.

For tag writing, mappings only apply to targets the per-field selection did not claim — a mapping
quietly replacing a field the user explicitly ticked would be a nasty surprise — and a mapping onto
a field audio files do not have produces a warning rather than vanishing.

Migration v11 also **drops the dead `field_mappings` table from v5**: nothing ever read or wrote it,
and its `(library_path, source_field)` primary key allowed one target per source, which cannot
express combining.

Missing: per-DJ-app profiles (only ID3 is configurable, though the schema is ready) and applying
mappings during sync.

*Epic* — **4** (shares machinery with Write Tags).

---

## Key Conversion

Convert all keys to **Open Key** or musical notation, or leave them original. Parses Camelot,
Rekordbox, Traktor, Serato, VirtualDJ and musical-note spellings. An **add leading zero** option
exists purely so DJ apps sort correctly (`01A, 02A … 10A` rather than `1A, 10A, 11A, 2A`).

> **Camelot is unavailable in Lexicon by design** — "due to licensing restrictions". Open Key is
> the same wheel with different letters and no encumbrance. See the action item in
> [`04-analysis.md`](04-analysis.md#key-detection); `decks` currently leans on Camelot everywhere.

*decks status* — **done and then some.** `crates/changes/src/key_format.rs` converts to both
Camelot and Open Key with a 24-key table plus enharmonics and parse-failure passthrough. Missing:
the leading-zero option, and the notation posture.

*Epic* — **6**.

---

## Field compatibility matrix (Lexicon → Rekordbox)

Rekordbox-relevant subset. Full matrix in the manual.

*Rekordbox 6/7 supports but Lexicon does not model:* Album Artist, Original Artist, Disc Number,
Release Date, Date Created.

*Lexicon has but Rekordbox cannot store:* **Energy, Danceability, Popularity, Happiness, Last
Played, Extra 1, Extra 2, Grouping, Producer.** These reach Rekordbox only through Field Mappings.

*Custom Tags* map to Rekordbox MyTags — **limited to 4 categories**, and smartlist tag rules to
**2 rules**.

---

## Conversion limitations (Rekordbox-relevant)

- **Rekordbox does not store last-played date.**
- **Rekordbox rejects two memory cues at the same position** — silently drops one. Relevant to the
  Cue Point Generator when fade-out and second breakdown coincide.
- **Rekordbox does not support album art on WAV files.**
- Rekordbox 5 / XML: no intelligent playlists, no memory-cue colours (Rekordbox doesn't write them
  to XML), documented XML import quirks.
- Traktor has no cue colours; Engine DJ has no track colours.

---

## Beatshift correction

Runs **automatically on every import and sync**. See
[`04-analysis.md`](04-analysis.md#beatshift). *decks status* — **missing**, and this is a
correctness issue given `decks` already writes cues.
