import { describe, it, expect } from "vitest";
import { findDuplicates } from "./playlist-dedupe";
import type { Track } from "../types";

function t(id: string): Track {
  return {
    id,
    title: id,
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
    label: null,
    remixer: null,
    mix: null,
    color: null,
    date_added: null,
    energy: null,
  };
}

describe("findDuplicates", () => {
  it("returns no duplicates for empty input", () => {
    const info = findDuplicates([]);
    expect(info.duplicateRowCount).toBe(0);
    expect(info.occurrenceCount.size).toBe(0);
  });

  it("returns no duplicates when all tracks are unique", () => {
    const info = findDuplicates([t("a"), t("b"), t("c")]);
    expect(info.duplicateRowCount).toBe(0);
    expect(info.duplicateRanks.get(0)).toBe(1);
    expect(info.duplicateRanks.get(1)).toBe(1);
    expect(info.duplicateRanks.get(2)).toBe(1);
  });

  it("counts a single duplicate pair", () => {
    const info = findDuplicates([t("a"), t("b"), t("a")]);
    expect(info.duplicateRowCount).toBe(1);
    expect(info.occurrenceCount.get("a")).toBe(2);
    expect(info.duplicateRanks.get(0)).toBe(1); // first "a"
    expect(info.duplicateRanks.get(1)).toBe(1); // first "b"
    expect(info.duplicateRanks.get(2)).toBe(2); // second "a"
  });

  it("handles a triple", () => {
    const info = findDuplicates([t("x"), t("x"), t("x")]);
    expect(info.duplicateRowCount).toBe(2);
    expect(info.occurrenceCount.get("x")).toBe(3);
    expect(info.duplicateRanks.get(0)).toBe(1);
    expect(info.duplicateRanks.get(1)).toBe(2);
    expect(info.duplicateRanks.get(2)).toBe(3);
  });

  it("preserves the original ordering", () => {
    const info = findDuplicates([t("a"), t("b"), t("a"), t("c"), t("b")]);
    expect(info.duplicateRanks.get(0)).toBe(1);
    expect(info.duplicateRanks.get(1)).toBe(1);
    expect(info.duplicateRanks.get(2)).toBe(2);
    expect(info.duplicateRanks.get(3)).toBe(1);
    expect(info.duplicateRanks.get(4)).toBe(2);
    expect(info.duplicateRowCount).toBe(2);
  });
});
