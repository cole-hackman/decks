# 00 — Product Overview

---

## What Lexicon is

A DJ **library manager** that sits between the DJ and their DJ apps. Import from any supported app,
organise and analyse in Lexicon, sync back out to one or many apps. Explicitly *not* a performance
app — though it has a full music player, that player exists to edit cues and audition, not to
perform.

Positioned as the successor to Rekord Buddy.

## Platform

Windows and macOS. **No Linux build.** A companion mobile app (iOS + Android) exists but requires
the desktop app; it does player, playlists, waveforms, cue creation/editing, loops, beatgrid
editing, beat jump, quantize, track editing and custom tags.

Fourteen UI languages (AI-generated translations, per the manual). Dark / Light / System themes.

User data lives under `Documents/Lexicon` — plugins, reports, and backups all land there. Worth
copying as a convention: every destructive utility writes a text report to a known folder.

## Plan tiers

| Tier | Price | Scope |
|---|---|---|
| Free | $0 | **Full library conversion between all apps**, including Field Mappings. Browse, create playlists. |
| Essential | $9.99/mo or $199 lifetime | Editing and quick cleanups. 1,000 cloud storage tracks. |
| Ultimate | $19.99/mo or $399 lifetime | Everything, including future features. Cloud Database Backup, Local Path Mappings. 10,000 cloud tracks. |
| Cloud Storage add-on | separate | Practically unlimited music upload, attachable to any plan including Free. Files < 200 MB. |

Known gating: Cue Point Generator and fingerprint duplicate detection are Ultimate; the
`Show Hotkeys` setting requires a subscription. On expiry the user keeps browsing and playlist
creation but **loses the ability to sync to a DJ app**.

*Relevance to `decks`* — none directly (no commercial model), but the tier split is a good signal
of which features Lexicon considers load-bearing: conversion is the free acquisition hook, and
*editing* is what people pay for.

---

## Global navigation surfaces

### Action Center

*What it does* — A command palette. `Cmd/Ctrl+Space`. Executes **any command that can be bound to a
hotkey** and navigates to any screen. Arrow keys navigate, `Enter` runs; `Enter` with nothing
highlighted runs the first result.

The design rule worth stealing: *hotkey-bindable and command-palette-reachable are the same list*.
One action registry drives both.

*decks status* — **missing.** `CLAUDE_CODE_PROMPT.md` §7 already calls for `Cmd+K`.

*Epic* — **2**.

### Find Popup

*What it does* — `Cmd/Ctrl+F`. Instant search across playlists, smartlists **and** tracks in one
box, with per-result actions: add selected tracks to that playlist, add to the play queue, or play.
`Enter` plays.

*decks status* — **done.** `Cmd/Ctrl+F` opens `FindPopup` over playlists, smartlists and tracks in
one box. `lib/find.ts` holds the ranking; per-result actions are `Enter` (open a container, play a
track), `Queue` on a track, and "add the current selection" on a playlist.

Deliberately **not** merged into the Action Center. `Cmd+K` searches *commands*, this searches
*content*; one box over both would have to rank a track title against "Toggle Sidepanel", which is
a comparison with no sensible answer.

Four decisions:

- **Three match tiers, not fuzzy matching** — exact, prefix, word-start, substring. Fuzzy
  subsequence matching is right for a palette of a hundred short fixed strings and wrong for four
  thousand track titles, where it matches almost everything and ranks by noise. `rain` finds
  "Acid Rain" ahead of "Braindance".
- **Each section is capped independently.** A library of thousands must not bury the one playlist
  that matched, and containers sort before tracks for the same reason.
- **Ties break alphabetically.** Without it, equal-scoring results come back in library order, so
  the same query returns a different top result after any re-sort and `Enter` plays something else.
- **An empty query returns nothing.** A Find popup that opens showing the whole library has
  answered a question nobody asked.

Folders are excluded: they cannot be played and cannot hold tracks, so every action on such a row
would fail. Row actions are visible on the **highlighted** row as well as on hover — this popup is
driven by the keyboard, and a hover-only action is one the keyboard cannot reach.

*Epic* — **2**.

### Sidepanel

*What it does* — A **second track browser**, opened from View → Toggle Sidepanel, so two playlists
can be viewed side by side. Pitched for set building, and repeatedly cited as the thing that makes
Genre/Artist Cleanup's alt-click-to-filter useful.

*decks status* — **done.** A resizable second browser on the right, toggled from the header or by
`Cmd/Ctrl+\` — registered in the action registry, so it is rebindable like everything else.

**It keeps its own selection**, deliberately. The point is comparing two playlists; a shared
selection would make it a mirror rather than a second view. Available from any view, not just the
playlist browser, since the reason to open it is usually something in the main pane.

*Epic* — **6**.

---

## Automatic Actions (settings)

A single settings group that captures Lexicon's automation philosophy. Every one of these is a
background behaviour the user opts into once:

| Setting | Behaviour |
|---|---|
| Auto Analyze New Tracks | Analyse on drag-in or Watch Folder. **Never** for DJ-app imports. |
| Auto Generate Cues on Play | Apply the current generator template to any played track lacking cues. Ignored when Custom Cue Anchors are on. |
| Auto Re-encode New MP3/MP4/M4A | Run the Beatshift Fixer on arrival, before any cues exist. Not for DJ-app imports. |
| Auto Write File (ID3) Tags | Write tags whenever a change is detected, honouring field mappings. **Skipped for bulk edits over 1,000 tracks** — those need a manual run. |
| Automatically Find Custom Tags for New Tracks | Look up genre custom tags for new arrivals. Adds tags only; **never touches the genre field**. |

The recurring pattern — *automation applies to tracks the user brought in, never to tracks imported
from a DJ app* — is a good default and should carry into `decks`.

*decks status* — **partial.** The settings group exists (`AutomaticActionsSection`, Settings), and
**Auto Analyze New Tracks** works: importing a watch-folder arrival detects BPM and key on the way
in, and analysis failing does not undo an import that already succeeded.

The other four are surfaced as **disabled toggles that state what they need**, rather than hidden or
offered as switches that quietly do nothing:

| Setting | Blocked on |
|---|---|
| Auto Generate Cues on Play | Automatic drop detection. Every anchor today comes from a cue the user placed, so there is nothing to generate from on a track with no cues — the setting would be a permanent no-op. |
| Auto Re-encode New MP3/MP4/M4A | The Beatshift Fixer, not built. |
| Auto Write File (ID3) Tags | Field mappings, so Lexicon-only fields project into real tag fields. Write Tags in the Files view does this manually today. |
| Automatically Find Custom Tags | The enrichment providers, not wired up. |

An unavailable action also reads as *off* at the point of use regardless of what is stored, so a
setting enabled before its feature regressed cannot silently take effect.

*Epic* — **4**.

---

## Other settings groups

`Appearance` (theme, language, show tips, show hotkeys) · `General` (minimize to tray on Windows,
date format) · `Music Player` · `File Management` · `Backup & Cloud Storage` · `Accessibility` ·
`Advanced` · `Other` · `Reset Settings`.

*decks status* — **partial.** `SettingsPanel` covers theme, library path, Anthropic key in the OS
keychain, and Claude Code detection. No language, no grouped structure, no reset.

*Epic* — **4**.
