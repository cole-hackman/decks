import { describe, expect, it } from "vitest";
import {
  coerceRule,
  defaultValueFor,
  describeRule,
  operatorsFor,
  takesOperand,
} from "./smartlist-fields";

describe("smartlist-fields", () => {
  it("offers only operators valid for the field kind", () => {
    expect(operatorsFor("title")).toContain("contains");
    expect(operatorsFor("title")).not.toContain("between");

    expect(operatorsFor("bpm")).toContain("between");
    expect(operatorsFor("bpm")).not.toContain("contains");

    expect(operatorsFor("has_cues")).toEqual(["is_true", "is_false"]);
    expect(operatorsFor("tags")).toEqual(["has_all", "has_any", "has_none"]);

    // Key gets equality but not substring matching — it is compared through
    // notation canonicalisation, so "contains" would be meaningless.
    expect(operatorsFor("musical_key")).toContain("equals");
    expect(operatorsFor("musical_key")).not.toContain("contains");
  });

  it("knows which operators take an operand", () => {
    expect(takesOperand("contains")).toBe(true);
    expect(takesOperand("between")).toBe(true);
    expect(takesOperand("is_none")).toBe(false);
    expect(takesOperand("is_true")).toBe(false);
  });

  it("produces a value shape matching the field and operator", () => {
    expect(defaultValueFor("title", "contains")).toEqual({ type: "text", value: "" });
    expect(defaultValueFor("bpm", "greater_than")).toEqual({ type: "number", value: 0 });
    expect(defaultValueFor("bpm", "between")).toEqual({ type: "range", value: [0, 0] });
    expect(defaultValueFor("tags", "has_all")).toEqual({ type: "tags", value: [] });
    expect(defaultValueFor("has_cues", "is_true")).toEqual({ type: "none" });
    expect(defaultValueFor("title", "is_none")).toEqual({ type: "none" });
  });

  it("coerces the operator when a field change makes it invalid", () => {
    // `contains` is not valid on a number field, so it falls back to the first
    // valid operator rather than producing a rule the backend would reject.
    const next = coerceRule("bpm", "contains");
    expect(operatorsFor("bpm")).toContain(next.op);
    expect(next.op).not.toBe("contains");
    expect(next.value).toEqual({ type: "number", value: 0 });
  });

  it("keeps a still-valid operator when the field changes", () => {
    const next = coerceRule("album", "contains");
    expect(next.op).toBe("contains");
  });

  it("describes rules readably", () => {
    expect(describeRule("genre", "equals", { type: "text", value: "House" })).toBe(
      'Genre is "House"',
    );
    expect(describeRule("bpm", "between", { type: "range", value: [120, 130] })).toBe(
      "BPM between 120–130",
    );
    expect(describeRule("has_cues", "is_true", { type: "none" })).toBe(
      "Has cues is true",
    );
    expect(
      describeRule("tags", "has_all", { type: "tags", value: ["a", "b"] }),
    ).toBe("Custom tags has all of 2 tag(s)");
  });

  it("offers comparison but not search operators for a date field", () => {
    // A date is compared, not searched — `contains` on a timestamp is noise.
    const ops = operatorsFor("date_added");
    expect(ops).toContain("greater_than");
    expect(ops).toContain("between");
    expect(ops).not.toContain("contains");
  });

  it("gives a date between rule two date operands, not two numbers", () => {
    expect(defaultValueFor("date_added", "between")).toEqual({
      type: "text_range",
      value: ["", ""],
    });
    expect(defaultValueFor("date_added", "greater_than")).toEqual({
      type: "text",
      value: "",
    });
  });

  it("treats the new library fields as text", () => {
    for (const field of ["label", "remixer", "mix", "color"] as const) {
      expect(operatorsFor(field)).toContain("contains");
    }
  });

  it("describes a date range readably", () => {
    expect(
      describeRule("date_added", "between", {
        type: "text_range",
        value: ["2025-01-01", "2025-06-30"],
      }),
    ).toBe("Date added between 2025-01-01–2025-06-30");
  });
});
