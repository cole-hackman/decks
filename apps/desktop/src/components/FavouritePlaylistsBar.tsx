import { useCallback, useEffect, useState } from "react";
import {
  addTracksToPlaylist,
  listFavouritePlaylists,
  toggleFavouritePlaylist,
} from "../ipc";
import { useToast } from "./Toast";
import { readDragPayload, TRACK_IDS_MIME } from "../lib/track-drag";
import type { FavouritePlaylist } from "../types";

interface Props {
  libraryPath: string;
  /** Tracks the hotkeys file. Frozen at press time by the caller. */
  selectedTrackIds: Set<string>;
  /** Open a favourite in the browser. */
  onOpenPlaylist: (playlistId: string) => void;
  /** Bumped by the caller to force a refetch after a star is toggled elsewhere. */
  refreshToken?: number;
}

/**
 * Favourite playlists — the spec's fast filing system.
 *
 * Per `docs/lexicon/02-library.md §Favorite Playlists`. Starred playlists pin
 * above the track browser with a hotkey each: **`1`–`9` jump to the playlist,
 * `Shift+1`–`9` file the current selection into it.**
 *
 * The hotkey is the position, not the playlist, and positions are stable across
 * sessions — un-starring closes the gap rather than leaving a hole, because a
 * key that quietly changes what it does between sessions is worse than one that
 * does nothing.
 *
 * **Not done:** drag-and-drop onto a favourite. The hotkeys cover the same
 * intent and the table has no drag source yet.
 */
export function FavouritePlaylistsBar({
  libraryPath,
  selectedTrackIds,
  onOpenPlaylist,
  refreshToken = 0,
}: Props) {
  const { toast } = useToast();
  const [favourites, setFavourites] = useState<FavouritePlaylist[]>([]);

  const refresh = useCallback(() => {
    if (!libraryPath) return;
    listFavouritePlaylists(libraryPath)
      .then((got) => setFavourites(Array.isArray(got) ? got : []))
      .catch(() => setFavourites([]));
  }, [libraryPath]);

  useEffect(refresh, [refresh, refreshToken]);

  /** Which favourite is under the pointer during a drag. */
  const [dropTarget, setDropTarget] = useState<string | null>(null);

  const file = useCallback(
    async (fav: FavouritePlaylist, ids: string[]) => {
      if (ids.length === 0) {
        toast({ variant: "info", message: "Nothing selected to file." });
        return;
      }
      try {
        const staged = await addTracksToPlaylist(libraryPath, fav.playlist_id, ids);
        const already = ids.length - staged.length;
        toast({
          variant: "success",
          message:
            staged.length === 0
              ? `Already in ${fav.name}.`
              : `Staged ${staged.length} track(s) for ${fav.name}.`,
          detail:
            already > 0 && staged.length > 0
              ? `${already} already there.`
              : undefined,
        });
        refresh();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [libraryPath, refresh, toast],
  );

  const unstar = useCallback(
    async (fav: FavouritePlaylist) => {
      try {
        await toggleFavouritePlaylist(libraryPath, fav.playlist_id);
        refresh();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [libraryPath, refresh, toast],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Never steal a digit from a text field, and never from a modified
      // chord that belongs to something else.
      const target = e.target as HTMLElement | null;
      if (
        target != null &&
        (target.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
      ) {
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      // `e.code` rather than `e.key`: with Shift held, `key` is "!" not "1".
      const match = /^Digit([1-9])$/.exec(e.code);
      if (!match) return;
      const fav = favourites.find((f) => f.seq === Number(match[1]));
      if (!fav) return;

      e.preventDefault();
      if (e.shiftKey) {
        void file(fav, Array.from(selectedTrackIds));
      } else {
        onOpenPlaylist(fav.playlist_id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [favourites, selectedTrackIds, file, onOpenPlaylist]);

  if (favourites.length === 0) return null;

  return (
    <div
      className="flex shrink-0 flex-wrap items-center gap-1 border-b border-border px-3 py-1.5 text-xs"
      aria-label="Favourite playlists"
      data-testid="favourite-playlists"
    >
      <span className="mr-1 text-[11px] text-muted">Favourites</span>
      {favourites.map((fav) => (
        <span
          key={fav.playlist_id}
          className={[
            "group flex items-center overflow-hidden rounded border",
            dropTarget === fav.playlist_id
              ? "border-accent ring-1 ring-inset ring-accent"
              : "border-border",
          ].join(" ")}
          onDragOver={(e) => {
            // Only accept a drag that carries our own payload. Without the
            // type check the bar would light up for a dragged file or a text
            // selection and then do nothing.
            if (!e.dataTransfer.types.includes(TRACK_IDS_MIME)) return;
            e.preventDefault();
            e.dataTransfer.dropEffect = "copy";
            setDropTarget(fav.playlist_id);
          }}
          onDragLeave={() => setDropTarget((c) => (c === fav.playlist_id ? null : c))}
          onDrop={(e) => {
            e.preventDefault();
            setDropTarget(null);
            const ids = readDragPayload(e.dataTransfer.getData(TRACK_IDS_MIME));
            // The drop carries its own ids rather than reading the current
            // selection: what was dragged is what gets filed, even if the
            // selection changed between the drag starting and the drop.
            if (ids.length > 0) void file(fav, ids);
          }}
        >
          <button
            type="button"
            className="px-2 py-0.5 hover:bg-surface-hover"
            title={`Press ${fav.seq} to open, Shift+${fav.seq} to file the selection`}
            onClick={() => onOpenPlaylist(fav.playlist_id)}
          >
            <span className="mr-1 tabular-nums text-muted">{fav.seq}</span>
            {fav.name}
            <span className="ml-1 tabular-nums text-muted">
              {fav.track_count}
            </span>
          </button>
          <button
            type="button"
            aria-label={`File selection into ${fav.name}`}
            className="border-l border-border px-1.5 py-0.5 text-muted hover:bg-surface-hover"
            onClick={() => void file(fav, Array.from(selectedTrackIds))}
          >
            +
          </button>
          <button
            type="button"
            aria-label={`Unstar ${fav.name}`}
            className="border-l border-border px-1.5 py-0.5 text-muted hover:bg-surface-hover"
            onClick={() => void unstar(fav)}
          >
            ✕
          </button>
        </span>
      ))}
      <span className="ml-1 text-[11px] text-muted">
        {/* Say what the keys do; a numbered chip alone reads as decoration. */}
        1–9 opens · Shift+1–9 or drag files the selection
      </span>
    </div>
  );
}
