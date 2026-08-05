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

*decks status* — **missing.** There is a global search input but no unified popup and no per-result
actions.

*Epic* — **2**.

### Sidepanel

*What it does* — A **second track browser**, opened from View → Toggle Sidepanel, so two playlists
can be viewed side by side. Pitched for set building, and repeatedly cited as the thing that makes
Genre/Artist Cleanup's alt-click-to-filter useful.

*decks status* — **missing.**

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

*decks status* — **missing**, all five.

*Epic* — **4**.

---

## Other settings groups

`Appearance` (theme, language, show tips, show hotkeys) · `General` (minimize to tray on Windows,
date format) · `Music Player` · `File Management` · `Backup & Cloud Storage` · `Accessibility` ·
`Advanced` · `Other` · `Reset Settings`.

*decks status* — **partial.** `SettingsPanel` covers theme, library path, Anthropic key in the OS
keychain, and Claude Code detection. No language, no grouped structure, no reset.

*Epic* — **4**.
