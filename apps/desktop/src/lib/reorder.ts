/**
 * Moving one item within an ordered list.
 *
 * Pure and separate from the components that drag things, so the ordering rule
 * is testable without a DOM — jsdom does not run drag events, and a reorder
 * bug that only a real browser can catch is one that ships.
 */
export function moveWithin<T>(items: T[], from: number, to: number): T[] {
  // Out-of-range indices return the list untouched rather than throwing or
  // silently clamping. A drop outside the list is a cancelled drag, and
  // clamping would turn it into a move the user did not make.
  if (from === to) return items;
  if (from < 0 || to < 0 || from >= items.length || to >= items.length) {
    return items;
  }
  const next = items.slice();
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

/** Index of `id` in `items`, or -1. Keyed lists drag by id, not by position. */
export function indexOfId<T extends { id: string }>(
  items: T[],
  id: string,
): number {
  return items.findIndex((i) => i.id === id);
}
