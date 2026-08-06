# Architecture Decision Records

## ADR-0001 — Tauri v2 over Electron

**Date:** 2026-05-10
**Status:** Accepted

**Context:** We need a cross-platform desktop shell for a native-feeling app that bundles Rust backend code alongside a React/TypeScript frontend. The two main candidates are Electron (Chromium + Node.js, JS-only backend) and Tauri v2 (system WebView + Rust backend, with optional JS/TS sidecar processes).

**Decision:** Use Tauri v2.

**Reasons:**
1. Our core logic is Rust (SQLCipher access, audio analysis, beam-search sequencer, ONNX runtime). Tauri lets us call these crates directly as a Rust binary; Electron would force a FFI boundary or a subprocess for every native call.
2. Binary size: Tauri apps are ~5–10 MB vs. 80–150 MB for Electron because Tauri uses the OS WebView instead of bundling Chromium.
3. Memory footprint: a typical Tauri app uses 30–80 MB RSS vs. 200–400 MB for Electron, which matters when the user is also running Rekordbox and a DAW.
4. Security model: Tauri's allowlist + CSP surface area is smaller than Electron's Node.js integration.
5. Active maintenance: Tauri v2 is stable as of late 2024 with macOS, Windows, and Linux support.

**Trade-offs accepted:**
- System WebView differences (Safari on macOS, WebView2 on Windows, WebKitGTK on Linux) mean we must test on all three. Mitigation: CI matrix + explicit polyfills for any missing APIs.
- Tauri's plugin ecosystem is smaller than Electron's. Mitigation: most functionality we need is in Rust crates, not JS plugins.

## ADR-0002 — Keep MVP Agent Runtime on Anthropic API, Detect Claude Code Separately

**Date:** 2026-05-11
**Status:** Accepted

**Context:** The current chat implementation uses the Anthropic Messages API from the desktop frontend, authenticated by an Anthropic API key stored in the OS keychain. Users with Claude Pro/Max may also be signed in to Claude Code locally, but that subscription is not the same product surface as a generic third-party app API key. Claude Code can authenticate with a Claude.ai subscription for terminal-based Claude Code workflows.

**Decision:** For MVP, keep the in-app chat runtime on the existing Anthropic API-key path and add local Claude Code detection in Settings/error states. Do not claim Claude subscription support until a dedicated Claude Code runtime adapter is implemented and tested.

**Reasons:**
1. The current agent loop depends on Messages API tool calls and streaming behavior.
2. Claude Code subscription authentication is CLI-owned; treating it as a drop-in API key would be misleading and brittle.
3. Detecting Claude Code status gives users an accurate explanation without blocking current agent functionality.

**Follow-up:** Add a separate Claude Code runtime adapter if it can preserve tool execution, conversation persistence, and safe staged-change behavior without direct Rekordbox DB writes.

## ADR-0003 — MCP Server as the Subscription-Friendly Runtime Path

**Date:** 2026-05-11
**Status:** Accepted

**Context:** Claude Code can use a Claude subscription as the model host and call local tools through MCP. OpenAI and Gemini can also consume MCP through their supported host surfaces, though OpenAI API usage generally needs a reachable HTTP/remote MCP transport rather than local stdio.

**Decision:** Make Rekordagent's backend tools available through a provider-neutral MCP server. Keep the embedded Tauri chat on Anthropic API keys for now, while recommending Claude Code + `decks mcp` for subscription-backed Claude usage.

**Reasons:**
1. This matches the proven reklawdbox-style model: model host owns authentication/subscription, Rekordagent owns local tools.
2. It avoids pretending a Claude Pro subscription is an Anthropic API key.
3. A shared Rust tool service keeps MCP, CLI, and Tauri behavior aligned.
4. Stdio MCP is the fastest path for Claude Code and Gemini CLI; HTTP MCP can be added later for OpenAI remote MCP.

