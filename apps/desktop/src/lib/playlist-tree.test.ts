import { describe, expect, it } from "vitest";
import { canDropInto, siblingsOf } from "./playlist-tree";
import type { Playlist } from "../types";

function node(
  id: string,
  kind: Playlist["kind"],
  parent_id: string | null,
  seq = 1,
): Playlist {
  return { id, name: id, kind, parent_id, seq };
}

/** f1 ▸ f2 ▸ p1 ; f3 and p2 at the root. */
const TREE: Playlist[] = [
  node("f1", "Folder", null, 1),
  node("f2", "Folder", "f1", 1),
  node("p1", "Playlist", "f2", 1),
  node("f3", "Folder", null, 2),
  node("p2", "Playlist", null, 3),
];

describe("canDropInto", () => {
  it("allows a playlist into an unrelated folder", () => {
    expect(canDropInto("p2", "f3", TREE)).toBe(true);
  });

  it("refuses a playlist as a destination", () => {
    // Rekordbox nests under folders only; a playlist parented to a playlist is
    // a shape nothing can render.
    expect(canDropInto("p2", "p1", TREE)).toBe(false);
  });

  it("refuses a folder into itself", () => {
    expect(canDropInto("f1", "f1", TREE)).toBe(false);
  });

  /// The drop that would detach a whole subtree from the root forever.
  it("refuses a folder into its own descendant", () => {
    expect(canDropInto("f1", "f2", TREE)).toBe(false);
  });

  it("still allows a folder into a sibling folder", () => {
    // The descendant check must not be so broad that it refuses real moves.
    expect(canDropInto("f1", "f3", TREE)).toBe(true);
  });

  it("refuses an unknown destination rather than assuming it is fine", () => {
    expect(canDropInto("p2", "nope", TREE)).toBe(false);
  });

  it("terminates on already-cyclic data rather than hanging", () => {
    // A database that already contains a cycle must not hang the render. The
    // verdict is not the point — reaching one is. `z` genuinely is not an
    // ancestor of `a`, so `true` is the right answer, and the applier's own
    // check agrees for the same reason.
    const cyclic: Playlist[] = [
      node("a", "Folder", "b"),
      node("b", "Folder", "a"),
    ];
    expect(canDropInto("z", "a", cyclic)).toBe(true);
    // And a real cycle member still cannot swallow its own ancestor.
    expect(canDropInto("a", "b", cyclic)).toBe(false);
  });
});

describe("siblingsOf", () => {
  it("returns one parent's children in tree order", () => {
    expect(siblingsOf(null, TREE).map((p) => p.id)).toEqual(["f1", "f3", "p2"]);
  });

  it("returns a folder's children", () => {
    expect(siblingsOf("f1", TREE).map((p) => p.id)).toEqual(["f2"]);
  });

  it("returns nothing for a childless parent", () => {
    expect(siblingsOf("p1", TREE)).toEqual([]);
  });
});
