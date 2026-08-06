import { useCallback, useMemo } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useToast } from "../components/Toast";
import type { TrackContextMenuAction } from "../components/TrackContextMenu";
import { CONTEXT_MENU_SEPARATOR } from "../components/TrackContextMenuItems";
import {
  archiveTracksFrom,
  analyzeTrack,
  libraryStageIntroCues,
  libraryStagePlaylistRemoveTrack,
  playTrack,
  revealInFinder,
} from "../ipc";
import type { Track } from "../types";

interface Options {
  libraryPath: string;
  /** Called when the user picks "Show details" so the inspector can be
   *  forced open if it was previously closed. */
  onShowDetails: (track: Track) => void;
  /** When the menu was opened from a playlist row, the playlist id of the
   *  parent playlist. Unlocks the "Remove from playlist" action. */
  playlistId?: string;
  /** Opens the tag picker for the right-clicked track. */
  onEditTags?: (track: Track) => void;
  /** Opens the Files view scoped to this track — the spec's "Send to". */
  onSendToFiles?: (track: Track) => void;
  /** Makes a new playlist out of the current selection. */
  onCreatePlaylist?: (track: Track) => void;
  /** Opens Mixable Tracks seeded from this track — the spec's
   *  "right-click → Track tools → Find mixable tracks". */
  onFindMixable?: (track: Track) => void;
}

/** Builds the right-click action set for a Track. Memoised so the menu
 *  component doesn't rebuild its items on every render. */