**Trade-offs accepted:**
- The in-app chat still needs an API key until it is replaced or backed by an external host workflow.
- MCP discovery uses host-safe underscore tool names while internal documentation may still mention dotted semantic names.
- XML export is not advertised over MCP until export logic moves into the shared tool service.

## ADR-0004 — Semantic CSS Token System Over Inline Tailwind Values

**Date:** 2026-05-11
**Status:** Accepted

**Context:** All colors in the initial codebase were hardcoded Tailwind utility classes spread across 8+ component files with an empty `tailwind.config.ts`. A palette or theme change required touching every component. The app needed a coherent design system that could be maintained without a global search-and-replace.

**Decision:** Define semantic CSS custom properties in `index.css` (e.g. `--bg-base`, `--text-ink`, `--accent`) using the space-separated RGB format (`R G B`) so Tailwind opacity modifiers work natively. Extend `tailwind.config.ts` with token names (`bg-base`, `text-ink`, `accent`, `edge`, etc.) that reference the CSS variables via `rgb(var(--name) / <alpha>)`.

**Reasons:**
1. A single `index.css` change swaps the entire app theme — no component churn.
2. Space-separated RGB format is required for Tailwind's `bg-X/50`-style opacity modifiers to work with custom properties.
3. Semantic names (`bg-elevated`, `text-ink-secondary`) communicate intent rather than raw color values, making per-component styling decisions easier to audit.

**Trade-offs accepted:**
- Component authors must use token names, not raw Tailwind colors, or the theme contract breaks. Enforced by convention, not tooling.
- shadcn drop-in components expect different token names (`bg-primary`, `text-foreground`, etc.). Resolved by adding a second alias layer in `tailwind.config.ts` and `index.css` mapping those names to our semantic tokens.

## ADR-0005 — "Precision Instrument" Design Aesthetic

**Date:** 2026-05-11
**Status:** Accepted

**Context:** The initial app used generic SaaS aesthetics: indigo accent, system UI font, generous padding. This reads as a web dashboard rather than a professional DJ tool. Users of this app are DJs who spend hours in Rekordbox, Serato, or on Pioneer CDJ hardware — they expect data density, not consumer-app comfort.

**Decision:** Commit to a "precision instrument" aesthetic modeled on Pioneer CDJ-3000 and Rekordbox desktop:
- **Background**: Near-true-black (`#0a0a0a`) base shell; `zinc-900`/`zinc-800` for surfaces.
- **Accent**: Amber/orange (`#f59e0b` family) instead of indigo. Indigo is generic SaaS; amber reads as hardware readout, edit/active state, record indicator.
- **Typography**: `Instrument Sans` for UI/labels; `IBM Plex Mono` for all data/numbers (BPM, key, duration, cue times, track IDs). IBM Plex Mono has the precise readout quality of CDJ displays at 10–12px.
- **Density**: 28px row heights in the track table; 10–12px data font. Generous spacing is a defect here, not a feature.
- **Hot-cue palette** (red/orange/yellow/green/cyan/blue/violet/pink) as a design anchor — the same hues reused for status badges and indicators to feel intentional.

**Trade-offs accepted:**
- Near-black backgrounds can feel harsh on low-brightness displays. Acceptable for a tool aimed at DJs in dark venues.
- Vendor-hosted Google Fonts add a network dependency. Mitigated with preconnect hints; no hard offline requirement exists.

## ADR-0006 — ElevenLabs UI Components via shadcn Registry Pattern

**Date:** 2026-05-11
**Status:** Accepted

**Context:** The initial chat UI was custom-built with basic divs and inconsistent styling. ElevenLabs open-sourced a set of React chat UI primitives (Message, Response, ShimmeringText, Conversation, Waveform) designed for AI voice/chat interfaces, distributed as a shadcn-style copy-paste registry rather than an npm package.

