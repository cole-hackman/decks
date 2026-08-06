import { describe, it, expect } from "vitest";
import {
  moveCell,
  moveForKey,
  startsEdit,
  initialEditValue,
  selectionRange,
  type Cell,
} from "./grid-nav";

const SIZE = { rows: 10, cols: 4 };
const at = (row: number, col: number): Cell => ({ row, col });

describe("moveCell", () => {
  it("walks in all four directions", () => {
    expect(moveCell(at(5, 2), "down", SIZE)).toEqual(at(6, 2));
    expect(moveCell(at(5, 2), "up", SIZE)).toEqual(at(4, 2));
    expect(moveCell(at(5, 2), "right", SIZE)).toEqual(at(5, 3));
    expect(moveCell(at(5, 2), "left", SIZE)).toEqual(at(5, 1));
  });

  it("clamps rather than wraps at every edge", () => {
    // Holding ↓ in a 4,000-track library must stop at the bottom, not return
    // to the top and let the next keystroke edit the wrong row.
    expect(moveCell(at(9, 0), "down", SIZE)).toEqual(at(9, 0));
    expect(moveCell(at(0, 0), "up", SIZE)).toEqual(at(0, 0));
    expect(moveCell(at(0, 3), "right", SIZE)).toEqual(at(0, 3));
    expect(moveCell(at(0, 0), "left", SIZE)).toEqual(at(0, 0));
  });

  it("does not spill horizontally onto another row", () => {
    // → at the last column is a no-op, not a jump to a different track.
    expect(moveCell(at(4, 3), "right", SIZE).row).toBe(4);
    expect(moveCell(at(4, 0), "left", SIZE).row).toBe(4);
  });

  it("home and end move within the row, not the grid", () => {
    expect(moveCell(at(6, 2), "home", SIZE)).toEqual(at(6, 0));
    expect(moveCell(at(6, 2), "end", SIZE)).toEqual(at(6, 3));
  });

  it("documentStart and documentEnd jump to the corners", () => {
    expect(moveCell(at(6, 2), "documentStart", SIZE)).toEqual(at(0, 0));
    expect(moveCell(at(6, 2), "documentEnd", SIZE)).toEqual(at(9, 3));
  });

  it("pages by the given number of rows and clamps", () => {
    expect(moveCell(at(0, 1), "pageDown", SIZE, 3)).toEqual(at(3, 1));
    expect(moveCell(at(0, 1), "pageDown", SIZE, 50)).toEqual(at(9, 1));
    expect(moveCell(at(9, 1), "pageUp", SIZE, 50)).toEqual(at(0, 1));
  });

  it("is a no-op on an empty grid instead of producing a negative cell", () => {
    const empty = { rows: 0, cols: 0 };
    expect(moveCell(at(0, 0), "down", empty)).toEqual(at(0, 0));
    expect(moveCell(at(0, 0), "documentEnd", empty)).toEqual(at(0, 0));
  });

  it("pulls an out-of-range cursor back in before moving", () => {
    // The grid shrinks under the cursor whenever a filter narrows the list.
    expect(moveCell(at(99, 99), "up", SIZE)).toEqual(at(8, 3));
    expect(moveCell(at(99, 99), "home", SIZE)).toEqual(at(9, 0));
  });
});

describe("moveForKey", () => {
  it("maps the arrows, Home/End and the page keys", () => {
    expect(moveForKey({ key: "ArrowDown" })).toBe("down");
    expect(moveForKey({ key: "ArrowUp" })).toBe("up");
    expect(moveForKey({ key: "ArrowLeft" })).toBe("left");
    expect(moveForKey({ key: "ArrowRight" })).toBe("right");
    expect(moveForKey({ key: "Home" })).toBe("home");
    expect(moveForKey({ key: "End" })).toBe("end");
    expect(moveForKey({ key: "PageUp" })).toBe("pageUp");
    expect(moveForKey({ key: "PageDown" })).toBe("pageDown");
  });

  it("promotes an arrow to a jump when Cmd or Ctrl is held", () => {
    expect(moveForKey({ key: "ArrowDown", metaKey: true })).toBe("documentEnd");
    expect(moveForKey({ key: "ArrowUp", ctrlKey: true })).toBe("documentStart");
    expect(moveForKey({ key: "ArrowLeft", metaKey: true })).toBe("home");
    expect(moveForKey({ key: "ArrowRight", metaKey: true })).toBe("end");
  });

  it("keeps shift, so a shift-extended move is still a move", () => {
    expect(moveForKey({ key: "ArrowDown", shiftKey: true })).toBe("down");
  });

  it("ignores Alt and anything it does not claim", () => {
    // A grid that swallows every keystroke is worse than one with none.
    expect(moveForKey({ key: "ArrowDown", altKey: true })).toBeNull();
    expect(moveForKey({ key: "a" })).toBeNull();
    expect(moveForKey({ key: "Tab" })).toBeNull();
    expect(moveForKey({ key: "Escape" })).toBeNull();
  });
});

describe("startsEdit", () => {
  it("opens on Enter and F2", () => {
    expect(startsEdit({ key: "Enter" })).toBe(true);
    expect(startsEdit({ key: "F2" })).toBe(true);
  });

  it("opens on a printable character — typing over a cell is the point", () => {
    expect(startsEdit({ key: "a" })).toBe(true);
    expect(startsEdit({ key: "7" })).toBe(true);
    expect(startsEdit({ key: "—" })).toBe(true);
  });

  it("does not open on a modified key, so Cmd+A still selects all", () => {
    expect(startsEdit({ key: "a", metaKey: true })).toBe(false);
    expect(startsEdit({ key: "a", ctrlKey: true })).toBe(false);
    expect(startsEdit({ key: "Enter", metaKey: true })).toBe(false);
  });

  it("does not open on navigation keys or space", () => {
    expect(startsEdit({ key: "ArrowDown" })).toBe(false);
    expect(startsEdit({ key: "Shift" })).toBe(false);
    expect(startsEdit({ key: "Escape" })).toBe(false);
    // Space is the play/pause shortcut; taking it would be a bad trade.
    expect(startsEdit({ key: " " })).toBe(false);
  });
});

describe("initialEditValue", () => {
  it("seeds with the typed character, replacing what was there", () => {
    expect(initialEditValue({ key: "N" }, "Old Title")).toBe("N");
  });

  it("seeds with the existing value when opened with Enter or F2", () => {
    expect(initialEditValue({ key: "Enter" }, "Old Title")).toBe("Old Title");
    expect(initialEditValue({ key: "F2" }, "Old Title")).toBe("Old Title");
  });
});

describe("selectionRange", () => {
  it("covers the rows between anchor and cursor, in either direction", () => {
    expect(selectionRange(2, 5)).toEqual([2, 3, 4, 5]);
    expect(selectionRange(5, 2)).toEqual([2, 3, 4, 5]);
  });

  it("is a single row when they coincide", () => {
    expect(selectionRange(3, 3)).toEqual([3]);
  });

  it("shrinks back rather than growing the other way", () => {
    // shift-↓ twice then shift-↑ once leaves two rows, not four.
    const anchor = 4;
    expect(selectionRange(anchor, 6)).toHaveLength(3);
    expect(selectionRange(anchor, 5)).toHaveLength(2);
  });
});