export function useTrackContextActions({
  libraryPath,
  onShowDetails,
  playlistId,
  onEditTags,
  onSendToFiles,
  onFindMixable,
  onCreatePlaylist,
}: Options): TrackContextMenuAction[] {
  const { toast } = useToast();
  const queryClient = useQueryClient();

  const doAnalyze = useCallback(
    (track: Track) => {
      if (!track.folder_path) {
        toast({
          variant: "error",
          message: "Cannot analyse",
          detail: "Track has no audio file path.",
        });
        return;
      }
      toast({ variant: "info", message: `Analysing ${track.title ?? "track"}…` });
      void analyzeTrack(libraryPath, track.id)
        .then((result) => {
          toast({
            variant: "success",
            message: `${track.title ?? "Track"} analysed`,
            detail: `BPM ${result.bpm.toFixed(1)} · Key ${result.musical_key} · ${(result.confidence * 100).toFixed(0)}% conf.`,
          });
        })
        .catch((e: unknown) => {
          toast({
            variant: "error",
            message: "Analysis failed",
            detail: e instanceof Error ? e.message : String(e),
          });
        });
    },
    [libraryPath, toast],
  );

  const doPlay = useCallback(
    (track: Track) => {
      if (!track.folder_path) {
        toast({ variant: "error", message: "No audio file to play" });
        return;
      }
      void playTrack(track.folder_path).catch((e: unknown) => {
        toast({
          variant: "error",
          message: "Could not start playback",
          detail: e instanceof Error ? e.message : String(e),
        });
      });
    },
    [toast],
  );

  const doReveal = useCallback(
    (track: Track) => {
      if (!track.folder_path) {
        toast({ variant: "error", message: "No file path to reveal" });
        return;
      }
      void revealInFinder(track.folder_path).catch((e: unknown) => {
        toast({
          variant: "error",
          message: "Could not reveal file",
          detail: e instanceof Error ? e.message : String(e),
        });
      });
    },
    [toast],
  );

  const doCopyPath = useCallback(
    (track: Track) => {
      const text = track.folder_path ?? "";
      if (!text) {
        toast({ variant: "error", message: "No file path to copy" });
        return;
      }
      void navigator.clipboard
        .writeText(text)
        .then(() =>
          toast({ variant: "success", message: "File path copied" }),
        )
        .catch(() => toast({ variant: "error", message: "Clipboard blocked" }));
    },
    [toast],
  );

  const doCopyId = useCallback(
    (track: Track) => {
      void navigator.clipboard
        .writeText(track.id)
        .then(() => toast({ variant: "success", message: "Track ID copied" }))
        .catch(() => toast({ variant: "error", message: "Clipboard blocked" }));
    },
    [toast],
  );

  const doRemoveFromPlaylist = useCallback(
    (track: Track) => {
      if (!playlistId) return;
      libraryStagePlaylistRemoveTrack(libraryPath, playlistId, track.id)
        .then(() => {
          void queryClient.invalidateQueries({ queryKey: ["staged-changes"] });
          toast({
            variant: "success",
            message: "Removal staged",
            detail: `“${track.title ?? "Track"}” will be removed on next export.`,
          });
        })
        .catch((e: unknown) => {
          toast({
            variant: "error",
            message: "Could not stage removal",
            detail: e instanceof Error ? e.message : String(e),
          });
        });
    },
    [libraryPath, playlistId, queryClient, toast],
  );

  /**
   * Archive, with the spec's context-sensitive playlist rule.
   *
   * From inside a playlist this also stages removal from **that** playlist and
   * leaves every other alone; from the browser it touches no playlist at all.
   * The asymmetry is the point: from a playlist you are saying "not in this
   * set", from the browser you are saying "not in my way".
   */
  const doArchive = useCallback(
    (track: Track) => {
      archiveTracksFrom(libraryPath, [track.id], playlistId ?? null)
        .then((result) => {
          void queryClient.invalidateQueries({ queryKey: ["staged-changes"] });
          toast({
            variant: "success",
            message: "Archived",
            detail:
              result.staged.length > 0
                ? "Removal from this playlist staged; other playlists are untouched."
                : "Still in every playlist it was in.",
          });
        })
        .catch((e: unknown) => {
          toast({
            variant: "error",
            message: "Could not archive",
            detail: e instanceof Error ? e.message : String(e),
          });
        });
    },
    [libraryPath, playlistId, queryClient, toast],
  );

  const doStageIntroCue = useCallback(
    (track: Track) => {
      libraryStageIntroCues(libraryPath, [track.id])
        .then((staged) => {
          void queryClient.invalidateQueries({ queryKey: ["staged-changes"] });
          toast({
            variant: "success",
            message: staged.length
              ? `Staged intro cue for ${track.title ?? "track"}`
              : "Nothing to stage — track already has an intro cue or no beat grid",
          });
        })
        .catch((e: unknown) => {
          toast({
            variant: "error",
            message: "Stage intro cue failed",
            detail: e instanceof Error ? e.message : String(e),
          });
        });
    },
    [libraryPath, queryClient, toast],
  );

  return useMemo<TrackContextMenuAction[]>(() => {
    const actions: TrackContextMenuAction[] = [
      {
        id: "show-details",
        label: "Show details",
        hint: "Enter",
        icon: (
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
            <path d="M2 2.5A1.5 1.5 0 013.5 1h9A1.5 1.5 0 0114 2.5v11A1.5 1.5 0 0112.5 15h-9A1.5 1.5 0 012 13.5v-11zM4 4v2h8V4H4zm0 3.5v1h8v-1H4zm0 2.5v1h5v-1H4z" />
          </svg>
        ),
        onSelect: onShowDetails,
      },
      {
        id: "play",
        label: "Play track",
        hint: "Space",
        icon: (
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
            <path d="M4 2.5a.5.5 0 01.76-.43l8 5.5a.5.5 0 010 .86l-8 5.5A.5.5 0 014 13.5v-11z" />
          </svg>
        ),
        onSelect: doPlay,
      },
      {
        id: "analyze",
        label: "Analyse audio",
        icon: (
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
            <path d="M2 8h2l1.5-4 3 8 2-5 1.5 3H14" />
          </svg>
        ),
        onSelect: doAnalyze,
      },
      ...(onEditTags
        ? [
            {
              id: "edit-tags",
              label: "Edit tags…",
              hint: "T",
              icon: (
                <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
                  <path d="M2 3a1 1 0 011-1h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 010 1.414l-5.586 5.586a1 1 0 01-1.414 0L2.293 9.293A1 1 0 012 8.586V3zm3 2a1 1 0 100 2 1 1 0 000-2z" />
                </svg>
              ),
              onSelect: (track: Track) => onEditTags(track),
            } as TrackContextMenuAction,
          ]
        : []),
      ...(onCreatePlaylist
        ? [
            {
              id: "create-playlist",
              label: "New playlist from selection…",
              icon: (
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" aria-hidden>
                  <path d="M2 4h8M2 8h5M2 12h5" />
                  <path d="M11 8.5v5M8.5 11h5" />
                </svg>
              ),
              onSelect: (track: Track) => onCreatePlaylist(track),
            } as TrackContextMenuAction,
          ]
        : []),
      ...(onFindMixable
        ? [
            {
              id: "find-mixable",
              label: "Find mixable tracks",
              hint: "By BPM and key",
              icon: (
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" aria-hidden>
                  <circle cx="7" cy="7" r="4.5" />
                  <path d="M10.5 10.5L14 14" />
                </svg>
              ),
              onSelect: (track: Track) => onFindMixable(track),
            } as TrackContextMenuAction,
          ]
        : []),
      ...(onSendToFiles
        ? [
            {
              id: "send-to-files",
              label: "Send to → Move files…",
              hint: "Moves on disk",
              icon: (
                <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
                  <path d="M1.5 3A1.5 1.5 0 013 1.5h3.586a1.5 1.5 0 011.06.44L8.708 3h4.792A1.5 1.5 0 0115 4.5v8A1.5 1.5 0 0113.5 14h-11A1.5 1.5 0 011 12.5v-9c0-.27.05-.526.14-.76L1.5 3zM7 8.5h3.5l-1.2-1.2.7-.7L12.7 9l-2.7 2.4-.7-.7L10.5 9.5H7v-1z" />
                </svg>
              ),
              onSelect: (track: Track) => onSendToFiles(track),
            } as TrackContextMenuAction,
          ]
        : []),
      {
        id: "stage-intro-cue",
        label: "Stage intro cue",
        hint: "Stages change",
        icon: (
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
            <path d="M3 2.5a.5.5 0 01.74-.44l9 5a.5.5 0 010 .88l-9 5A.5.5 0 013 12.5V8H1.5a.5.5 0 010-1H3V2.5z" />
          </svg>
        ),
        onSelect: doStageIntroCue,
      },
      CONTEXT_MENU_SEPARATOR,
      {
        id: "reveal",
        label: "Reveal in Finder",
        icon: (
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
            <path d="M1.5 3A1.5 1.5 0 013 1.5h3.586a1.5 1.5 0 011.06.44L8.708 3h4.792A1.5 1.5 0 0115 4.5v8A1.5 1.5 0 0113.5 14h-11A1.5 1.5 0 011 12.5v-9c0-.27.05-.526.14-.76L1.5 3z" />
          </svg>
        ),
        onSelect: doReveal,
      },
      {
        id: "copy-path",
        label: "Copy file path",
        icon: (
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
            <path d="M4 2.5A1.5 1.5 0 015.5 1h6A1.5 1.5 0 0113 2.5v8A1.5 1.5 0 0111.5 12h-6A1.5 1.5 0 014 10.5v-8zM2.5 4A1.5 1.5 0 001 5.5v8A1.5 1.5 0 002.5 15h6A1.5 1.5 0 0010 13.5V13H5.5A2.5 2.5 0 013 10.5V4h-.5z" />
          </svg>
        ),
        onSelect: doCopyPath,
      },
      {
        id: "copy-id",
        label: "Copy track ID",
        icon: (
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
            <path d="M2.5 3a.5.5 0 00-.5.5v9a.5.5 0 00.5.5h11a.5.5 0 00.5-.5v-9a.5.5 0 00-.5-.5h-11zm1.5 3h8v1H4V6zm0 3h5v1H4V9z" />
          </svg>
        ),
        onSelect: doCopyId,
      },
    ];

    actions.push(CONTEXT_MENU_SEPARATOR, {
      id: "archive",
      label: playlistId ? "Archive (and remove from this playlist)" : "Archive",
      hint: playlistId ? "Stages change" : "Hidden, not deleted",
      icon: (
        <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
          <path d="M2 3.5A.5.5 0 012.5 3h11a.5.5 0 01.5.5v1a.5.5 0 01-.5.5h-11a.5.5 0 01-.5-.5v-1zM3 6h10v6.5a.5.5 0 01-.5.5h-9a.5.5 0 01-.5-.5V6zm2.5 2a.5.5 0 000 1h5a.5.5 0 000-1h-5z" />
        </svg>
      ),
      onSelect: doArchive,
    });

    if (playlistId) {
      actions.push(CONTEXT_MENU_SEPARATOR, {
        id: "remove-from-playlist",
        label: "Remove from playlist",
        hint: "Stages change",
        destructive: true,
        icon: (
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
            <path d="M3 8h10" />
            <path d="M5.5 5.5L3 8l2.5 2.5" />
          </svg>
        ),
        onSelect: doRemoveFromPlaylist,
      });
    }

    return actions;
  }, [
    onShowDetails,
    doPlay,
    doAnalyze,
    doStageIntroCue,
    doReveal,
    doCopyPath,
    doCopyId,
    doRemoveFromPlaylist,
    doArchive,
    playlistId,
    onEditTags,
    onSendToFiles,
    onFindMixable,
    onCreatePlaylist,
  ]);
}