**Decision:** Copy ElevenLabs UI components directly into `src/components/ui/`, adapt import paths to our `@/*` alias, and integrate them into ChatPanel and TrackDetailPanel. Wire up the required infrastructure: `@/*` path alias in `tsconfig.json` + `vite.config.ts`, `src/lib/utils.ts` with `cn()`, shadcn color name aliases in `tailwind.config.ts` and `index.css`.

**Reasons:**
1. The components solve real UX problems (StickToBottom scroll, Streamdown markdown, ShimmeringText thinking state) without reinventing them.
2. Copy-paste ownership means we can modify or remove any component without a fork/patch cycle.
3. The shadcn alias layer is a one-time setup that also enables future shadcn/ui component drops without per-component token mapping.

**Trade-offs accepted:**
- Bundle jumped ~470 KB → ~1.1 MB (gzipped) due to Streamdown's bundled shiki syntax highlighter. Acceptable for MVP; can be code-split later.
- The ElevenLabs `AudioPlayer` component was deliberately skipped — our existing `useAudioPlayer` + rodio backend already works; adding a competing HTML5 audio path would create duplication.

## ADR-0007 — Playlist Duplicate Entries Surfaced, Not Removed

**Date:** 2026-05-11
**Status:** Accepted

**Context:** The playlist panel was showing duplicate track rows. This looked like a data-display bug, but investigation confirmed that `djmdSongPlaylist` stores one row per playlist entry without a unique constraint — Rekordbox legitimately allows the same track to appear multiple times in the same playlist.

**Decision:** Surface duplicates explicitly rather than deduplicating them. A `DUP` badge (amber-outlined mono pill) marks any row whose track ID has appeared earlier in the list. The playlist header shows the duplicate count when any exist.

**Reasons:**
1. The data is correct. Silently deduplicating would destroy user intent (e.g., a DJ set that intentionally revisits a track).
2. Making duplicates visible tells the user when their playlist has a repeat, which is often accidental and useful to know.
3. Deletion/deduplication is a write operation; all MVP changes route through the staged-change system, not the UI directly.

**Trade-offs accepted:**
- "DUP" labeling may confuse users who don't know Rekordbox allows this. Tooltip/help text can clarify; deferred.

## ADR-0008 — Synthetic Waveform as Honestly-Labeled Preview

**Date:** 2026-05-11
**Status:** Accepted

**Context:** The track inspector needed a waveform visualization. Real audio waveform rendering requires decoding audio frames from disk (via a Rust audio crate like `symphonia`), downsampling to peaks, and shipping the peak array over IPC to the renderer — significant engineering work not needed for MVP.

**Decision:** Render a decorative synthetic waveform using a seeded pseudorandom generator (seed = `track.id` hash) as a deterministic stand-in. Use the ElevenLabs `StaticWaveform` component. Label the time-range header "preview" to communicate that this is not a real audio analysis. Cue markers and region gradients are real data overlaid on the fake waveform.

**Reasons:**
1. The cue-position visualization is useful even without real audio shape; the waveform fills the space and gives context for relative positions.
2. Deterministic seeding means the waveform doesn't change between renders for the same track, which avoids jarring visual noise.
3. Honest labeling avoids misleading the user about analysis quality.

**Follow-up:** Replace with real peak data once `symphonia` decode → downsample → IPC path is implemented. The `<StaticWaveform data={peaks}>` prop interface already accepts real data.

## ADR-0009 — Treat Agent-Driven Sessions as Untrusted Until Verified

**Date:** 2026-05-15
**Status:** Accepted

**Context:** The Phase 16–22 work (Gemini CLI agent sessions over 2026-05-11 and 2026-05-12) shipped a large amount of feature code but left the workspace in a non-compiling state. JOURNAL.md and STATUS.md described the work as "complete" with `pnpm tsc --noEmit` and `cargo check` "verified clean" — claims that were not reproducible. Specific issues:

