import { describe, expect, it } from "vitest";
import { energyToDisplay } from "./energy";

describe("energyToDisplay", () => {
  it("maps the stored range onto 1–10", () => {
    expect(energyToDisplay(0.1)).toBe(1);
    expect(energyToDisplay(1.0)).toBe(10);
  });

  it("reaches every integer in between", () => {
    // A gap would mean a smartlist rule like `energy = 7` never matched.
    const seen = new Set<number>();
    for (let i = 0; i <= 100; i++) seen.add(energyToDisplay(0.1 + 0.9 * (i / 100)));
    expect([...seen].sort((a, b) => a - b)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
  });

  it("clamps rather than escaping the scale", () => {
    // Rows written by an older analyzer version, or a hand-edited cache, can
    // hold anything. The scale has no 0 and no 11.
    expect(energyToDisplay(0)).toBe(1);
    expect(energyToDisplay(-5)).toBe(1);
    expect(energyToDisplay(2)).toBe(10);
  });

  it("does not propagate a NaN into the UI", () => {
    expect(energyToDisplay(NaN)).toBe(1);
    expect(energyToDisplay(Infinity)).toBe(1);
  });

  it("rounds the same way the Rust half does", () => {
    // `energy::to_display` uses f32::round — half away from zero — and these
    // are the boundaries where a floor or a banker's rounding would disagree.
    expect(energyToDisplay(0.65)).toBe(7);
    expect(energyToDisplay(0.649)).toBe(6);
    expect(energyToDisplay(0.75)).toBe(8);
  });
});
