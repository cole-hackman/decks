interface Props {
  /** 0–255 bar heights, or `undefined` while it is still being fetched. */
  bars: number[] | undefined;
  /** Rendered width in px. Height comes from the row. */
  width?: number;
  height?: number;
}

/**
 * The inline per-row waveform preview.
 *
 * Per `docs/lexicon/02-library.md §Browser`. Drawn as a single `<path>` rather
 * than forty `<rect>`s: at forty bars across four thousand rows that is the
 * difference between a table that scrolls and one that does not.
 *
 * Deliberately monochrome. The ANLZ preview carries Pioneer's colour bands, and
 * `WaveformPanel` renders them — but at 14px tall in a table, colour reads as
 * noise rather than as frequency content, and the shape is the whole point.
 */
export function RowWaveform({ bars, width = 120, height = 14 }: Props) {
  // Absent means "we have no waveform for this track", which is different from
  // silence and must not draw a flat line.
  if (!bars || bars.length === 0) return null;

  const step = width / bars.length;
  const mid = height / 2;

  // One path, alternating up-and-down strokes: M x,top V bottom for each bar.
  const d = bars
    .map((value, i) => {
      const half = Math.max(0.5, (value / 255) * mid);
      const x = (i + 0.5) * step;
      return `M${x.toFixed(2)} ${(mid - half).toFixed(2)}V${(mid + half).toFixed(2)}`;
    })
    .join("");

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      aria-hidden
      data-testid="row-waveform"
      className="text-ink-faint"
      preserveAspectRatio="none"
    >
      <path
        d={d}
        stroke="currentColor"
        strokeWidth={Math.max(1, step * 0.7)}
        strokeLinecap="butt"
        fill="none"
      />
    </svg>
  );
}
