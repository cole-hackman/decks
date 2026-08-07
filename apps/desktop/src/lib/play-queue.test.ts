import { describe, it, expect } from "vitest";
import {
  EMPTY_QUEUE,
  advance,
  clearQueue,
  currentId,
  enqueue,
  enqueueNext,
  jumpTo,
  moveItem,
  nextIndex,
  playNow,
  removeAt,
  rewind,
  shuffleUpcoming,
  upcoming,
  type QueueState,
} from "./play-queue";

/** A queue of `a b c d e` with `c` playing. */
const midSet: QueueState = {
  items: ["a", "b", "c", "d", "e"],
  currentIndex: 2,
  autoplay: true,
};

describe("enqueue", () => {
  it("appends to the end", () => {
    expect(enqueue(midSet, ["f"]).items).toEqual(["a", "b", "c", "d", "e", "f"]);
  });

  it("keeps duplicates — queueing the same track twice is a real thing", () => {
    expect(enqueue(midSet, ["a"]).items).toHaveLength(6);
  });

  it("is a no-op for an empty list, returning the same object", () => {
    expect(enqueue(midSet, [])).toBe(midSet);
  });

  it("does not move the marker", () => {
    expect(currentId(enqueue(midSet, ["f"]))).toBe("c");
  });
});

describe("enqueueNext", () => {
  it("inserts immediately after what is playing", () => {
    expect(enqueueNext(midSet, ["x"]).items).toEqual([
      "a",
      "b",
      "c",
      "x",
      "d",
      "e",
    ]);
  });

  it("goes to the front when nothing is playing yet", () => {
    const fresh = { ...EMPTY_QUEUE, items: ["a", "b"] };
    expect(enqueueNext(fresh, ["x"]).items).toEqual(["x", "a", "b"]);
  });

  it("keeps the marker on the playing track", () => {
    expect(currentId(enqueueNext(midSet, ["x"]))).toBe("c");
  });
});

describe("removeAt", () => {
  it("drops the item", () => {
    expect(removeAt(midSet, 4).items).toEqual(["a", "b", "c", "d"]);
  });

  it("keeps the marker on the same track when removing something before it", () => {
    // Deleting a played entry must not silently skip the next one.
    const after = removeAt(midSet, 0);
    expect(currentId(after)).toBe("c");
    expect(after.currentIndex).toBe(1);
  });

  it("leaves the marker in place when the playing track is removed", () => {
    // The next item slides into the slot; the following advance picks up from
    // there rather than skipping it.
    const after = removeAt(midSet, 2);
    expect(after.items).toEqual(["a", "b", "d", "e"]);
    expect(currentId(after)).toBe("d");
  });

  it("clamps when the playing track was last", () => {
    const atEnd = { ...midSet, currentIndex: 4 };
    const after = removeAt(atEnd, 4);
    expect(after.items).toEqual(["a", "b", "c", "d"]);
    expect(after.currentIndex).toBe(3);
  });

  it("ignores an out-of-range index", () => {
    expect(removeAt(midSet, 99)).toBe(midSet);
    expect(removeAt(midSet, -1)).toBe(midSet);
  });
});

describe("moveItem", () => {
  it("reorders", () => {
    expect(moveItem(midSet, 4, 0).items).toEqual(["e", "a", "b", "c", "d"]);
  });

  it("follows the playing track when it is the one moved", () => {
    const after = moveItem(midSet, 2, 0);
    expect(after.items).toEqual(["c", "a", "b", "d", "e"]);
    expect(currentId(after)).toBe("c");
  });

  it("keeps the marker when something moves across it from before", () => {
    const after = moveItem(midSet, 0, 4);
    expect(currentId(after)).toBe("c");
  });

  it("keeps the marker when something moves across it from after", () => {
    const after = moveItem(midSet, 4, 0);
    expect(currentId(after)).toBe("c");
  });

  it("is a no-op for a move onto itself or out of range", () => {
    expect(moveItem(midSet, 2, 2)).toBe(midSet);
    expect(moveItem(midSet, 0, 99)).toBe(midSet);
  });
});

describe("shuffleUpcoming", () => {
  it("leaves everything up to and including the playing track alone", () => {
    // Shuffling history would move the marker under the playing track, which
    // reads as the queue losing its place mid-set.
    const after = shuffleUpcoming(midSet, () => 0);
    expect(after.items.slice(0, 3)).toEqual(["a", "b", "c"]);
    expect(currentId(after)).toBe("c");
  });

  it("permutes the upcoming items without losing any", () => {
    const after = shuffleUpcoming(midSet, () => 0);
    expect([...upcoming(after)].sort()).toEqual(["d", "e"]);
  });

  it("shuffles the whole list when nothing has played yet", () => {
    const fresh = { ...EMPTY_QUEUE, items: ["a", "b", "c"] };
    const after = shuffleUpcoming(fresh, () => 0);
    expect([...after.items].sort()).toEqual(["a", "b", "c"]);
    expect(after.currentIndex).toBe(-1);
  });
});

describe("clearQueue", () => {
  it("keeps the playing track — Clear Queue is not Stop", () => {
    const after = clearQueue(midSet);
    expect(after.items).toEqual(["c"]);
    expect(currentId(after)).toBe("c");
  });

  it("empties outright when nothing is playing", () => {
    const fresh = { ...EMPTY_QUEUE, items: ["a", "b"] };
    expect(clearQueue(fresh).items).toEqual([]);
    expect(clearQueue(fresh).currentIndex).toBe(-1);
  });
});

describe("advance and rewind", () => {
  it("steps forward and back", () => {
    expect(currentId(advance(midSet))).toBe("d");
    expect(currentId(rewind(midSet))).toBe("b");
  });

  it("stops at the end rather than looping", () => {
    const atEnd = { ...midSet, currentIndex: 4 };
    expect(advance(atEnd)).toBe(atEnd);
    expect(nextIndex(atEnd)).toBeNull();
  });

  it("stops at the start", () => {
    const atStart = { ...midSet, currentIndex: 0 };
    expect(rewind(atStart)).toBe(atStart);
  });

  it("starts from the first item when nothing has played", () => {
    const fresh = { ...EMPTY_QUEUE, items: ["a", "b"] };
    expect(currentId(advance(fresh))).toBe("a");
  });
});

describe("playNow", () => {
  it("replaces the queue rather than appending", () => {
    // Playing a track from the browser should feel like playing a track, not
    // like queueing one behind whatever was already lined up.
    const after = playNow(midSet, ["z"]);
    expect(after.items).toEqual(["z"]);
    expect(currentId(after)).toBe("z");
  });

  it("is a no-op for an empty list", () => {
    expect(playNow(midSet, [])).toBe(midSet);
  });
});

describe("jumpTo", () => {
  it("moves the marker to a clicked entry", () => {
    expect(currentId(jumpTo(midSet, 0))).toBe("a");
  });

  it("ignores an out-of-range index", () => {
    expect(jumpTo(midSet, 99)).toBe(midSet);
  });
});

describe("upcoming", () => {
  it("is everything after the playing track", () => {
    expect(upcoming(midSet)).toEqual(["d", "e"]);
  });

  it("is the whole queue before anything plays", () => {
    expect(upcoming({ ...EMPTY_QUEUE, items: ["a", "b"] })).toEqual(["a", "b"]);
  });
});
