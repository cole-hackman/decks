# 03 — Smartlists

The rules-driven dynamic playlist system. Owned by **Epic 1**, the first build epic.

Lexicon calls them Smartlists; the same idea appears elsewhere as Intelligent Playlists, Smart
Crates, or Filter Folders.

---

## Smartlists

*What it does* — A playlist whose membership is computed from rules rather than stored as a track
list. Tracks entering the library join automatically when they match. Created the same way as a
normal playlist: right-click any playlist or folder and add a smartlist there, so smartlists live
inside the ordinary playlist tree rather than a separate space.

*Rules model*

- Unlimited rules per smartlist.
- A top-level combinator: **Any Rule** (union) or **All Rules** (intersection).
- **OR clauses** link individual rules into a group. Critically, OR grouping is only available when
  the smartlist is set to *All Rules* — that is the only way to express
  `(Genre = House OR Genre = Techno) AND (Rating = 3)`. This is a two-level structure, not
  arbitrary nesting: an AND of clauses, where each clause is an OR of rules.
- **Archived tracks are excluded by default.** They only appear if a rule explicitly asks for them.

*Field vocabulary* — the rule fields are the Lexicon track fields, enumerated in
[`01-interop.md`](01-interop.md#dj-app-field-compatibility). Notably this includes the four
Lexicon-only analysis fields (Energy, Danceability, Popularity, Happiness) and Custom Tags, none of
which exist in any DJ app.

*Operators* — the manual specifies the operator set precisely in the context of track-browser
search, and the same vocabulary governs rules:

| Type | Operators |
|---|---|
| Text | contains, equals, `None` (field is empty) |
| Number | `>`, `<`, `>=`, `<=`, `a-b` (between), `!` (not) |
| Date | `YYYY-MM-DD` comparison, with `>` and `<` |
| Key | equality with **automatic notation conversion** — searching `4M` matches `Am` |
| Custom Tags | `Has all these tags` / `Has none of these tags`; full-label match only, never partial |

*Performance contract* — smartlist contents recompute when the smartlist is selected, but at most
**once every 30 seconds**. A newly added track can therefore take up to 30s to appear. The UI shows
a loading state when a recompute happens. This is an explicit, documented tradeoff: switching
between smartlists stays instant on large libraries.

*Import* — smartlists import from DJ apps automatically, but **only during a Full Import**. A
smartlist has no stored track list, so a partial import has no way to determine membership.

*Degradation on sync* — when a target app cannot express a rule, the smartlist is materialised as a
**normal playlist** containing the currently matching tracks. The user keeps the right tracks even
though the app loses the rule. Known limits:

- **Rekordbox 5** has no intelligent playlists over XML at all — always materialised.
- **Rekordbox 6/7** supports only **two** MyTag smartlist rules.
- Lexicon tag rules map onto Rekordbox 6/7 MyTag rules, and only two of them survive:
  `Has all these tags` → `contains`, `Has none of these tags` → `does not contain`.

*UI surface* — smartlist editor with a per-DJ-app **compatibility indicator** showing at a glance
which targets can represent the current rule set natively and which will be flattened.

*Data model* — a smartlist is `(id, name, parent_folder, combinator, [clause])` where each clause is
an ordered list of rules OR'd together. Membership is never persisted; it is always derived.

*decks status* — **missing.** `decks` has a rich ad-hoc filter system
(`apps/desktop/src/lib/filters.ts`, `FilterDrawer`, `FilterChips`) with BPM/year ranges,
key/genre multi-select, tag filters with `tagMatchAll`, and per-library persistence in
`localStorage` — but filters are transient UI state, not first-class saved objects, have no
combinator or OR-clause structure, and never materialise into playlists.
`changes::applier::SyncOptions.all_smartlists_to_playlists` is accepted by the option struct and
then ignored.

*Epic* — **1**.

---

## Smartlist Generator

*What it does* — Bulk-creates smartlists in one pass, so a user with a large library gets a
navigable structure without hand-building dozens of rule sets. Generates from:

- any track field (one smartlist per distinct value — per genre, per label, per key…)
- any custom tag category (one smartlist per tag in that category)
- special generators: **Decade**, **BPM range**, **play count**

*Idempotency contract* — output lands in a reserved `Lexicon` playlist folder. Re-running is safe
and will not duplicate smartlists that are still in that folder. Moving a generated smartlist out
of the folder detaches it, and the next run regenerates it in place. This is a neat trick worth
copying exactly: the folder *is* the generation ledger, so no separate state is needed.

*UI surface* — top menu bar → Utility → Smartlist Generator.

*decks status* — **missing.**

*Epic* — **1**.

---

## Implementation notes for Epic 1

Things this spec implies that are easy to get wrong:

1. **The OR structure is two-level, not a general tree.** Model it as
   `All([Clause(Or([rule, rule])), Clause(Or([rule]))])` rather than a recursive AST. Any-mode is a
   flat union with no clause layer. Building a general boolean tree would be more code and would
   not match the product.
2. **Key equality must run through notation conversion**, reusing the existing
   `crates/changes/src/key_format.rs` table rather than comparing strings.
3. **Custom tag matching is exact-label only** — a deliberate performance decision, not an
   oversight. Match it.
4. **The 30-second recompute throttle is a feature.** Implement the cache and the loading state,
   not just the query.
5. **Archived-by-default exclusion** must be part of the evaluator, since `decks` already has an
   `archived_tracks` table in the cache DB.
6. Evaluation should compile to SQL against `master.db` where the field lives there, and fall back
   to in-memory filtering for cache-backed fields (energy, tags) — `decks` already has this split,
   via `hydrate_energy` and `list_track_tags_map`.
