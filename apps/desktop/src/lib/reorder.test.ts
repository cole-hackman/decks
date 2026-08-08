import { describe, expect, it } from "vitest";
import { indexOfId, moveWithin } from "./reorder";

describe("moveWithin", () => {
  const list = ["a", "b", "c", "d"];

  it("moves an item forward", () => {
    expect(moveWithin(list, 0, 2)).toEqual(["b", "c", "a", "d"]);
  });

  it("moves an item backward", () => {
    expect(moveWithin(list, 3, 1)).toEqual(["a", "d", "b", "c"]);
  });

  it("returns the same list when nothing moved", () => {
    expect(moveWithin(list, 1, 1)).toBe(list);
  });

  it("leaves the list untouched for an out-of-range index", () => {
    // A drop outside the list is a cancelled drag. Clamping would turn it into
    // a move the user did not make.
    expect(moveWithin(list, 0, 9)).toBe(list);
    expect(moveWithin(list, -1, 2)).toBe(list);
    expect(moveWithin(list, 9, 0)).toBe(list);
  });

  it("never drops or duplicates an item", () => {
    for (let from = 0; from < list.length; from++) {
      for (let to = 0; to < list.length; to++) {
        const out = moveWithin(list, from, to);
        expect([...out].sort()).toEqual([...list].sort());
      }
    }
  });

  it("does not mutate the input", () => {
    const original = [...list];
    moveWithin(list, 0, 3);
    expect(list).toEqual(original);
  });
});

describe("indexOfId", () => {
  const items = [{ id: "x" }, { id: "y" }];

  it("finds a known id", () => {
    expect(indexOfId(items, "y")).toBe(1);
  });

  it("returns -1 for an unknown id, which moveWithin then ignores", () => {
    expect(indexOfId(items, "nope")).toBe(-1);
    expect(moveWithin(items, indexOfId(items, "nope"), 0)).toBe(items);
  });
});
