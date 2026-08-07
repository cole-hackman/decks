import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  applyFilters,
  EMPTY_FILTERS,
  activeFilterCount,
  distinctValues,
  isInboxTrack,
  loadPersistedFilters,
  persistFilters,
  trackMissesField,
  type FilterContext,
  type Filters,
} from "./filters";
import type { Track } from "../types";

function track(partial: Partial<Track>): Track {
  return {
    id: "t",
    title: "Untitled",
    artist: null,
    album: null,
    genre: null,
    musical_key: null,
    bpm: null,
    duration_secs: null,
    rating: null,
    comment: null,
    folder_path: null,
    analysis_data_path: null,
    file_type: null,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    energy: null,
    ...partial,
  };
}

const CTX: FilterContext = {
  tracksWithCues: new Set(),
  tracksInAnyPlaylist: new Set(),
  tracksWithMissingFiles: new Set(),
  tagsByTrack: new Map(),
};

describe("applyFilters", () => {
  const tracks = [
    track({ id: "1", title: "Dark Matter", artist: "Surgeon", bpm: 140, musical_key: "8A", genre: "Techno", release_year: 2014 }),
    track({ id: "2", title: "Acid Rain", artist: "Plastikman", bpm: 125, musical_key: "5A", genre: "Acid", release_year: 1993 }),
    track({ id: "3", title: "Force + Form", artist: null, bpm: 0, musical_key: null, genre: null, release_year: null, comment: "needs metadata" }),
    track({ id: "4", title: "Drop", artist: "Boys Noize", bpm: 128, musical_key: "5A", genre: "Techno", release_year: 2020, comment: "festival opener" }),
  ];

  it("returns everything with empty filters", () => {
    expect(applyFilters(tracks, EMPTY_FILTERS, CTX)).toHaveLength(4);
  });

  it("text query matches title, artist, comment", () => {
    expect(applyFilters(tracks, { ...EMPTY_FILTERS, query: "surgeon" }, CTX).map((t) => t.id)).toEqual(["1"]);
    expect(applyFilters(tracks, { ...EMPTY_FILTERS, query: "FESTIVAL" }, CTX).map((t) => t.id)).toEqual(["4"]);
  });

  it("BPM range is inclusive on both ends", () => {
    const out = applyFilters(tracks, { ...EMPTY_FILTERS, bpmMin: 125, bpmMax: 130 }, CTX);
    expect(out.map((t) => t.id)).toEqual(["2", "4"]);
  });

  it("year range works", () => {
    const out = applyFilters(tracks, { ...EMPTY_FILTERS, yearMin: 2000 }, CTX);
    expect(out.map((t) => t.id)).toEqual(["1", "4"]);
  });

  it("key multi-select", () => {
    const out = applyFilters(tracks, { ...EMPTY_FILTERS, keys: ["5A"] }, CTX);
    expect(out.map((t) => t.id)).toEqual(["2", "4"]);
  });

  it("missing toggles intersect", () => {
    const out = applyFilters(tracks, { ...EMPTY_FILTERS, missing: ["artist", "bpm"] }, CTX);
    expect(out.map((t) => t.id)).toEqual(["3"]);
  });

  it("comment substring is case-insensitive", () => {
    const out = applyFilters(tracks, { ...EMPTY_FILTERS, commentContains: "OPENER" }, CTX);
    expect(out.map((t) => t.id)).toEqual(["4"]);
  });

  it("hasCues: yes / no uses the context set", () => {
    const ctx: FilterContext = { tracksWithCues: new Set(["1", "4"]), tracksInAnyPlaylist: new Set(), tracksWithMissingFiles: new Set(), tagsByTrack: new Map() };
    expect(applyFilters(tracks, { ...EMPTY_FILTERS, hasCues: "yes" }, ctx).map((t) => t.id)).toEqual(["1", "4"]);
    expect(applyFilters(tracks, { ...EMPTY_FILTERS, hasCues: "no" }, ctx).map((t) => t.id)).toEqual(["2", "3"]);
  });

  it("notInAnyPlaylist hides tracks present in the membership set", () => {
    const ctx: FilterContext = { tracksWithCues: new Set(), tracksInAnyPlaylist: new Set(["2", "3"]), tracksWithMissingFiles: new Set(), tagsByTrack: new Map() };
    expect(applyFilters(tracks, { ...EMPTY_FILTERS, notInAnyPlaylist: true }, ctx).map((t) => t.id)).toEqual(["1", "4"]);
  });

  it("missingFiles restricts to tracks in the missing-files set", () => {
    const ctx: FilterContext = {
      tracksWithCues: new Set(),
      tracksInAnyPlaylist: new Set(),
      tracksWithMissingFiles: new Set(["3"]),
      tagsByTrack: new Map(),
    };
    expect(
      applyFilters(tracks, { ...EMPTY_FILTERS, missingFiles: true }, ctx).map(
        (t) => t.id,
      ),
    ).toEqual(["3"]);
  });

  it("combined filters AND together", () => {
    const out = applyFilters(
      tracks,
      { ...EMPTY_FILTERS, bpmMin: 120, bpmMax: 130, keys: ["5A"] },
      CTX,
    );
    expect(out.map((t) => t.id)).toEqual(["2", "4"]);
  });

  it("treats bpm 0 as missing", () => {
    const out = applyFilters(tracks, { ...EMPTY_FILTERS, missing: ["bpm"] }, CTX);
    expect(out.map((t) => t.id)).toEqual(["3"]);
  });

  it("tagIds OR keeps tracks bound to any of the selected tags", () => {
    const ctx: FilterContext = {
      tracksWithCues: new Set(),
      tracksInAnyPlaylist: new Set(),
      tracksWithMissingFiles: new Set(),
      tagsByTrack: new Map([
        ["1", new Set(["chill"])],
        ["2", new Set(["hype"])],
        ["4", new Set(["chill", "hype"])],
      ]),
    };
    const out = applyFilters(
      tracks,
      { ...EMPTY_FILTERS, tagIds: ["chill", "hype"], tagMatchAll: false },
      ctx,
    );
    expect(out.map((t) => t.id).sort()).toEqual(["1", "2", "4"]);
  });

  it("tagIds AND requires every selected tag to be present", () => {
    const ctx: FilterContext = {
      tracksWithCues: new Set(),
      tracksInAnyPlaylist: new Set(),
      tracksWithMissingFiles: new Set(),
      tagsByTrack: new Map([
        ["1", new Set(["chill"])],
        ["2", new Set(["hype"])],
        ["4", new Set(["chill", "hype"])],
      ]),
    };
    const out = applyFilters(
      tracks,
      { ...EMPTY_FILTERS, tagIds: ["chill", "hype"], tagMatchAll: true },
      ctx,
    );
    expect(out.map((t) => t.id)).toEqual(["4"]);
  });

  it("empty tagIds is a no-op", () => {
    const ctx: FilterContext = {
      tracksWithCues: new Set(),
      tracksInAnyPlaylist: new Set(),
      tracksWithMissingFiles: new Set(),
      tagsByTrack: new Map([["1", new Set(["chill"])]]),
    };
    const out = applyFilters(tracks, { ...EMPTY_FILTERS, tagIds: [] }, ctx);
    expect(out).toHaveLength(4);
  });

  it("tagIds filter excludes tracks with no tag bindings", () => {
    const ctx: FilterContext = {
      tracksWithCues: new Set(),
      tracksInAnyPlaylist: new Set(),
      tracksWithMissingFiles: new Set(),
      tagsByTrack: new Map([["1", new Set(["chill"])]]),
    };
    const out = applyFilters(
      tracks,
      { ...EMPTY_FILTERS, tagIds: ["chill"] },
      ctx,
    );
    expect(out.map((t) => t.id)).toEqual(["1"]);
  });
});

