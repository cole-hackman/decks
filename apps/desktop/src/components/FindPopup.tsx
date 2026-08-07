import { useEffect, useMemo, useRef, useState } from "react";
import { ListMusicIcon, PlayIcon, PlusIcon, SparklesIcon } from "lucide-react";
import { findAll, flatten, type FindResult } from "../lib/find";
import type { Playlist, Smartlist, Track } from "../types";

interface Props {
  open: boolean;
  onClose: () => void;
  tracks: Track[];
  playlists: Playlist[];
  smartlists: Smartlist[];
  /** Tracks currently selected in the browser — what "Add to playlist" adds. */
  selectedTracks: Track[];
  onPlayTrack: (track: Track) => void;
  onQueueTrack: (track: Track) => void;
  onOpenPlaylist: (id: string) => void;
  onOpenSmartlist: (id: string) => void;
  /** Adds the current selection to a playlist. Absent when nothing is selected. */
  onAddSelectionToPlaylist?: (playlistId: string, name: string) => void;
}

const SECTION_LABEL: Record<FindResult["kind"], string> = {
  playlist: "Playlists",
  smartlist: "Smartlists",
  track: "Tracks",
};

function KindIcon({ kind }: { kind: FindResult["kind"] }) {
  const className = "h-3.5 w-3.5 shrink-0 text-ink-faint";
  if (kind === "playlist") return <ListMusicIcon className={className} />;
  if (kind === "smartlist") return <SparklesIcon className={className} />;
  return <PlayIcon className={className} />;
}

/**
 * Find Popup — `Cmd/Ctrl+F`.
 *
 * Per `docs/lexicon/00-overview.md §Find Popup`. Deliberately *not* the Action
 * Center: `Cmd+K` searches commands, this searches **content**. Keeping them
 * separate means neither has to rank a track title against "Toggle Sidepanel",
 * which is a comparison with no sensible answer.
 *
 * `Enter` plays the highlighted result — or opens it, when the result is a
 * container. The per-result buttons are for the actions `Enter` cannot express.
 */
