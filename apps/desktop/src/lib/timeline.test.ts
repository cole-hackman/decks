import { describe, expect, it } from "vitest";
import {
  bpmDirection,
  buildTimeline,
  keysMix,
  LARGE_PLAYLIST_THRESHOLD,
  type TimelineTrack,
} from "./timeline";

function track(over: Partial<TimelineTrack> & { id: string }): TimelineTrack {
  return {
    title: `Track ${over.id}`,
    artist: "Someone",
    musical_key: null,
    bpm: null,
    rating: null,
    energy: null,
    ...over,
  };
}

describe("bpmDirection", () => {
  it("reads the direction of travel", () => {
    expect(bpmDirection(124, 128)).toBe("up");
    expect(bpmDirection(128, 124)).toBe("down");
    expect(bpmDirection(128, 128)).toBe("same");
  });

  it("calls a missing tempo unknown, not unchanged", () => {
    // "Unchanged" is a claim about two numbers. Painting an absence grey would
    // read as information the chart does not have.
    expect(bpmDirection(null, 128)).toBe("unknown");
    expect(bpmDirection(128, null)).toBe("unknown");
    expect(bpmDirection(undefined, undefined)).toBe("unknown");
  });

  it("ignores differences a DJ would not hear", () => {
    // A red bar for 0.04 BPM is noise dressed as a signal.
    expect(bpmDirection(128.0, 128.04)).toBe("same");
    expect(bpmDirection(128.0, 128.1)).toBe("up");
  });
});

describe("keysMix", () => {
  it("accepts the traditional moves", () => {
    expect(keysMix("8A", "8A")).toBe(true);
    expect(keysMix("8A", "8B")).toBe(true); // relative major
    expect(keysMix("8A", "9A")).toBe(true);
    expect(keysMix("12A", "1A")).toBe(true); // wraps
  });

  it("rejects a clash", () => {
    expect(keysMix("8A", "11A")).toBe(false);
    expect(keysMix("8A", "9B")).toBe(false);
  });

  it("is null rather than false when a key is unreadable", () => {
    // "These do not mix" and "we cannot tell" are different claims.
    expect(keysMix(null, "8A")).toBeNull();
    expect(keysMix("8A", "nonsense")).toBeNull();
  });

  it("understands spelled-out keys", () => {
    expect(keysMix("C minor", "5A")).toBe(true);
  });
});

describe("buildTimeline", () => {
  it("scales heights within the set, not against an absolute range", () => {
    // A warm-up running 118-124 should show its shape, not six flat bars near
    // the bottom of a 60-200 axis.
    const bars = buildTimeline(
      [
        track({ id: "a", bpm: 118 }),
        track({ id: "b", bpm: 121 }),
        track({ id: "c", bpm: 124 }),
      ],
      "bpm",
    );
    expect(bars.map((b) => b.height)).toEqual([0, 0.5, 1]);
  });

  it("gives full-height bars when every value is identical", () => {
    // Rather than dividing by a zero span.
    const bars = buildTimeline(
      [track({ id: "a", bpm: 128 }), track({ id: "b", bpm: 128 })],
      "bpm",
    );
    expect(bars.map((b) => b.height)).toEqual([1, 1]);
  });

  it("leaves a track with no value without a bar", () => {
    const bars = buildTimeline(
      [track({ id: "a", energy: 4 }), track({ id: "b" })],
      "energy",
    );
    expect(bars[1].value).toBeNull();
    expect(bars[1].height).toBeNull();
    // ...and says which value is missing rather than showing a silent gap.
    expect(bars[1].label).toBe("Track b — no energy");
  });

  it("marks the first bar's direction unknown — there is nothing before it", () => {
    const bars = buildTimeline(
      [track({ id: "a", bpm: 124 }), track({ id: "b", bpm: 128 })],
      "bpm",
    );
    expect(bars[0].direction).toBe("unknown");
    expect(bars[1].direction).toBe("up");
  });

  it("reports key compatibility against the previous track", () => {
    const bars = buildTimeline(
      [
        track({ id: "a", musical_key: "8A" }),
        track({ id: "b", musical_key: "9A" }),
        track({ id: "c", musical_key: "2B" }),
      ],
      "key",
    );
    expect(bars[0].compatibleWithPrevious).toBeNull(); // nothing before it
    expect(bars[1].compatibleWithPrevious).toBe(true);
    expect(bars[2].compatibleWithPrevious).toBe(false);
  });

  it("charts the key by wheel position, not alphabetically", () => {
    const bars = buildTimeline(
      [
        track({ id: "a", musical_key: "1A" }),
        track({ id: "b", musical_key: "12A" }),
      ],
      "key",
    );
    expect(bars[0].value).toBe(1);
    expect(bars[1].value).toBe(12);
    expect(bars[0].camelot).toBe("1A");
  });

  it("labels a BPM bar with its direction so the colour is not the only cue", () => {
    const bars = buildTimeline(
      [track({ id: "a", bpm: 124 }), track({ id: "b", bpm: 128 })],
      "bpm",
    );
    expect(bars[1].label).toBe("Track b — 128.0 BPM ↑");
  });

  it("labels a rating bar without a spurious arrow", () => {
    const bars = buildTimeline(
      [track({ id: "a", rating: 3, bpm: 120 }), track({ id: "b", rating: 5, bpm: 128 })],
      "rating",
    );
    expect(bars[1].label).toBe("Track b — 5★");
  });

  it("returns a bar per track, in order, and never drops one", () => {
    const bars = buildTimeline(
      [track({ id: "a" }), track({ id: "b", bpm: 128 }), track({ id: "c" })],
      "bpm",
    );
    expect(bars.map((b) => b.trackId)).toEqual(["a", "b", "c"]);
  });

  it("handles an empty set", () => {
    expect(buildTimeline([], "bpm")).toEqual([]);
  });

  it("keeps the large-playlist threshold where the spec put it", () => {
    // It is a set-building tool, not a collection tool — four thousand
    // two-pixel bars say nothing about flow.
    expect(LARGE_PLAYLIST_THRESHOLD).toBe(200);
  });
});
