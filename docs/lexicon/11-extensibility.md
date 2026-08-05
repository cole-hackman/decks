# 11 — Extensibility

Deferred past this initiative, but recorded because it shapes decisions we make earlier.

---

## Plugins

*What it does* — Third-party plugins written in **JavaScript** that can modify tracks and playlists
"in almost any way", and can control and automate parts of Lexicon.

*Distribution* — no registry. A plugin is a **ZIP dropped into `Documents/Lexicon/Plugins`**,
auto-detected on restart, then run from the top menu bar → Plugins. Community distribution happens
on their forum and Discord.

*Integration* — every plugin action can be bound to a hotkey in settings, so plugins are
first-class citizens of the same action registry that drives the Action Center. There is a
documented Local API.

*decks status* — **missing.** `crates/plugins` is a 9-line placeholder. `CLAUDE_CODE_PROMPT.md` §4
specifies a Deno-sandboxed host with a manifest declaring allowed tool calls — a stronger security
model than Lexicon's, which appears to grant plugins broad access.

*Epic* — deferred.

*Note for earlier epics* — the thing to preserve now is that **`crates/agent-tools` is already the
shared tool service** backing the chat panel, the MCP server and the CLI. A plugin host is a fourth
consumer of that same surface, not a new subsystem. Keep new capability behind `ToolRequest` rather
than reaching into crates directly, and the plugin epic stays cheap.

---

## Stream Deck

*What it does* — Documented Elgato Stream Deck integration, riding on the same hotkey/action layer.

*decks status* — **missing.** *Epic* — deferred.

---

## Hotkeys as the universal substrate

Pulling the thread across this file, [`00-overview.md`](00-overview.md) and
[`05-cues-player.md`](05-cues-player.md), Lexicon has one idea implemented once and reused
everywhere:

> Every action is a named, bindable command. The Action Center searches that list. Hotkeys bind to
> it. Global hotkeys promote a binding system-wide. Plugins register into it. Stream Deck buttons
> trigger it. Favourite playlists and cue templates generate entries in it.

`decks` currently has ad-hoc handlers in a `useKeyboardShortcuts` hook. **The single highest-value
architectural move available in Epic 2 is to introduce a real action registry** — `id`, label,
handler, default binding, and whether the action is available in the current context — and drive
the existing shortcuts through it. Every later epic gets its hotkeys, palette entries and (much
later) plugin surface for free.
