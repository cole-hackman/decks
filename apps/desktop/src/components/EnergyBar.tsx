import { energyToDisplay } from "../lib/energy";

/**
 * Tiny horizontal bar visualising a track's energy, with the 1–10 number in the
 * tooltip and in the accessible value.
 *
 * The bar takes the stored 0.1–1.0 value because that is what fills a width,
 * but everything a user reads off it is the 1–10 scale of ADR-0015 — a tooltip
 * saying "Energy 0.62" is a number on no published scale at all, which is the
 * state `PARITY.md` recorded as "cached + displayed; no defined scale".
 */
export function EnergyBar({ value }: { value: number }) {
  const clamped = Math.max(0, Math.min(1, value));
  const pct = clamped * 100;
  const display = energyToDisplay(clamped);
  return (
    <div
      role="progressbar"
      aria-valuemin={1}
      aria-valuemax={10}
      aria-valuenow={display}
      aria-label="Energy"
      title={`Energy ${display} of 10`}
      className="inline-block h-2 w-12 overflow-hidden rounded bg-elevated align-middle"
    >
      <div
        data-testid="energy-bar-fill"
        className="h-full rounded-l bg-accent"
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}
