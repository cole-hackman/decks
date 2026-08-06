import { describe, it, expect } from "vitest";
import { findAll, flatten, scoreMatch, PER_KIND_LIMIT } from "./find";
import type { Playlist, Smartlist, Track } from "../types";

function track(id: string, title: string, artist: string | null): Track {
  return {
    id,
    title,
    artist,
    album: null,
    genre: null,
    musical_key: null,
    bpm: 128,
    duration_secs: 200,
    rating: null,
    comment: null,
    folder_path: `/music/${id}.mp3`,
    analysis_data_path: null,
    file_type: null,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    energy: null,
  };
}

function playlist(id: string, name: string, kind: Playlist["kind"]): Playlist {
  return { id, name, kind, parent_id: null, seq: null };
}

function smartlist(id: string, name: string, ruleCount: number): Smartlist {
  return {
    id,
    name,
    parent_folder_id: null,
    combinator: "All",
    clauses: [
      {
        rules: Array.from({ length: ruleCount }, () => ({
          field: "genre",
          op: "contains",
          value: "House",
        })),
      },
    ],
    created_at: 0,
    updated_at: 0,
  } as unknown as Smartlist;
}

const CORPUS = {
  tracks: [
    track("1", "Acid Rain", "Aphex Twin"),
    track("2", "Braindance", "Surgeon"),
    track("3", "Rain Dance", "Nobody"),
  ],
  playlists: [
    playlist("p1", "Rainy Warmup", "Playlist"),
    playlist("p2", "Rain Folder", "Folder"),
  ],
  smartlists: [smartlist("s1", "Rain Selection", 2)],
};

describe("scoreMatch", () => {
  it("ranks exact above prefix above word-start above substring", () => {
    expect(scoreMatch("rain", "rain")).toBeGreaterThan(
      scoreMatch("rainy day", "rain"),
    );
    expect(scoreMatch("rainy day", "rain")).toBeGreaterThan(
      scoreMatch("acid rain", "rain"),
    );
    expect(scoreMatch("acid rain", "rain")).toBeGreaterThan(
      scoreMatch("braindance", "rain"),
    );
  });

  it("is case-insensitive", () => {
    expect(scoreMatch("Acid Rain", "RAIN")).toBeGreaterThan(0);
  });

  it("is zero for no match", () => {
    expect(scoreMatch("Acid Rain", "techno")).toBe(0);
  });

  it("does not blow up on regex metacharacters in the query", () => {
    // A user typing "(" must not throw — the word-start test builds a regex.
    expect(() => scoreMatch("Track (Remix)", "(")).not.toThrow();
    expect(scoreMatch("Track (Remix)", "(rem")).toBeGreaterThan(0);
  });
});

describe("findAll", () => {
  it("returns nothing for an empty query", () => {
    // A Find popup that opens showing the whole library has answered a
    // question nobody asked.
    const found = findAll("", CORPUS);
    expect(flatten(found)).toHaveLength(0);
  });

  it("searches tracks, playlists and smartlists in one pass", () => {
    const found = findAll("rain", CORPUS);
    expect(found.track.map((r) => r.label)).toContain("Acid Rain");
    expect(found.playlist.map((r) => r.label)).toContain("Rainy Warmup");
    expect(found.smartlist.map((r) => r.label)).toContain("Rain Selection");
  });

  it("skips folders — they cannot be played or hold tracks", () => {
    const found = findAll("rain", CORPUS);
    expect(found.playlist.map((r) => r.label)).not.toContain("Rain Folder");
  });

  it("matches on artist as well as title", () => {
    const found = findAll("aphex", CORPUS);
    expect(found.track.map((r) => r.label)).toEqual(["Acid Rain"]);
  });

  it("ranks a word-start match above a mid-word one", () => {
    // "Rain Dance" and "Acid Rain" both start a word with it; "Braindance"
    // only contains it, and must come last.
    const labels = findAll("rain", CORPUS).track.map((r) => r.label);
    expect(labels.indexOf("Braindance")).toBe(labels.length - 1);
  });

  it("labels a smartlist with its rule count", () => {
    expect(findAll("rain", CORPUS).smartlist[0].sublabel).toBe("2 rules");
  });

  it("says '1 rule' rather than '1 rules'", () => {
    const corpus = { ...CORPUS, smartlists: [smartlist("s2", "Rain One", 1)] };
    expect(findAll("rain", corpus).smartlist[0].sublabel).toBe("1 rule");
  });

  it("caps each section independently", () => {
    const many = Array.from({ length: 50 }, (_, i) =>
      track(`t${i}`, `Rain ${i}`, null),
    );
    const found = findAll("rain", { ...CORPUS, tracks: many });
    expect(found.track).toHaveLength(PER_KIND_LIMIT);
    // A big track list must not bury the playlists.
    expect(found.playlist).toHaveLength(1);
  });

  it("breaks score ties alphabetically, so the same query is stable", () => {
    // Without this, equal-scoring results come back in library order and Enter
    // plays something different after any re-sort.
    const tracks = [
      track("1", "Rain Zebra", null),
      track("2", "Rain Alpha", null),
      track("3", "Rain Mango", null),
    ];
    const labels = findAll("rain", { ...CORPUS, tracks }).track.map(
      (r) => r.label,
    );
    expect(labels).toEqual(["Rain Alpha", "Rain Mango", "Rain Zebra"]);
  });

  it("ignores surrounding whitespace", () => {
    expect(flatten(findAll("  rain  ", CORPUS)).length).toBeGreaterThan(0);
  });
});

describe("flatten", () => {
  it("puts playlists and smartlists before tracks", () => {
    // Containers first: they are the rarer, more deliberate target, and a
    // library of thousands would otherwise always outrank them.
    const order = flatten(findAll("rain", CORPUS)).map((r) => r.kind);
    expect(order[0]).toBe("playlist");
    expect(order[1]).toBe("smartlist");
    expect(order[order.length - 1]).toBe("track");
  });
});