describe("activeFilterCount", () => {
  it("returns 0 for empty filters", () => {
    expect(activeFilterCount(EMPTY_FILTERS)).toBe(0);
  });

  it("does not count the query input", () => {
    expect(activeFilterCount({ ...EMPTY_FILTERS, query: "hello" })).toBe(0);
  });

  it("counts each active filter once", () => {
    expect(
      activeFilterCount({
        ...EMPTY_FILTERS,
        bpmMin: 120,
        bpmMax: 130, // same group, counts once
        keys: ["5A"],
        missing: ["artist"],
        hasCues: "yes",
      }),
    ).toBe(4);
  });

  it("counts tagIds as a single active filter", () => {
    expect(
      activeFilterCount({ ...EMPTY_FILTERS, tagIds: ["a", "b"] }),
    ).toBe(1);
  });
});

describe("isInboxTrack", () => {
  // A "complete" track has all core metadata, is in a playlist, and has cues —
  // it should NOT be in the inbox.
  const complete = track({
    id: "complete",
    artist: "A",
    bpm: 128,
    musical_key: "8A",
    genre: "Techno",
  });
  const ctxComplete: FilterContext = {
    tracksWithCues: new Set(["complete"]),
    tracksInAnyPlaylist: new Set(["complete"]),
    tracksWithMissingFiles: new Set(),
    tagsByTrack: new Map(),
  };

  it("excludes complete tracks", () => {
    expect(isInboxTrack(complete, ctxComplete)).toBe(false);
  });

  it("includes tracks not in any playlist", () => {
    const ctx: FilterContext = {
      ...ctxComplete,
      tracksInAnyPlaylist: new Set(),
    };
    expect(isInboxTrack(complete, ctx)).toBe(true);
  });

  it("includes tracks with no cues", () => {
    const ctx: FilterContext = {
      ...ctxComplete,
      tracksWithCues: new Set(),
    };
    expect(isInboxTrack(complete, ctx)).toBe(true);
  });

  it("includes tracks missing artist/bpm/key/genre individually", () => {
    for (const field of ["artist", "bpm", "musical_key", "genre"] as const) {
      const t = track({ ...complete, [field]: null });
      expect(isInboxTrack(t, ctxComplete)).toBe(true);
    }
  });

  it("treats bpm=0 as missing (so 0 bpm imported tracks land in inbox)", () => {
    const t = track({ ...complete, bpm: 0 });
    expect(isInboxTrack(t, ctxComplete)).toBe(true);
  });

  it("does NOT include tracks missing release_year (year isn't an inbox signal)", () => {
    const t = track({ ...complete, release_year: null });
    expect(isInboxTrack(t, ctxComplete)).toBe(false);
  });
});

