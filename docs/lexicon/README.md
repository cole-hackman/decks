# Lexicon Reference

This directory is the reference specification for the Lexicon DJ parity initiative: a
domain-by-domain description of what Lexicon does, paired with an honest assessment of where
`decks` stands against it.

It supersedes `docs/lexiconparityfeatures.md`, which specced the first seven parity features and is
retained for history.

## How to use this

- **[`PARITY.md`](PARITY.md)** — the feature matrix. Every Lexicon capability, its `decks` status,
  and the epic that owns it. Start here.
- **[`../ROADMAP.md`](../ROADMAP.md)** — the epic queue and acceptance criteria.
- The numbered files below — the behavioural spec for each domain. Cite these from commit messages
  and PR bodies (`per docs/lexicon/03-smartlists.md §Rules`).

| File | Domain |
|---|---|
| [`00-overview.md`](00-overview.md) | Product shape, platform, plan gating, settings surface |
| [`01-interop.md`](01-interop.md) | Import, sync, field mappings, key conversion, conversion limits |
| [`02-library.md`](02-library.md) | Track browser, playlists, playlist tools, editing, tags, timeline |
| [`03-smartlists.md`](03-smartlists.md) | Smartlist rules engine and generator |
| [`04-analysis.md`](04-analysis.md) | Analyzer, key/BPM/energy, mixable tracks, beatshift |
| [`05-cues-player.md`](05-cues-player.md) | Player, cues, loops, beatgrid, Cue Point Generator |
| [`06-files.md`](06-files.md) | Watch folder, move/rename patterns, write tags, path mappings |
| [`07-health.md`](07-health.md) | Duplicates, lost/broken tracks, tags & album art, unused files |
| [`08-streaming.md`](08-streaming.md) | Streaming services, charts, store links, track matcher |
| [`09-history-backup.md`](09-history-backup.md) | History, database backup, cloud storage |
| [`10-recipes.md`](10-recipes.md) | The Recipes bulk-transformation system and CSV tag import |
| [`11-extensibility.md`](11-extensibility.md) | Plugins, Stream Deck, export/send-to |
| [`GAPS.md`](GAPS.md) | Anything still unverified |

## Entry shape

Every feature entry follows the same shape so `PARITY.md` stays derivable from these files:

> **Feature name**
> *What it does* — behaviour, in our own words.
> *UI surface* — where it lives.
> *Data model* — what it implies for storage.
> *decks status* — `done` / `partial` / `missing`, with the specific gap named.
> *Epic* — which epic owns closing the gap.

## Sourcing and copyright

Written from the official Lexicon manual (95 sections), supplied by the project owner, plus eight
Lexicon UI screenshots in [`../lexicon_feats/`](../lexicon_feats/).

**The manual itself is not committed.** It is Lexicon's copyrighted documentation and this
repository is public and GPL-3.0. `docs/lexicon/source/` is gitignored; drop the manual there in a
fresh checkout if you need to re-derive a section. These spec files deliberately describe
*behaviour* in our own words rather than reproducing the manual's prose — the same clean-room
posture ADR-0012 applies to reading Mixxx.

Nothing here is a claim that Lexicon's implementation is being copied. It is a description of a
product's observable feature set, used to set our own requirements.
