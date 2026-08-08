/**
 * Dragging tracks out of the browser.
 *
 * The payload is deliberately a plain list of track ids under a custom MIME
 * type, not JSON of whole `Track` objects: a drop target only ever needs to say
 * *which* tracks, and shipping the rows themselves would mean two copies of the
 * truth, the stale one being whichever the drag started with.
 */
export const TRACK_IDS_MIME = "application/x-decks-track-ids";

/**
 * What a drag starting on `rowTrackId` should carry.
 *
 * Dragging a row **inside** the selection drags the whole selection — that is
 * what a multi-select is for, and dragging one of five highlighted rows to mean
 * only that row would make the highlight a lie. Dragging a row *outside* it
 * drags just that row, and does not silently extend the selection.
 */
export function dragPayload(
  rowTrackId: string,
  selected: ReadonlySet<string>,
): string[] {
  return selected.has(rowTrackId) ? Array.from(selected) : [rowTrackId];
}

/** Read a drop payload back, tolerating a drag that came from anywhere else. */
export function readDragPayload(data: string | null | undefined): string[] {
  if (!data) return [];
  return data
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
}

export function encodeDragPayload(ids: string[]): string {
  return ids.join("\n");
}