1. `apps/desktop/src-tauri/src/lib.rs` registered the `library_stage_intro_cues` Tauri command but never implemented the function body.
2. `crates/agent-tools/src/service.rs` was updated with handlers for `RelocateScan` / `RelocateApply` / `LibraryReadFileTags` / `LibraryAnalyzeTrack` / `LibraryScanAndProposeMissing` / `HealthFuzzyDuplicateScan`, but the corresponding `ToolRequest` enum variants were never added to `types.rs`.
3. An entire 399-line `SetBuilderView.tsx` Phase 3 prototype was committed but never imported and never typechecked.
4. A `health__audio_fingerprint_scan` switch arm + IPC wrapper called a Tauri command that was never registered — would crash the agent on invocation.
5. STATUS.md was simultaneously *too pessimistic* (claiming HTTP MCP transport and diff grouping needed work — both were already shipped) and *too optimistic* (claiming a green test baseline that did not exist).

**Decision:** Treat agent-shipped work as untrusted until independently verified. Going forward:

1. Before accepting any agent's claim that a feature is "shipped", run the full local verification suite — `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm build`, `pnpm e2e`. A JOURNAL claim of "verified clean" without these commands' output captured in the same commit should be regarded as unconfirmed.
2. Compile-time errors in `main` are a release blocker regardless of how recently work was merged. STATUS.md must reflect actual workspace state, not aspirational state.
3. Doc drift cuts both ways: features can be shipped without STATUS.md catching up, and STATUS.md can list features as missing when they exist. Resolve drift before planning new work.

**Reasons:**
1. The cost of a broken `main` propagates: every later session is built on a foundation that does not build, and bugs compound silently.
2. Doc drift creates plan distortions — work was scoped against an inaccurate picture of reality.
3. Agent sessions that mix research, plumbing, and feature shipping can leave partial wiring behind. Independent verification is the cheapest way to catch this.

**Trade-offs accepted:**
- This rule makes agent-led sessions feel slower because every "done" is gated on verification output, not on the agent's self-report.
- Some of the partial wiring left behind (e.g., SetBuilderView, audio-fingerprint scan) had legitimate aspirations; deleting them in this remediation pass forfeits that progress. The deleted code is in git history and can be revisited.

## ADR-0010 — Sync Relaxes the "Never Write master.db" Invariant Under WriteGuard

**Date:** 2026-05-24
**Status:** Accepted

**Context:** Until now, the project-wide invariant was "the application never mutates the user's `master.db` directly — accepted changes round-trip through Rekordbox XML export, which the user imports manually." Sub-Plan 6 wires the SyncPanel options (`cue_destination`, `keep_grids`, `convert_keys`) end-to-end into `changes::applier::apply_with_options`, which writes directly to `djmdContent` / `djmdCue` / `djmdKey` / playlist tables on the real database. That is, by construction, master.db mutation.

**Decision:** The "never mutate master.db" invariant is formally relaxed for the Sync feature only:

1. Sync is an explicit, opt-in user action. The user picks the changes, confirms via a dialog whose body reads "A timestamped backup will be created beside master.db on the first write of this session," and clicks Apply. No background path mutates master.db.
2. Every Sync write is gated by `decks_core::rekordbox_db::WriteGuard::acquire_for_write`, which (a) probes whether Rekordbox holds a WAL lock and refuses to proceed if so, and (b) creates a timestamped sibling backup `master.db.<unix-ts>.bak` on first write of the session. The transaction commits atomically inside `WriteGuard::with_tx`, so partial writes cannot corrupt master.db.
3. XML export remains the default, non-destructive path. Cleanup, Smart Fixes, Track Matcher, and the agent flow all stage `ChangeKind::*` records that are previewable in the Diff panel and exported via `export_accepted_changes` by default. Sync is offered as the more convenient alternative for users who want to skip the manual rekordbox-import dance and accept the (mitigated) write risk.
4. The disposable-DB smoke harness (`scripts/real-library-smoke.sh` + a copy of a real master.db on a scratch path) is the verification gate before each release that touches the applier. Tests in `crates/changes/src/applier{,/tracks.rs,/cues.rs}` exercise SQL-level correctness against synthetic schemas; the disposable-DB test exercises round-trip behaviour against a real Rekordbox 7 schema (column quirks, FK shape, ANLZ presence) without touching `~/Library/Pioneer/rekordbox/master.db`.

