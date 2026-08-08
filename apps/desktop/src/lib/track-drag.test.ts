import { describe, expect, it } from "vitest";
import {
  dragPayload,
  encodeDragPayload,
  readDragPayload,
} from "./track-drag";

describe("dragPayload", () => {
  it("drags the whole selection when the row is part of it", () => {
    // Dragging one of five highlighted rows to mean only that row would make
    // the highlight a lie.
    const selected = new Set(["a", "b", "c"]);
    expect(dragPayload("b", selected).sort()).toEqual(["a", "b", "c"]);
  });

  it("drags only the row when it is outside the selection", () => {
    // And does not silently extend the selection to include it.
    const selected = new Set(["a", "b"]);
    expect(dragPayload("z", selected)).toEqual(["z"]);
  });

  it("drags one row when nothing is selected", () => {
    expect(dragPayload("z", new Set())).toEqual(["z"]);
  });
});

describe("payload encoding", () => {
  it("round-trips", () => {
    const ids = ["a", "b", "c"];
    expect(readDragPayload(encodeDragPayload(ids))).toEqual(ids);
  });

  it("reads nothing from a drag that carried nothing", () => {
    // A drag from another app, or from a browser that dropped the data.
    expect(readDragPayload(null)).toEqual([]);
    expect(readDragPayload("")).toEqual([]);
    expect(readDragPayload("   ")).toEqual([]);
  });

  it("ignores blank entries rather than staging an empty id", () => {
    expect(readDragPayload("a\n\n b \n")).toEqual(["a", "b"]);
  });
});
