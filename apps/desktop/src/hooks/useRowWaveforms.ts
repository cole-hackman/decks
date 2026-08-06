import { useEffect, useRef, useState } from "react";
import { getRowWaveforms } from "../ipc";

/** Bars per row preview. Enough to read a shape at ~120px wide, cheap to ship. */
export const ROW_WAVEFORM_BARS = 40;

/**
 * Lazily fetch inline waveforms for the rows currently on screen.
 *
 * Per `docs/lexicon/02-library.md §Browser`. Three properties make this safe to
 * run against a four-thousand-track library:
 *
 * - **Only visible ids are ever requested**, in one batched call per scroll
 *   settle rather than one per row.
 * - **Results are cached for the session**, so scrolling back up is free. The
 *   cache is keyed by library path and cleared when that changes, because two
 *   libraries can legitimately use the same track ids.
 * - **A track that has been asked for once is never asked again**, even when
 *   the answer was "no waveform". Without that, every scroll past a track with
 *   no ANLZ would re-read the disk to be told the same thing.
 */
export function useRowWaveforms(libraryPath: string, visibleIds: string[]) {
  const [waveforms, setWaveforms] = useState<Record<string, number[]>>({});
  const asked = useRef<Set<string>>(new Set());
  const library = useRef(libraryPath);

  if (library.current !== libraryPath) {
    library.current = libraryPath;
    asked.current = new Set();
  }

  // `visibleIds` is a fresh array every render; the join is what stops the
  // effect from re-firing on scroll positions that expose the same rows.
  const key = visibleIds.join(",");

  useEffect(() => {
    if (!libraryPath) return;
    const wanted = visibleIds.filter((id) => !asked.current.has(id));
    if (wanted.length === 0) return;
    for (const id of wanted) asked.current.add(id);

    let live = true;
    void getRowWaveforms(libraryPath, wanted, ROW_WAVEFORM_BARS)
      .then((found) => {
        if (!live) return;
        setWaveforms((prev) => ({ ...prev, ...found }));
      })
      .catch(() => {
        // A failed batch must not take the table with it. The rows simply show
        // nothing, and are not retried — a disk that just failed will fail
        // again on the next scroll tick.
      });
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [libraryPath, key]);

  return waveforms;
}
