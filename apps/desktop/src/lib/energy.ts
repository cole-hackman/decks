/**
 * The energy scale, frontend half.
 *
 * The cache stores 0.1–1.0; the user sees **1–10**, per ADR-0015 and
 * `docs/lexicon/04-analysis.md §Energy`. This mirrors `energy::to_display` in
 * `crates/audio-analysis/src/energy.rs` — the same rounding, so a track never
 * reads as a 7 in the table and lands as an 8 in a Field Mapping.
 *
 * It lives in `lib/` rather than beside the component because the Rust side has
 * three consumers (the table, the toast, the mapping profiles) and a copy per
 * consumer is how the two halves of a scale drift apart.
 */

/** Stored 0.1–1.0 → the 1–10 integer the user sees. */
export function energyToDisplay(energy: number): number {
  if (!Number.isFinite(energy)) return 1;
  return Math.min(10, Math.max(1, Math.round(energy * 10)));
}