**Reasons:**
1. Round-trip through XML export is technically lossless but operationally painful — the user has to quit Rekordbox, run File → Import, and confirm a fairly opaque XML merge. For "rename one genre across 200 tracks" workflows the friction overwhelms the value.
2. The mitigations (WriteGuard lock probe + timestamped backup + transactional writes) reduce the realistic blast radius of a bad write from "library destroyed" to "revert by copying the .bak back" — a recoverable failure mode.
3. The Cleanup option toggles (cue destination, key conversion, keep-grids) are write-time decisions; they don't make sense in an export-only model because XML import doesn't expose these knobs.

**Trade-offs accepted:**
- Future contributors must remember that a Sync code path runs against the real database. Synthetic-fixture tests catch most regressions, but anything that depends on real-library quirks (column variants, ANLZ presence, FK collation) needs the disposable-DB smoke before shipping.
- The `keep_grids` and `cue_destination` semantics are intentionally narrow in this first cut: `keep_grids` only skips `TrackMetadataEdit{field: "BPM"}` (beat-grid ANLZ edits are not staged anywhere today, so there's nothing else to skip); `cue_destination` only controls the `djmdCue.Kind` value of newly inserted hot/memory cue rows (it does not retroactively re-slot existing cues). Both are documented inline in `crates/changes/src/applier.rs`.
- `convert_keys` writes the converted string through the existing `djmdKey` FK path (`get_or_create_fk`). If the user later switches between Camelot and Open Key formats, that creates additional `djmdKey` rows. Acceptable: `djmdKey` is small and Rekordbox tolerates orphan keys.

## ADR-0011 — Relicense from MIT to GPL-3.0-or-later

**Date:** 2026-08-05
**Status:** Accepted

**Context:** The Lexicon parity initiative (`docs/ROADMAP.md`) needs analysis components that do not exist under permissive licences. The strongest available key detector (`libKeyFinder`, GPL-3.0), the reference tempo/onset library (`aubio`, GPL-3.0), the BBC waveform tool (`audiowaveform`, GPL-3.0), and the single best cross-app reference implementation (`Mixxx`, GPL-2.0-or-later) are all copyleft. Under MIT we could only subprocess unmodified GPL binaries — a defensible but uncertain position — and could never read Mixxx's importers except through an expensive clean-room process.

The project owner has confirmed there is no commercial or proprietary path for `decks` and that GPL is acceptable.

**Decision:** Relicense the repository as **GPL-3.0-or-later**.

1. `LICENSE` becomes GPL-3.0. `Cargo.toml` `[workspace.package] license` becomes `GPL-3.0-or-later`. `package.json` and `README.md` follow.
2. The vendored reklawdbox code (MIT) and `stratum-dsp` carry over cleanly — MIT is GPL-compatible — and `NOTICE` keeps the attribution unchanged. Their original MIT grant is unaffected for anyone taking that code from upstream.
3. GPL-3 lets us link `libKeyFinder`, bundle `aubio` and `audiowaveform`, and use `mutagen` if ever needed. It also makes Mixxx code legally readable and reusable, though ADR-0012 still prefers spec-driven implementation.

**Reasons:**
1. Analysis quality is the product. Key detection that disagrees with what DJs see in Mixed In Key or Rekordbox is worse than useless, and the best available detectors are GPL.
2. The alternative — separate-process invocation of unmodified GPL binaries — is a legal *position*, not a certainty. Removing the question entirely is cheaper than defending it.
3. Nothing is lost: there is no commercial plan, and copyleft is well matched to a tool whose whole premise is that users own their own library data.

**Trade-offs accepted:**
- A proprietary or dual-licensed future is foreclosed without relicensing consent from every contributor. Acceptable today (effectively single-author); it will get harder.
- Distributed binaries must ship source or a written offer. The GitHub release process satisfies this as long as tags remain public.
- **Relicensing does not unlock non-free model weights.** `madmom`'s models are CC-BY-NC-SA and Essentia's TensorFlow models are CC-BY-NC-ND. Those are *non-free*, not merely incompatible — no licence choice on our side makes them redistributable, and the ND term additionally forbids fine-tuning. They remain excluded. See ADR-0012.

## ADR-0012 — Third-Party Analysis and Format Stack

**Date:** 2026-08-05
**Status:** Accepted

**Context:** The parity roadmap needs beat/downbeat tracking good enough to drive cue generation (Epic 3), key detection users trust (Epic 6), an energy metric with a defensible definition (Epic 4), and cross-format duplicate detection (Epic 5). The open-source landscape here is unusually treacherous: the pattern across music information retrieval is **permissive code, restrictive model weights**.

**Decision:** Adopt the following, and record what is deliberately excluded.

Adopt:
- **`beat_this_cpp`** (MIT, a port of CPJKU's `beat_this`) for beat and **downbeat** tracking. Downbeats are what the Cue Point Generator needs to place cues on bar boundaries; MIT means no licence question at all.
- **`libKeyFinder`** (GPL-3.0, maintained by the Mixxx team) for key detection. The DJ-industry reference implementation, now legal for us to link under ADR-0011. Keep the existing `stratum-dsp` chroma detector as the zero-setup fallback.
- **`libebur128`** (MIT) for ITU-R BS.1770 loudness. This becomes the honest, documented basis for the Energy field — an absolute, reproducible measurement rather than an opaque score.
- **Chromaprint** for cross-format audio fingerprinting, upgrading the current 128-byte chromagram hash so an MP3 and a WAV of the same recording match. **Must be built against KissFFT or FFmpeg's FFT, never FFTW3** — FFTW3 makes the result GPL-2-incompatible in ways that would bite even us.
- Keep **`lofty`** (Rust, permissive) for tag I/O. It already covers MP3/FLAC/M4A/WAV read and write; swapping to TagLib buys nothing.

Reference-only, never linked or copied:
- **`pyrekordbox`** (MIT) — the best available documentation of `master.db` write semantics and ANLZ structure. MIT, so we *may* copy; we prefer to read and reimplement in Rust.
- **Mixxx** (GPL-2.0-or-later) — now legally usable under ADR-0011, but still preferred as a spec source. Its *wiki* format documentation is the genuinely valuable part.
- **Deep Symmetry's Kaitai `.ksy` specs** for PDB/ANLZ, if USB export is ever revived from `deferred`.

**Excluded, and why:**
- **`madmom`** — code is BSD but the models are CC-BY-NC-SA. Non-free.
- **Essentia** (AGPL-3.0) and especially its **TensorFlow models** (CC-BY-NC-ND) — the models are non-free and no-derivatives, so even fine-tuning is prohibited. Essentia's AGPL code could combine with GPL-3, but without the models it offers little we need.
- **Spotify audio features** — the `audio-features` and `audio-analysis` endpoints were deprecated on 2024-11-27 and return 403 for applications registered after that date. Lexicon populates Danceability/Popularity/Happiness from Spotify; **we cannot follow them there.** Our equivalents must come from our own analysis, or not exist. Recorded in `docs/lexicon/07-health.md`.
- **YouTube audio extraction** — explicit ToS violation. Not built.

**Trade-offs accepted:**
- Danceability, Popularity and Happiness have no good open source. Popularity is inherently a catalog metric we cannot compute locally; it may simply never ship. Danceability we can approximate from onset density and rhythmic regularity, but it will not match Spotify's numbers and we should not pretend otherwise.
- Chromaprint's LGPL is a non-issue under GPL-3, but the FFTW3 build trap is easy to fall into via a distro package. Pin the build.

## ADR-0013 — Smartlist Rule Model

**Date:** 2026-08-05
**Status:** Accepted (amended 2026-08-05 — see "Implementation note" below)

**Context:** Epic 1 introduces smartlists. The obvious implementation is a general boolean expression tree. Lexicon's actual model is narrower, and the narrowness is deliberate.

**Decision:** Model a smartlist as a **two-level structure**, not a recursive tree.

```
Smartlist { combinator: Any | All, clauses: Vec<Clause> }
Clause    { rules: Vec<Rule> }          // rules within a clause are OR'd
Rule      { field, operator, value }
```

- `combinator: All` — clauses are AND'd; rules inside a clause are OR'd. This is exactly Lexicon's "OR clauses only work in All Rules mode", and expresses `(Genre = House OR Genre = Techno) AND (Rating = 3)`.
- `combinator: Any` — a flat union; every clause holds exactly one rule.

Evaluation:
1. Compile to SQL against `master.db` for fields that live there.
2. Fall back to in-memory filtering for cache-backed fields (energy, custom tags), reusing the existing batched `CacheDb::get_energy_by_uris` and `list_track_tags_map` paths so we do not reintroduce N+1 queries.
3. Exclude archived tracks unless a rule explicitly selects them.
4. Cache results with a **30-second minimum recompute interval**, surfacing a loading state when a recompute occurs.
5. Key equality routes through `changes::key_format` so `4M` matches `Am`.
6. Custom tag matching is **exact-label only** — a deliberate performance decision, not an oversight.

Degradation on sync: when a target cannot express a rule, materialise the smartlist as a normal playlist containing the current matches. For Rekordbox 6/7, tag rules map to MyTag rules (`Has all these tags` → `contains`, `Has none of these tags` → `does not contain`), limited to **4 MyTag categories and 2 rules**; anything beyond that materialises.

**Reasons:**
1. A general boolean tree is more code, a harder editor UI, and does not match the product we are cloning. Users do not ask for arbitrary nesting; they ask for "these genres, at this rating".
2. The same two-level shape already appears in Lexicon's Custom Tags page selection semantics (OR within a category, AND across categories). One model, two surfaces.
3. The 30-second throttle is a documented product behaviour, not an optimisation to add later — building it in from the start avoids a rewrite when libraries get large.

**Trade-offs accepted:**
- Expressions like `(A AND B) OR (C AND D)` are inexpressible. Lexicon has the same limitation. If it ever bites, the escape hatch is a saved smartlist referenced as a rule by another smartlist, which composes without a general tree.
- Materialising on sync means the DJ app's copy goes stale until the next sync. This is inherent to any app lacking the rule, and is what Lexicon does.

**Implementation note (Epic 1, 2026-08-05):** point 1 above proposed compiling rules to SQL where the field lives in `master.db` and falling back to in-memory filtering otherwise. As implemented, **evaluation is entirely in memory**. Two reasons emerged while building it: the app already loads the full track list to render the virtualized table and already builds the derived sets (`tracksWithCues`, `tracksInAnyPlaylist`, `tracksWithMissingFiles`, `tagsByTrack`) for the filter drawer, so the inputs are in hand either way; and a majority of the interesting rule fields — energy, custom tags, archived state, missing-file state — live in the local cache or on the filesystem rather than in `master.db`, so a SQL path would have to be abandoned mid-query for most non-trivial rule sets. A hybrid would mean two evaluators to keep in agreement for no measured gain. The SQL path stays available if profiling on a large library ever justifies it; the evaluator's signature (`&[Track] + EvalContext`) does not preclude it.
