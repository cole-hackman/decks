import type { Playlist } from "../types";

function isFolder(p: Playlist): boolean {
  return p.kind === "Folder";
}

/**
 * Whether `dragged` may be dropped **into** `target`.
 *
 * Mirrors the applier's refusals rather than trusting them to be hit later: a
 * drop the sync would reject should not look like it worked until the user
 * opens the review table.
 *
 * Both checks matter —
 *
 * - Rekordbox nests under folders only, so a playlist is never a destination.
 * - A folder dropped into its own descendant detaches that whole subtree from
 *   the root. It still exists, and it is unreachable from the tree forever.
 *
 * Pure and outside the component so the rules are testable without a DOM —
 * jsdom does not run drag events, so a rule that lives inside a drop handler
 * is a rule nothing checks.
 */
export function canDropInto(
  draggedId: string,
  targetId: string,
  playlists: Playlist[],
): boolean {
  if (draggedId === targetId) return false;
  const target = playlists.find((p) => p.id === targetId);
  if (!target || !isFolder(target)) return false;

  // Walk upward from the target: if we reach the dragged node, the target is
  // beneath it. `seen` guards against already-cyclic data hanging the render.
  const byId = new Map(playlists.map((p) => [p.id, p]));
  const seen = new Set<string>();
  let current: string | null | undefined = targetId;
  while (current && !seen.has(current)) {
    if (current === draggedId) return false;
    seen.add(current);
    current = byId.get(current)?.parent_id ?? null;
  }
  return true;
}

/**
 * The siblings of `parentId`, in tree order.
 *
 * A reorder writes the whole new order for one parent, the same contract
 * `reorder_tags` uses — so the caller needs the current list to permute.
 */
export function siblingsOf(
  parentId: string | null,
  playlists: Playlist[],
): Playlist[] {
  return playlists
    .filter((p) => (p.parent_id ?? null) === parentId)
    .sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
}
