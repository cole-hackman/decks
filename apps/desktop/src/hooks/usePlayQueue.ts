import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  EMPTY_QUEUE,
  advance,
  clearQueue,
  currentId,
  enqueue,
  enqueueNext,
  jumpTo,
  moveItem,
  playNow,
  removeAt,
  rewind,
  shuffleUpcoming,
  type QueueState,
} from "../lib/play-queue";
import type { Track } from "../types";

/**
 * The play queue, bound to the transport.
 *
 * Per `docs/lexicon/05-cues-player.md §Music player`. `lib/play-queue` owns the
 * list arithmetic; this owns the two things it cannot: resolving ids back to
 * tracks, and starting playback when the marker moves.
 *
 * The queue is **in-memory and per-session**. A queue is what you are about to
 * play right now — persisting it across restarts would mean opening the app
 * tomorrow to last night's leftovers, which is not what anyone wants from a
 * queue. Playlists are the thing that persists.
 */
export interface PlayQueue {
  state: QueueState;
  /** The queued tracks, resolved and in order. Unknown ids are dropped. */
  tracks: Track[];
  currentTrack: Track | null;
  addToQueue: (tracks: Track[]) => void;
  playNextInQueue: (tracks: Track[]) => void;
  startPlaying: (tracks: Track[]) => void;
  playQueueIndex: (index: number) => void;
  removeFromQueue: (index: number) => void;
  reorderQueue: (from: number, to: number) => void;
  shuffle: () => void;
  clear: () => void;
  skipForward: () => void;
  skipBack: () => void;
  setAutoplay: (on: boolean) => void;
}

interface Options {
  /** Every track the app knows about, for resolving ids. */
  library: Track[];
  /** Starts the transport. Called whenever the marker lands on a new track. */
  play: (track: Track) => void | Promise<void>;
  /**
   * True when the current track has drained.
   *
   * The transport emits `playback-ended`; the caller turns that into this flag.
   * Passing the flag rather than a callback keeps the advance in one place, so
   * a rapid end-of-track cannot advance twice.
   */
  endedAt: number | null;
}

export function usePlayQueue({ library, play, endedAt }: Options): PlayQueue {
  const [state, setState] = useState<QueueState>(EMPTY_QUEUE);

  const byId = useMemo(() => {
    const map = new Map<string, Track>();
    for (const t of library) map.set(t.id, t);
    return map;
  }, [library]);

  const tracks = useMemo(
    () =>
      state.items
        .map((id) => byId.get(id))
        .filter((t): t is Track => t != null),
    [state.items, byId],
  );

  const nowPlayingId = currentId(state);
  const currentTrack = nowPlayingId != null ? byId.get(nowPlayingId) ?? null : null;

  /**
   * The id we last handed to the transport.
   *
   * Without it, any re-render that changes `byId` (a library refetch, an edit)
   * would look like a new track and restart playback from zero.
   */
  const playingRef = useRef<string | null>(null);

  useEffect(() => {
    if (nowPlayingId == null || nowPlayingId === playingRef.current) return;
    const track = byId.get(nowPlayingId);
    if (!track) return;
    playingRef.current = nowPlayingId;
    void play(track);
  }, [nowPlayingId, byId, play]);

  // Advance when the current track drains, but only if autoplay is on. At the
  // end of the queue `advance` is a no-op, so playback simply stops.
  const lastEnded = useRef<number | null>(null);
  useEffect(() => {
    if (endedAt == null || endedAt === lastEnded.current) return;
    lastEnded.current = endedAt;
    setState((q) => (q.autoplay ? advance(q) : q));
  }, [endedAt]);

  const ids = useCallback((list: Track[]) => list.map((t) => t.id), []);

  return {
    state,
    tracks,
    currentTrack,
    addToQueue: useCallback(
      (list) => setState((q) => enqueue(q, ids(list))),
      [ids],
    ),
    playNextInQueue: useCallback(
      (list) => setState((q) => enqueueNext(q, ids(list))),
      [ids],
    ),
    startPlaying: useCallback(
      (list) => setState((q) => playNow(q, ids(list))),
      [ids],
    ),
    playQueueIndex: useCallback((index) => setState((q) => jumpTo(q, index)), []),
    removeFromQueue: useCallback(
      (index) => setState((q) => removeAt(q, index)),
      [],
    ),
    reorderQueue: useCallback(
      (from, to) => setState((q) => moveItem(q, from, to)),
      [],
    ),
    shuffle: useCallback(() => setState((q) => shuffleUpcoming(q)), []),
    clear: useCallback(() => setState((q) => clearQueue(q)), []),
    skipForward: useCallback(() => setState((q) => advance(q)), []),
    skipBack: useCallback(() => setState((q) => rewind(q)), []),
    setAutoplay: useCallback(
      (on) => setState((q) => ({ ...q, autoplay: on })),
      [],
    ),
  };
}