describe("trackMissesField", () => {
  it("treats whitespace-only strings as missing", () => {
    expect(trackMissesField(track({ artist: "   " }), "artist")).toBe(true);
    expect(trackMissesField(track({ artist: "x" }), "artist")).toBe(false);
  });

  it("treats non-finite or non-positive numbers as missing", () => {
    expect(trackMissesField(track({ bpm: 0 }), "bpm")).toBe(true);
    expect(trackMissesField(track({ bpm: -1 }), "bpm")).toBe(true);
    expect(trackMissesField(track({ bpm: NaN }), "bpm")).toBe(true);
    expect(trackMissesField(track({ bpm: 128 }), "bpm")).toBe(false);
  });
});

describe("persistFilters / loadPersistedFilters", () => {
  const STORAGE: Record<string, string> = {};
  const realStorage = globalThis.localStorage;

  beforeEach(() => {
    for (const k of Object.keys(STORAGE)) delete STORAGE[k];
    const mock: Storage = {
      getItem: (k) => (k in STORAGE ? STORAGE[k] : null),
      setItem: (k, v) => {
        STORAGE[k] = v;
      },
      removeItem: (k) => {
        delete STORAGE[k];
      },
      clear: () => {
        for (const k of Object.keys(STORAGE)) delete STORAGE[k];
      },
      key: (i) => Object.keys(STORAGE)[i] ?? null,
      get length() {
        return Object.keys(STORAGE).length;
      },
    };
    Object.defineProperty(globalThis, "localStorage", {
      value: mock,
      configurable: true,
      writable: true,
    });
  });

  afterEach(() => {
    Object.defineProperty(globalThis, "localStorage", {
      value: realStorage,
      configurable: true,
      writable: true,
    });
  });

  it("round-trips persisted state for a given library", () => {
    const filters: Filters = {
      ...EMPTY_FILTERS,
      bpmMin: 120,
      bpmMax: 130,
      keys: ["5A", "8A"],
      genres: ["Techno"],
      hasCues: "yes",
      tagIds: ["a", "b"],
      tagMatchAll: true,
    };
    persistFilters(filters, "/Users/dj/lib-a");
    const loaded = loadPersistedFilters("/Users/dj/lib-a");
    expect(loaded.bpmMin).toBe(120);
    expect(loaded.bpmMax).toBe(130);
    expect(loaded.keys).toEqual(["5A", "8A"]);
    expect(loaded.genres).toEqual(["Techno"]);
    expect(loaded.hasCues).toBe("yes");
    expect(loaded.tagIds).toEqual(["a", "b"]);
    expect(loaded.tagMatchAll).toBe(true);
  });

  it("scopes storage by libraryPath — different libraries don't bleed", () => {
    persistFilters(
      { ...EMPTY_FILTERS, bpmMin: 100, keys: ["1A"] },
      "/lib/a",
    );
    const a = loadPersistedFilters("/lib/a");
    const b = loadPersistedFilters("/lib/b");
    expect(a.bpmMin).toBe(100);
    expect(a.keys).toEqual(["1A"]);
    expect(b).toEqual(EMPTY_FILTERS);
  });

  it("resets query and missingFiles on reload", () => {
    persistFilters(
      {
        ...EMPTY_FILTERS,
        query: "ephemeral search",
        missingFiles: true,
        bpmMin: 120,
      },
      "/lib/a",
    );
    const loaded = loadPersistedFilters("/lib/a");
    expect(loaded.query).toBe("");
    expect(loaded.missingFiles).toBe(false);
    expect(loaded.bpmMin).toBe(120);
  });

  it("null libraryPath uses the legacy un-keyed storage key", () => {
    // Seed legacy data directly.
    globalThis.localStorage.setItem(
      "decks.filters.v1",
      JSON.stringify({ bpmMin: 90 }),
    );
    const loaded = loadPersistedFilters(null);
    expect(loaded.bpmMin).toBe(90);
    // And the keyed variant doesn't pick it up.
    expect(loadPersistedFilters("/lib/x").bpmMin).toBeNull();
  });

  it("does not throw when localStorage.setItem throws (e.g. quota)", () => {
    const throwingStorage: Storage = {
      getItem: () => null,
      setItem: () => {
        throw new Error("QuotaExceededError");
      },
      removeItem: () => {},
      clear: () => {},
      key: () => null,
      length: 0,
    };
    Object.defineProperty(globalThis, "localStorage", {
      value: throwingStorage,
      configurable: true,
      writable: true,
    });
    expect(() =>
      persistFilters({ ...EMPTY_FILTERS, bpmMin: 120 }, "/lib/a"),
    ).not.toThrow();
  });

  it("does not throw when localStorage.getItem returns malformed JSON", () => {
    globalThis.localStorage.setItem(
      "decks.filters.v1::/lib/a",
      "{not json",
    );
    expect(() => loadPersistedFilters("/lib/a")).not.toThrow();
    expect(loadPersistedFilters("/lib/a")).toEqual(EMPTY_FILTERS);
  });

});

