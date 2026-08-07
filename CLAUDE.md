# decks

Local-first AI DJ assistant for Rekordbox 7. Tauri 2 + React frontend over a Rust workspace that
reads the Rekordbox SQLCipher `master.db`, stages library changes, and writes back via XML export
or an opt-in guarded Sync.

**Read these before doing anything else:**

1. [`docs/CLAUDE_CODE_PROMPT.md`](docs/CLAUDE_CODE_PROMPT.md) — the operating contract. Principles,
   session workflow, definition of done, locked tech stack.
2. [`docs/STATUS.md`](docs/STATUS.md) — current phase, current task, blockers.
3. [`docs/ROADMAP.md`](docs/ROADMAP.md) — the Lexicon parity epic queue.
4. Last few entries of [`docs/JOURNAL.md`](docs/JOURNAL.md) — what past sessions were thinking.

## Current initiative

Feature parity with [Lexicon DJ](https://www.lexicondj.com), Rekordbox-first.

- [`docs/lexicon/`](docs/lexicon/) — the reference specification, by domain. Cite it in commits
  and PRs (`per docs/lexicon/03-smartlists.md §Rules`).
- [`docs/lexicon/PARITY.md`](docs/lexicon/PARITY.md) — every feature, its status, its owning epic.
- One epic per branch (`claude/lexicon-<epic>`), one draft PR each, reviewed before the next.

## Definition of done

A change is not done until all of these pass:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm test && pnpm typecheck && pnpm lint
pnpm e2e
```

And: the feature is reachable from the UI (never tests-only), `docs/STATUS.md` and
`docs/JOURNAL.md` are updated **in the same commit** as the code, and any behaviour change is
reflected in the relevant `docs/*.md`.

## Non-negotiables

- **`master.db` is read-only by default.** The single exception is the opt-in Sync flow, gated by
  `WriteGuard` — which refuses to run while Rekordbox holds the WAL lock and takes a timestamped
  backup before the first write of a session. See ADR-0010.
- **No telemetry, no analytics, no remote logging.** The library never leaves the machine except
  through enrichment APIs the user explicitly enables, and those go through a local cache first.
- **No stub logic in production paths.** Stub UI behind a feature flag is fine; logic that returns
  fake data is not.
- **API keys live in the OS keychain**, never in plaintext config.
- **New capability goes through `crates/agent-tools::ToolRequest`** where it makes sense, so the
  chat panel, the MCP server and the CLI all gain it from one implementation.

## Layout

```
apps/desktop     Tauri 2 shell — src/ (React) + src-tauri/ (Rust IPC, ~98 commands)
apps/cli         `decks mcp` | `mcp-http` | `tools call`
crates/          one crate per bounded concern; see README.md
docs/            architecture, data-model, tools, ADRs, Lexicon spec, journal
fixtures/        synthetic test libraries (real fixtures are gitignored)
```

## Licence

GPL-3.0-or-later (ADR-0011). Vendored reklawdbox code was MIT — compatible, attribution preserved
in `NOTICE`. Do not add dependencies with non-free model weights; see ADR-0012 for the adopted
analysis stack and what is deliberately excluded.

`docs/lexicon/source/` is gitignored — it holds Lexicon's copyrighted manual for reference and must
never be committed.