export function FindPopup({
  open,
  onClose,
  tracks,
  playlists,
  smartlists,
  selectedTracks,
  onPlayTrack,
  onQueueTrack,
  onOpenPlaylist,
  onOpenSmartlist,
  onAddSelectionToPlaylist,
}: Props) {
  const [query, setQuery] = useState("");
  const [highlight, setHighlight] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const grouped = useMemo(
    () => findAll(query, { tracks, playlists, smartlists }),
    [query, tracks, playlists, smartlists],
  );
  const flat = useMemo(() => flatten(grouped), [grouped]);
  const trackById = useMemo(() => {
    const map = new Map<string, Track>();
    for (const t of tracks) map.set(t.id, t);
    return map;
  }, [tracks]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setHighlight(0);
    inputRef.current?.focus();
  }, [open]);

  // Keep the highlight inside the result set as the query narrows it.
  useEffect(() => {
    setHighlight((h) => (h >= flat.length ? 0 : h));
  }, [flat.length]);

  if (!open) return null;

  const activate = (result: FindResult) => {
    if (result.kind === "playlist") onOpenPlaylist(result.id);
    else if (result.kind === "smartlist") onOpenSmartlist(result.id);
    else {
      const track = trackById.get(result.id);
      if (track) onPlayTrack(track);
    }
    onClose();
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setHighlight((h) => Math.min(flat.length - 1, h + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setHighlight((h) => Math.max(0, h - 1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const result = flat[highlight];
      if (result) activate(result);
    }
  };

  let index = -1;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Find"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={onKeyDown}
        className="flex max-h-[60vh] w-full max-w-xl flex-col overflow-hidden rounded-lg border border-edge bg-surface shadow-xl"
      >
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Find playlists, smartlists and tracks…"
          aria-label="Find in library"
          className="shrink-0 border-b border-edge bg-transparent px-4 py-3 text-sm text-ink outline-none placeholder:text-ink-faint"
        />

        <div className="min-h-0 flex-1 overflow-y-auto" data-testid="find-results">
          {query.trim() === "" ? (
            <p className="px-4 py-6 text-center text-[12px] text-ink-faint">
              Start typing to search your library.
            </p>
          ) : flat.length === 0 ? (
            <p
              className="px-4 py-6 text-center text-[12px] text-ink-muted"
              data-testid="find-empty"
            >
              Nothing matches “{query.trim()}”.
            </p>
          ) : (
            (["playlist", "smartlist", "track"] as const).map((kind) => {
              const results = grouped[kind];
              if (results.length === 0) return null;
              return (
                <section key={kind}>
                  <h3 className="px-4 pt-3 pb-1 font-mono text-[10px] uppercase tracking-wider text-ink-faint">
                    {SECTION_LABEL[kind]}
                  </h3>
                  <ul>
                    {results.map((result) => {
                      index += 1;
                      const myIndex = index;
                      const active = myIndex === highlight;
                      const track =
                        result.kind === "track"
                          ? trackById.get(result.id)
                          : undefined;
                      return (
                        <li
                          key={`${result.kind}-${result.id}`}
                          aria-selected={active}
                          role="option"
                          onMouseEnter={() => setHighlight(myIndex)}
                          className={[
                            "group flex items-center gap-2 px-4 py-1.5 text-[13px]",
                            active ? "bg-accent/12 text-ink" : "text-ink-secondary",
                          ].join(" ")}
                        >
                          <KindIcon kind={result.kind} />
                          <button
                            type="button"
                            onClick={() => activate(result)}
                            className="min-w-0 flex-1 truncate text-left"
                          >
                            {result.label}
                            {result.sublabel && (
                              <span className="ml-2 text-[11px] text-ink-faint">
                                {result.sublabel}
                              </span>
                            )}
                          </button>

                          {result.kind === "track" && track && (
                            <button
                              type="button"
                              onClick={() => {
                                onQueueTrack(track);
                                onClose();
                              }}
                              aria-label={`Add ${result.label} to queue`}
                              title="Add to queue"
                              // Visible on the highlighted row as well as on
                              // hover: this popup is driven by the keyboard,
                              // and a hover-only action is one the keyboard
                              // cannot reach.
                              className={[
                                "shrink-0 rounded border border-edge px-1.5 py-0.5 text-[11px] text-ink-secondary hover:border-edge-strong hover:text-ink",
                                active ? "block" : "hidden group-hover:block",
                              ].join(" ")}
                            >
                              Queue
                            </button>
                          )}

                          {result.kind === "playlist" &&
                            onAddSelectionToPlaylist && (
                              <button
                                type="button"
                                onClick={() => {
                                  onAddSelectionToPlaylist(
                                    result.id,
                                    result.label,
                                  );
                                  onClose();
                                }}
                                aria-label={`Add ${selectedTracks.length} selected track(s) to ${result.label}`}
                                title={`Add ${selectedTracks.length} selected track(s)`}
                                className={[
                                  "shrink-0 items-center gap-0.5 rounded border border-edge px-1.5 py-0.5 text-[11px] text-ink-secondary hover:border-edge-strong hover:text-ink",
                                  active ? "flex" : "hidden group-hover:flex",
                                ].join(" ")}
                              >
                                <PlusIcon className="h-2.5 w-2.5" />
                                {selectedTracks.length}
                              </button>
                            )}
                        </li>
                      );
                    })}
                  </ul>
                </section>
              );
            })
          )}
        </div>

        <footer className="shrink-0 border-t border-edge px-4 py-1.5 text-[10px] text-ink-faint">
          <kbd>↑↓</kbd> move · <kbd>Enter</kbd> play or open · <kbd>Esc</kbd>{" "}
          close
        </footer>
      </div>
    </div>
  );
}