describe("distinctValues", () => {
  it("returns distinct sorted non-empty values", () => {
    const tracks = [
      track({ genre: "Techno" }),
      track({ genre: "Acid" }),
      track({ genre: "Techno" }),
      track({ genre: null }),
      track({ genre: "  " }),
    ];
    expect(distinctValues(tracks, (t) => t.genre)).toEqual(["Acid", "Techno"]);
  });
});

describe("tag groups — OR within a category, AND across", () => {
  const ctx: FilterContext = {
    tracksWithCues: new Set(),
    tracksInAnyPlaylist: new Set(),
    tracksWithMissingFiles: new Set(),
    tagsByTrack: new Map([
      ["house-peak", new Set(["house", "peak"])],
      ["techno-peak", new Set(["techno", "peak"])],
      ["house-warm", new Set(["house", "warm"])],
      ["untagged", new Set<string>()],
    ]),
  };
  const tracks = ["house-peak", "techno-peak", "house-warm", "untagged"].map(
    (id) => track({ id }),
  );

  const ids = (f: Partial<Filters>) =>
    applyFilters(tracks, { ...EMPTY_FILTERS, ...f }, ctx).map((t) => t.id);

  it("requires one tag from every group", () => {
    // (house OR techno) AND peak
    expect(
      ids({ tagGroups: [["house", "techno"], ["peak"]] }).sort(),
    ).toEqual(["house-peak", "techno-peak"]);
  });

  it("ORs within a single group", () => {
    expect(ids({ tagGroups: [["house", "techno"]] }).sort()).toEqual([
      "house-peak",
      "house-warm",
      "techno-peak",
    ]);
  });

  it("excludes a track missing any group entirely", () => {
    expect(ids({ tagGroups: [["house"], ["peak"]] })).toEqual(["house-peak"]);
  });

  it("never matches an untagged track", () => {
    expect(ids({ tagGroups: [["house"]] })).not.toContain("untagged");
  });

  it("ignores empty groups rather than excluding everything", () => {
    // A category with nothing selected is not a constraint.
    expect(ids({ tagGroups: [["house"], []] })).toEqual([
      "house-peak",
      "house-warm",
    ]);
  });

  it("falls back to the flat list when no groups are given", () => {
    expect(ids({ tagIds: ["peak"], tagMatchAll: false }).sort()).toEqual([
      "house-peak",
      "techno-peak",
    ]);
  });

  it("takes precedence over the flat list when both are set", () => {
    // The grouped form is the more specific statement.
    expect(
      ids({
        tagIds: ["warm"],
        tagMatchAll: false,
        tagGroups: [["house"], ["peak"]],
      }),
    ).toEqual(["house-peak"]);
  });
});
