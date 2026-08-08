import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { EnergyBar } from "./EnergyBar";

function fillWidth() {
  const fill = screen.getByTestId("energy-bar-fill") as HTMLElement;
  return fill.style.width;
}

describe("EnergyBar", () => {
  it("renders 0% width for value 0", () => {
    render(<EnergyBar value={0} />);
    expect(fillWidth()).toBe("0%");
  });

  it("renders 50% width for value 0.5", () => {
    render(<EnergyBar value={0.5} />);
    expect(fillWidth()).toBe("50%");
  });

  it("renders 100% width for value 1", () => {
    render(<EnergyBar value={1} />);
    expect(fillWidth()).toBe("100%");
  });

  it("clamps values above 1", () => {
    render(<EnergyBar value={2} />);
    expect(fillWidth()).toBe("100%");
  });

  it("clamps negative values", () => {
    render(<EnergyBar value={-0.5} />);
    expect(fillWidth()).toBe("0%");
  });

  it("announces the 1–10 scale, not the stored fraction", () => {
    // A screen reader saying "0.42" reads out a number on no published scale.
    // The scale is 1–10 (ADR-0015), so that is what the ARIA range says too.
    render(<EnergyBar value={0.42} />);
    const bar = screen.getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuenow", "4");
    expect(bar).toHaveAttribute("aria-valuemin", "1");
    expect(bar).toHaveAttribute("aria-valuemax", "10");
    expect(bar).toHaveAttribute("title", "Energy 4 of 10");
  });

  it("still fills proportionally to the stored value", () => {
    // The bar's width is the one place the raw 0–1 is still the right unit.
    render(<EnergyBar value={0.42} />);
    expect(fillWidth()).toBe("42%");
  });
});
