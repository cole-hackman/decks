import {
  ChevronDownIcon,
  ChevronUpIcon,
  PlayIcon,
  ShuffleIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";
import type { PlayQueue } from "../hooks/usePlayQueue";
import type { Track } from "../types";

interface Props {
  queue: PlayQueue;
  /** "View in collection" — reveals the queued track in the browser. */
  onReveal?: (track: Track) => void;
  onClose?: () => void;
}

function label(track: Track): string {
  return track.artist ? `${track.artist} — ${track.title}` : track.title;
}

/**
 * The play queue.
 *
 * Per `docs/lexicon/05-cues-player.md §Music player`. One list with a marker in
 * it rather than a "now playing" box above a separate list — the queue is the
 * thing the user reasons about, and splitting it in two makes "what comes after
 * this" harder to read, not easier.
 *
 * Reordering is up/down buttons rather than drag-and-drop. The table has no
 * drag source yet, so a drop target here would be half a feature; buttons work
 * with the keyboard and can be replaced by dnd-kit later without changing the
 * queue's semantics.
 */
export function PlayQueuePanel({ queue, onReveal, onClose }: Props) {
  const { state, tracks } = queue;
  const upcomingCount = Math.max(0, tracks.length - state.currentIndex - 1);

  return (
    <aside
      className="flex h-full w-72 shrink-0 flex-col border-l border-edge bg-surface"
      aria-label="Play queue"
    >
      <header className="flex shrink-0 items-center justify-between border-b border-edge px-3 py-2">
        <div>
          <h2 className="text-sm font-semibold text-ink">Queue</h2>
          <p className="text-[11px] text-ink-faint">
            {tracks.length === 0
              ? "Nothing queued"
              : `${upcomingCount} up next of ${tracks.length}`}
          </p>
        </div>
        {onClose && (
          <button
            type="button"
            onClick={onClose}
            aria-label="Close queue"
            className="text-ink-faint hover:text-ink"
          >
            <XIcon className="h-4 w-4" />
          </button>
        )}
      </header>

      <div className="flex shrink-0 items-center gap-2 border-b border-edge px-3 py-2 text-[11px]">
        <label className="flex items-center gap-1 text-ink-secondary">
          <input
            type="checkbox"
            checked={state.autoplay}
            onChange={(e) => queue.setAutoplay(e.target.checked)}
          />
          Autoplay
        </label>
        <div className="ml-auto flex gap-1">
          <button
            type="button"
            onClick={queue.shuffle}
            disabled={upcomingCount < 2}
            title="Shuffle what has not played yet"
            className="flex items-center gap-1 rounded border border-edge px-1.5 py-0.5 text-ink-secondary hover:border-edge-strong hover:text-ink disabled:opacity-40"
          >
            <ShuffleIcon className="h-3 w-3" />
            Shuffle
          </button>
          <button
            type="button"
            onClick={queue.clear}
            disabled={tracks.length === 0}
            title="Remove everything after the current track"
            className="flex items-center gap-1 rounded border border-edge px-1.5 py-0.5 text-ink-secondary hover:border-edge-strong hover:text-ink disabled:opacity-40"
          >
            <Trash2Icon className="h-3 w-3" />
            Clear
          </button>
        </div>
      </div>

      {tracks.length === 0 ? (
        <p className="p-3 text-[12px] text-ink-muted">
          Right-click a track and choose <em>Add to queue</em>.
        </p>
      ) : (
        <ol className="min-h-0 flex-1 overflow-y-auto" data-testid="queue-list">
          {tracks.map((track, index) => {
            const isCurrent = index === state.currentIndex;
            const played = index < state.currentIndex;
            return (
              <li
                key={`${track.id}-${index}`}
                aria-current={isCurrent ? "true" : undefined}
                className={[
                  "group flex items-center gap-1 border-b border-edge/40 px-2 py-1 text-[12px]",
                  isCurrent
                    ? "bg-accent/12 text-ink"
                    : played
                      ? "text-ink-faint"
                      : "text-ink-secondary",
                ].join(" ")}
              >
                <button
                  type="button"
                  onClick={() => queue.playQueueIndex(index)}
                  title="Play now"
                  aria-label={`Play ${label(track)}`}
                  className="shrink-0 text-ink-faint hover:text-accent"
                >
                  <PlayIcon className="h-3 w-3" />
                </button>
                <button
                  type="button"
                  onClick={() => onReveal?.(track)}
                  disabled={!onReveal}
                  title={onReveal ? "View in collection" : undefined}
                  className="min-w-0 flex-1 truncate text-left disabled:cursor-default"
                >
                  {label(track)}
                </button>
                <span className="hidden shrink-0 gap-0.5 group-hover:flex">
                  <button
                    type="button"
                    onClick={() => queue.reorderQueue(index, index - 1)}
                    disabled={index === 0}
                    aria-label={`Move ${label(track)} up`}
                    className="text-ink-faint hover:text-ink disabled:opacity-30"
                  >
                    <ChevronUpIcon className="h-3 w-3" />
                  </button>
                  <button
                    type="button"
                    onClick={() => queue.reorderQueue(index, index + 1)}
                    disabled={index === tracks.length - 1}
                    aria-label={`Move ${label(track)} down`}
                    className="text-ink-faint hover:text-ink disabled:opacity-30"
                  >
                    <ChevronDownIcon className="h-3 w-3" />
                  </button>
                  <button
                    type="button"
                    onClick={() => queue.removeFromQueue(index)}
                    aria-label={`Remove ${label(track)} from queue`}
                    className="text-ink-faint hover:text-red-500"
                  >
                    <XIcon className="h-3 w-3" />
                  </button>
                </span>
              </li>
            );
          })}
        </ol>
      )}
    </aside>
  );
}
