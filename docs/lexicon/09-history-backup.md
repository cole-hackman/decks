# 09 — History, Backup, Cloud

---

## History

*What it does* — Every play session from the DJ app, in one place, reachable from the sidebar.
Functions as a gig log.

*Metadata* — each history set takes a **rating** and a **location**.

*The snapshot rule, which is the important design decision* — Lexicon stores a **snapshot** of
track data at play time, decoupled from the library. Editing a track later does **not** rewrite
history. History is a true historical record, not a view over current data.

*Using history* — a set can be saved as a regular playlist. Because the original tracks may since
have been deleted, Lexicon re-matches by filename and location with a priority system — which
depends on filenames not having changed. A set can also be copied wholesale to the clipboard.

*Import* — history imports automatically from any DJ app (except Engine DJ) on **any** import, full
or partial, and sets are never duplicated.

*Deleting* — sets can be deleted, individual tracks can be removed from a set, and **Lexicon
remembers deletions** so a re-import doesn't resurrect them. This matters because DJ apps log
practice sessions and false starts.

*decks status* — **missing.** `Track.dj_play_count` is read from Rekordbox but no session data is
touched.

*Epic* — **6**.

*Implementation note* — Rekordbox stores history in `djmdHistory` / `djmdSongHistory`. The snapshot
rule means our own tables, not a view over `master.db`, and the deleted-set ledger needs to persist
in the cache DB so re-imports stay idempotent.

---

## Database Backup

*What it does* — `Backups → Database Backup → Create backup`. Produces a **ZIP** of tags (artist,
title, cue points, …), playlists and settings. **Does not include music files.**

*Restore* — from the ZIP. **Restoring completely deletes the existing database** — flagged loudly
in the manual, and our UI should be at least as blunt.

*Retention* — default location `Documents/Lexicon/Backups`. **Lexicon auto-deletes backups older
than one month**; anything worth keeping must be moved elsewhere. Sensible, and users need to be
told.

*decks status* — **done**, at **Settings → Database Backup**. `WriteGuard`'s timestamped
`master.db` copy protects the *source*; this protects everything `decks` knows that Rekordbox does
not — custom tags, the archive, smartlists, staged changes, path mappings, watch folders,
conversations — which lived only in the cache DB.

Divergences, all deliberate:

- **A JSON document, not a ZIP.** Lexicon ZIPs because it bundles several files; this is one
  document, and compressing a few hundred kilobytes of text buys nothing a user can feel. What it
  buys instead is worth more: the backup is *inspectable*, and it survives schema changes.
  Restoring a copied SQLite file into a newer schema is a gamble; restoring named columns is not —
  unknown columns are dropped and **named in the report**, and a table missing from the backup is
  left alone rather than emptied.
- **Analysis caches are excluded.** Waveform peaks, fingerprints and audio features derive from
  files still on disk. Including them would multiply the backup's size to save CPU the user spends
  once.
- **Nothing is auto-deleted.** Lexicon removes its own backups after a month; `decks` saves where
  the user chose and leaves them there. A tool that deletes the user's backups on a timer is doing
  something they did not ask for, and the retention note says so plainly.
- **A backup from a newer build is refused**, not partially applied.

Restoring **replaces**, per the spec, in a single transaction — a failure part-way leaves the cache
as it was rather than half-swapped. The file is inspected first and its contents shown *in the
confirmation*, so the user sees what they are swapping in rather than only what they are losing.
A file that is not a backup is caught on read, before anything is deleted.

Table names on restore are checked against a fixed allowlist before they reach any SQL, with a test
covering the case.

*Epic* — **5**.

---

## DJ App Backups

*What it does* — Lexicon **automatically creates a backup before writing to any DJ app database**,
stored in a `Lexicon` folder inside that app's own database folder.

*decks status* — **done.** `WriteGuard` takes a timestamped `master.db` backup before the first
write of a session, and refuses to write at all while Rekordbox holds the WAL lock — which is
stricter than Lexicon. See ADR-0010.

---

## Cloud Database Backup *(Ultimate)*

Same ZIP, stored in Lexicon's cloud, restorable after logging in on a different computer. Weekly
reminder on Ultimate. **Limit of 10 backups.** Pairs with Local Path Mappings for the documented
two-computer workflow.

*decks status* — **missing, and deferred past this initiative.** `decks` is local-first with no
account system and no telemetry; a hosted backup service is a product decision, not a feature gap.

---

## Cloud Storage *(add-on)*

Uploads the actual **music files**, which then become streamable or downloadable on demand — a
track you don't have locally still plays. Limits: 1,000 tracks on Essential, 10,000 on Ultimate,
unlimited with the add-on; fair use, < 200 MB per file. Every transfer writes a report into
`Documents/Lexicon`.

*decks status* — **missing, and deferred past this initiative**, for the same reason.
