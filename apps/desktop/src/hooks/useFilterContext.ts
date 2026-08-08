import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import {
  listTracksWithCues,
  listTracksInAnyPlaylist,
  listTracksWithMissingFiles,
  listTrackTagsMap,
} from "../ipc";
import type { FilterContext } from "../lib/filters";

/**
 * Library-wide lookups used by the filter system. Cached per library path;
 * the user manually refreshes by changing libraries or restarting the app.
 *
 * `enableMissingFiles` lazy-loads the on-disk existence check — it scans every
 * track's `folder_path` against the filesystem, which is expensive on large
 * libraries, so we only run it once the user enables the filter.
 */
export function useFilterContext(
  libraryPath: string | null,
  enableMissingFiles = false,
): {
  ctx: FilterContext;
  isLoading: boolean;
  missingFilesLoading: boolean;
} {
  const cues = useQuery<string[], Error>({
    queryKey: ["tracks-with-cues", libraryPath],
    queryFn: () => listTracksWithCues(libraryPath!),
    enabled: libraryPath !== null,
    staleTime: Infinity,
  });

  const inPlaylist = useQuery<string[], Error>({
    queryKey: ["tracks-in-any-playlist", libraryPath],
    queryFn: () => listTracksInAnyPlaylist(libraryPath!),
    enabled: libraryPath !== null,
    staleTime: Infinity,
  });

  const missingFiles = useQuery<string[], Error>({
    queryKey: ["tracks-with-missing-files", libraryPath],
    queryFn: () => listTracksWithMissingFiles(libraryPath!),
    enabled: libraryPath !== null && enableMissingFiles,
    // Five minutes, matching the shell's own memo. `Infinity` here would mean
    // a file restored on disk stayed marked missing until the app restarted —
    // the shell would happily re-scan, but nothing would ever ask it to.
    staleTime: 5 * 60 * 1000,
  });

  const tagBindings = useQuery<Record<string, string[]>, Error>({
    queryKey: ["track-tags-map", libraryPath],
    queryFn: () => listTrackTagsMap(libraryPath!),
    enabled: libraryPath !== null,
    staleTime: Infinity,
  });

  const ctx = useMemo<FilterContext>(
    () => ({
      tracksWithCues: new Set(cues.data ?? []),
      tracksInAnyPlaylist: new Set(inPlaylist.data ?? []),
      tracksWithMissingFiles: new Set(missingFiles.data ?? []),
      tagsByTrack: tagBindings.data
        ? new Map(
            Object.entries(tagBindings.data).map(
              ([trackId, ids]) => [trackId, new Set(ids)],
            ),
          )
        : new Map(),
    }),
    [cues.data, inPlaylist.data, missingFiles.data, tagBindings.data],
  );

  return {
    ctx,
    isLoading: cues.isLoading || inPlaylist.isLoading,
    missingFilesLoading: missingFiles.isLoading || missingFiles.isFetching,
  };
}
