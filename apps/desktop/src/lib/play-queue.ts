/**
 * The play queue — what plays after the track that is playing.
 *
 * Per `docs/lexicon/05-cues-player.md §Music player`. The transport in
 * `src-tauri/src/audio.rs` already plays one file and emits `playback-ended`
 * when it drains; this is the list that decides what happens next.
 *
 * Pure functions over an immutable queue, so the interesting behaviour —
 * advancing past a track that was removed while it played, shuffling only the
 * part that has not played yet — is testable without a `rodio` sink.
 *
 * **Track ids, not tracks.** The queue survives a library refresh, a filter
 * change and a re-sort; holding whole `Track` objects would pin stale copies of
 * rows the user has since edited.
 */

export interface QueueState {
  /** Track ids, in play order. */
  items: string[];
  /**
   * Index of the track currently playing, or `-1` before anything has started.
   *
   * Kept as an index into `items` rather than as a separate "now playing" field
   * so the queue is one list the user can see, with a marker in it — which is
   * what the panel draws.
   */
  currentIndex: number;
  /** Play the next item when the current one drains. */
  autoplay: boolean;
}

export const EMPTY_QUEUE: QueueState = {
  items: [],
  currentIndex: -1,
  autoplay: true,
};

/** The id playing right now, or `null`. */
export function currentId(queue: QueueState): string | null {
  return queue.items[queue.currentIndex] ?? null;
}

/**
 * Append to the end of the queue.
 *
 * Duplicates are allowed: queueing the same track twice in a set is a real
 * thing people do, and silently dropping the second one would look like the
 * click did not register.
 */
export function enqueue(queue: QueueState, ids: string[]): QueueState {
  if (ids.length === 0) return queue;
  return { ...queue, items: [...queue.items, ...ids] };
}

/** Insert immediately after the current track — "play next". */
export function enqueueNext(queue: QueueState, ids: string[]): QueueState {
  if (ids.length === 0) return queue;
  const at = queue.currentIndex + 1;
  return {
    ...queue,
    items: [...queue.items.slice(0, at), ...ids, ...queue.items.slice(at)],
  };
}

/**
 * Drop the item at `index`.
 *
 * Removing something *before* the current track shifts the marker back by one
 * so it still points at the same track — the alternative is that deleting a
 * played entry silently skips the next one.
 */
export function removeAt(queue: QueueState, index: number): QueueState {
  if (index < 0 || index >= queue.items.length) return queue;
  const items = queue.items.filter((_, i) => i !== index);
  let currentIndex = queue.currentIndex;
  if (index < currentIndex) currentIndex -= 1;
  else if (index === currentIndex) {
    // The playing track was removed. Leave the marker where it is so the next
    // advance picks up the item that slid into its place, and clamp so a
    // removal from the tail does not point past the end.
    currentIndex = Math.min(currentIndex, items.length - 1);
  }
  return { ...queue, items, currentIndex };
}

/** Move an item, keeping the marker on whatever is playing. */
export function moveItem(
  queue: QueueState,
  from: number,
  to: number,
): QueueState {
  if (
    from === to ||
    from < 0 ||
    to < 0 ||
    from >= queue.items.length ||
    to >= queue.items.length
  ) {
    return queue;
  }
  const playing = currentId(queue);
  const items = [...queue.items];
  const [moved] = items.splice(from, 1);
  items.splice(to, 0, moved);

  // Re-find the playing track by identity rather than arithmetic. With
  // duplicates in the queue that is ambiguous, so bias to the old index —
  // which is the one the user was looking at.
  let currentIndex = queue.currentIndex;
  if (playing != null) {
    if (from === queue.currentIndex) currentIndex = to;
    else if (from < queue.currentIndex && to >= queue.currentIndex)
      currentIndex -= 1;
    else if (from > queue.currentIndex && to <= queue.currentIndex)
      currentIndex += 1;
  }
  return { ...queue, items, currentIndex };
}

/** Everything that has not played yet. */
export function upcoming(queue: QueueState): string[] {
  return queue.items.slice(queue.currentIndex + 1);
}

/**
 * Shuffle **only what has not played yet**.
 *
 * Shuffling the whole list would reorder history and move the marker under the
 * playing track, which reads as the queue losing its place mid-set.
 *
 * `random` is injectable so the shuffle is deterministic under test; the
 * default is `Math.random`.
 */
export function shuffleUpcoming(
  queue: QueueState,
  random: () => number = Math.random,
): QueueState {
  const head = queue.items.slice(0, queue.currentIndex + 1);
  const tail = [...queue.items.slice(queue.currentIndex + 1)];
  for (let i = tail.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [tail[i], tail[j]] = [tail[j], tail[i]];
  }
  return { ...queue, items: [...head, ...tail] };
}

/**
 * Clear the queue.
 *
 * Keeps the playing track as the sole entry rather than emptying outright —
 * `Clear Queue` means "nothing after this", not "stop the music". Stopping is
 * the transport's job and has its own button.
 */
export function clearQueue(queue: QueueState): QueueState {
  const playing = currentId(queue);
  if (playing == null) return { ...queue, items: [], currentIndex: -1 };
  return { ...queue, items: [playing], currentIndex: 0 };
}

/** Where an advance would land, or `null` at the end of the queue. */
export function nextIndex(queue: QueueState): number | null {
  const next = queue.currentIndex + 1;
  return next < queue.items.length ? next : null;
}

/** Step forward. A no-op at the end, so autoplay stops rather than looping. */
export function advance(queue: QueueState): QueueState {
  const next = nextIndex(queue);
  return next == null ? queue : { ...queue, currentIndex: next };
}

/** Step back. A no-op at the start. */
export function rewind(queue: QueueState): QueueState {
  if (queue.currentIndex <= 0) return queue;
  return { ...queue, currentIndex: queue.currentIndex - 1 };
}

/**
 * Play `ids` now, replacing the queue.
 *
 * The double-click / `Play` path. Replacing rather than appending is what makes
 * playing a track from the browser feel like playing a track rather than
 * queueing one behind whatever was already lined up.
 */
export function playNow(queue: QueueState, ids: string[]): QueueState {
  if (ids.length === 0) return queue;
  return { ...queue, items: [...ids], currentIndex: 0 };
}

/** Jump the marker to an entry the user clicked in the panel. */
export function jumpTo(queue: QueueState, index: number): QueueState {
  if (index < 0 || index >= queue.items.length) return queue;
  return { ...queue, currentIndex: index };
}
