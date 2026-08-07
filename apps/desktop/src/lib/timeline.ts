import { toCamelot } from "./camelot";

/**
 * Track Timeline — the shape of a set, per `docs/lexicon/02-library.md
 * §Track Timeline`.
 *
 * Pure so the interesting decisions (what a bar is worth, what colour it takes,
 * whether the chart should show at all) are testable without a DOM.
 */

/** What the bars measure. */
export type TimelineMetric = "bpm" | "energy" | "rating" | "key";

/** How the bars are coloured. */
export type TimelineColorMode = "key" | "bpm_change";

/** The minimum a track needs to appear on the timeline. */
export interface TimelineTrack {
  id: string;
  title: string | null;
  artist: string | null;
  musical_key: string | null;
  bpm: number | null;
  rating: number | null;
  energy: number | null;
}

/**
 * Beyond this many tracks the timeline is hidden by default.
 *
 * The spec's reasoning, which is worth keeping: it is a set-building tool, not
 * a collection tool. Four thousand two-pixel bars say nothing about flow.
 */
export const LARGE_PLAYLIST_THRESHOLD = 200;

export type BpmDirection = "up" | "down" | "same" | "unknown";

export interface TimelineBar {
  trackId: string;
  title: string | null;
  artist: string | null;
  /** `null` when the track has no value for the chosen metric. */
  value: number | null;
  /** 0–1 within the metric's range, for the bar height. `null` mirrors value. */
  height: number | null;
  /** How this track's tempo compares with the previous one. */
  direction: BpmDirection;
  /** Camelot code, when the key parses. */
  camelot: string | null;
  /** Whether this track's key mixes with the previous track's. */
  compatibleWithPrevious: boolean | null;
  /** What the bar says when hovered. */
  label: string;
}

/** Colours for `bpm_change` mode. Green rose, red fell, grey unchanged. */
export const DIRECTION_COLORS: Record<BpmDirection, string> = {
  up: "#22c55e",
  down: "#ef4444",
  same: "#9ca3af",
  unknown: "#4b5563",
};

function metricValue(track: TimelineTrack, metric: TimelineMetric): number | null {
  switch (metric) {
    case "bpm":
      return track.bpm;
    case "energy":
      return track.energy;
    case "rating":
      return track.rating;
    case "key": {
      // The wheel position, so the "key" chart shows movement around it
      // rather than an alphabetical scatter.
      const camelot = toCamelot(track.musical_key);
      if (camelot == null) return null;
      const n = Number.parseInt(camelot, 10);
      return Number.isNaN(n) ? null : n;
    }
  }
}

/**
 * Compare tempo with the previous track.
 *
 * A missing tempo on **either** side is `unknown`, not `same`. "Unchanged" is a
 * claim about two numbers; calling an absence unchanged would paint a grey bar
 * that reads as information.
 */
export function bpmDirection(
  previous: number | null | undefined,
  current: number | null | undefined,
): BpmDirection {
  if (previous == null || current == null) return "unknown";
  // Rounded to a tenth: 128.00 and 128.04 are the same tempo to a DJ, and a
  // red bar for 0.04 BPM is noise dressed as a signal.
  const delta = Math.round((current - previous) * 10) / 10;
  if (delta > 0) return "up";
  if (delta < 0) return "down";
  return "same";
}

/** Whether two keys mix, on the traditional wheel: same, relative, or ±1. */
export function keysMix(
  a: string | null | undefined,
  b: string | null | undefined,
): boolean | null {
  const ca = toCamelot(a);
  const cb = toCamelot(b);
  if (ca == null || cb == null) return null;
  const na = Number.parseInt(ca, 10);
  const nb = Number.parseInt(cb, 10);
  const la = ca.slice(-1).toUpperCase();
  const lb = cb.slice(-1).toUpperCase();
  if (Number.isNaN(na) || Number.isNaN(nb)) return null;
  const diff = Math.abs(na - nb);
  const distance = Math.min(diff, 12 - diff);
  if (distance === 0) return true; // same key or its relative
  return distance === 1 && la === lb;
}

/**
 * Build the bars for a set.
 *
 * Heights are scaled **within the set**, not against an absolute range: a
 * warm-up that runs 118–124 should show its shape, not six flat bars near the
 * bottom of a 60–200 axis. A set where every value is identical gets full-height
 * bars rather than a divide-by-zero.
 */
export function buildTimeline(
  tracks: TimelineTrack[],
  metric: TimelineMetric,
): TimelineBar[] {
  const values = tracks
    .map((t) => metricValue(t, metric))
    .filter((v): v is number => v != null);
  const min = values.length > 0 ? Math.min(...values) : 0;
  const max = values.length > 0 ? Math.max(...values) : 0;
  const span = max - min;

  return tracks.map((track, i) => {
    const previous = i > 0 ? tracks[i - 1] : null;
    const value = metricValue(track, metric);
    const camelot = toCamelot(track.musical_key);
    const direction = bpmDirection(previous?.bpm, track.bpm);

    return {
      trackId: track.id,
      title: track.title,
      artist: track.artist,
      value,
      height: value == null ? null : span === 0 ? 1 : (value - min) / span,
      direction,
      camelot,
      compatibleWithPrevious:
        previous == null ? null : keysMix(previous.musical_key, track.musical_key),
      label: barLabel(track, metric, value, direction),
    };
  });
}

const METRIC_UNIT: Record<TimelineMetric, string> = {
  bpm: " BPM",
  energy: " energy",
  rating: "★",
  key: "",
};

function barLabel(
  track: TimelineTrack,
  metric: TimelineMetric,
  value: number | null,
  direction: BpmDirection,
): string {
  const name = track.title ?? "Unknown title";
  if (value == null) {
    // Say which value is missing rather than showing a gap with no reason.
    return `${name} — no ${metric === "key" ? "key" : metric}`;
  }
  const shown =
    metric === "key"
      ? (track.musical_key ?? String(value))
      : `${metric === "bpm" ? value.toFixed(1) : value}${METRIC_UNIT[metric]}`;
  const arrow =
    metric === "bpm" && direction !== "unknown"
      ? { up: " ↑", down: " ↓", same: " =" }[direction]
      : "";
  return `${name} — ${shown}${arrow}`;
}
