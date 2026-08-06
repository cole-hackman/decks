import { useMemo, useState } from "react";
import { colorForKey } from "../lib/camelot";
import {
  buildTimeline,
  DIRECTION_COLORS,
  LARGE_PLAYLIST_THRESHOLD,
  type TimelineColorMode,
  type TimelineMetric,
  type TimelineTrack,
} from "../lib/timeline";

interface Props {
  tracks: TimelineTrack[];
  /** Names the set in the header — a playlist or a history set. */
  label?: string;
  onSelectTrack?: (trackId: string) => void;
}

const METRICS: { id: TimelineMetric; label: string }[] = [
  { id: "bpm", label: "BPM" },
  { id: "energy", label: "Energy" },
  { id: "rating", label: "Rating" },
  { id: "key", label: "Key" },
];

/**
 * Track Timeline — the shape of a set at a glance.
 *
 * Per `docs/lexicon/02-library.md §Track Timeline`. A chart above the browser
 * showing how a playlist flows: BPM, Energy, Rating or Key, with bars coloured
 * either by key or by **BPM change** — green if the tempo rose against the
 * previous track, red if it fell, grey if it held. The spec is right that the
 * BPM-change mode is the one you actually read a set with.
 *
 * **Hidden by default past {@link LARGE_PLAYLIST_THRESHOLD} tracks**, per the
 * spec: it is a set-building tool, not a collection tool, and four thousand
 * two-pixel bars say nothing about flow. The user can still ask for it.
 *
 * **Danceability, Popularity and Happiness are not offered** — `Track` does not
 * carry them (Epic 4). Absent beats a flat empty chart.
 */
export function TrackTimeline({ tracks, label, onSelectTrack }: Props) {
  const [metric, setMetric] = useState<TimelineMetric>("bpm");
  const [colorMode, setColorMode] = useState<TimelineColorMode>("bpm_change");
  const [shown, setShown] = useState(tracks.length <= LARGE_PLAYLIST_THRESHOLD);

  const bars = useMemo(() => buildTimeline(tracks, metric), [tracks, metric]);
  const clashes = useMemo(
    () => bars.filter((b) => b.compatibleWithPrevious === false).length,
    [bars],
  );

  if (tracks.length === 0) return null;

  if (!shown) {
    return (
      <div
        className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-1.5 text-[11px] text-muted"
        data-testid="timeline-hidden"
      >
        <span>
          {tracks.length} tracks — the timeline is for building a set, not
          browsing a collection.
        </span>
        <button
          type="button"
          className="underline"
          onClick={() => setShown(true)}
        >
          Show anyway
        </button>
      </div>
    );
  }

  return (
    <section
      className="shrink-0 border-b border-border px-3 py-2"
      aria-label="Track timeline"
      data-testid="track-timeline"
    >
      <div className="mb-1.5 flex flex-wrap items-center gap-2 text-[11px]">
        {label != null && <span className="font-medium">{label}</span>}
        <label className="flex items-center gap-1">
          <span className="text-muted">Show</span>
          <select
            aria-label="Timeline metric"
            className="rounded border border-border bg-surface px-1.5 py-0.5 text-[11px]"
            value={metric}
            onChange={(e) => setMetric(e.target.value as TimelineMetric)}
          >
            {METRICS.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          <span className="text-muted">Colour by</span>
          <select
            aria-label="Timeline colour mode"
            className="rounded border border-border bg-surface px-1.5 py-0.5 text-[11px]"
            value={colorMode}
            onChange={(e) => setColorMode(e.target.value as TimelineColorMode)}
          >
            <option value="bpm_change">BPM change</option>
            <option value="key">Key</option>
          </select>
        </label>
        {clashes > 0 && (
          <span className="text-amber-500" data-testid="timeline-clashes">
            {clashes} key change(s) outside the wheel
          </span>
        )}
        <button
          type="button"
          className="ml-auto text-muted underline"
          onClick={() => setShown(false)}
        >
          Hide
        </button>
      </div>

      <div
        className="flex h-16 items-end gap-px overflow-x-auto"
        data-testid="timeline-bars"
      >
        {bars.map((bar) => {
          const colour =
            colorMode === "key"
              ? (colorForKey(bar.camelot) ?? DIRECTION_COLORS.unknown)
              : DIRECTION_COLORS[bar.direction];
          return (
            <button
              key={bar.trackId}
              type="button"
              // The label carries the value and, for BPM, the direction — so
              // the colour is never the only way to read the chart.
              title={bar.label}
              aria-label={bar.label}
              className="min-w-[3px] flex-1 rounded-t transition-opacity hover:opacity-70"
              style={{
                height:
                  bar.height == null
                    ? "2px"
                    : `${Math.max(6, bar.height * 100)}%`,
                backgroundColor:
                  bar.height == null ? "transparent" : colour,
                borderBottom: bar.height == null ? "2px dotted #6b7280" : undefined,
              }}
              onClick={() => onSelectTrack?.(bar.trackId)}
            />
          );
        })}
      </div>

      <p className="mt-1 text-[11px] text-muted" data-testid="timeline-legend">
        {colorMode === "bpm_change"
          ? "Green: tempo rose · Red: fell · Grey: held"
          : "Coloured by key. A dotted stub means the track has no value."}
      </p>
    </section>
  );
}
