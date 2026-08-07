import { useCallback, useEffect, useState } from "react";
import { TrackTable } from "./TrackTable";
import { useFilterContext } from "../hooks/useFilterContext";
import { EMPTY_FILTERS } from "../lib/filters";
import { useDialog } from "../hooks/useDialog";
import { useToast } from "./Toast";
import { DeleteFromDiskDialog } from "./DeleteFromDiskDialog";
import {
  archiveTracks,
  clearIncoming,
  listIncomingTracks,
  markIncomingReviewed,
} from "../ipc";
import type { Track } from "../types";

interface Props {
  libraryPath: string;
  selectedTrackIds: Set<string>;
  onSelectionChange: (ids: Set<string>) => void;
  onSelect: (track: Track) => void;
  onTrackContextMenu?: (track: Track, anchor: { x: number; y: number }) => void;
}

export function IncomingView({
  libraryPath,
  selectedTrackIds,
  onSelectionChange,
  onSelect,
  onTrackContextMenu,
}: Props) {
  const dialog = useDialog();
  const { toast } = useToast();
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  /**
   * Triage's third outcome.
   *
   * Incoming already has "keep" (Selected done) and "put away" (Archive
   * selected). The spec's third is "this was a mistake, get rid of it", which
   * is the case where a file has never been in a playlist and never been
   * played — so the guards almost always pass and this is where it is least
   * dangerous. It still goes through the same preview and the same quarantine.
   */
  const [deleting, setDeleting] = useState(false);
  const { ctx: filterCtx } = useFilterContext(libraryPath);

  const refresh = useCallback(async () => {
    if (!libraryPath) return;
    setLoading(true);
    try {
      const rows = await listIncomingTracks(libraryPath);
      setTracks(rows);
    } catch (e) {
      toast({ variant: "error", message: "Failed to load incoming tracks", detail: String(e) });
    } finally {
      setLoading(false);
    }
  }, [libraryPath, toast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleClear = async () => {
    if (tracks.length === 0) return;
    const ok = await dialog.confirm({
      title: `Clear incoming inbox?`,
      body: `Marks ${tracks.length} track(s) as reviewed. They will only reappear here if added again after this point.`,
      confirmLabel: "Clear",
    });
    if (!ok) return;
    await clearIncoming(libraryPath);
    onSelectionChange(new Set());
    await refresh();
  };

  /**
   * `Selected done` — the detail that makes triage fast.
   *
   * Marks the selection reviewed and then **immediately selects the next track
   * in the list**, so a whole inbox can be cleared with one repeated keystroke
   * rather than a click-then-reach-for-the-mouse cycle per track. The next
   * track is chosen from the list as it was *before* removal, so it is the one
   * that visually follows what the user was just looking at.
   */
  const handleSelectedDone = useCallback(async () => {
    if (selectedTrackIds.size === 0) return;
    const done = [...selectedTrackIds];
    const lastIndex = Math.max(
      ...done.map((id) => tracks.findIndex((t) => t.id === id)),
    );
    const next = tracks.slice(lastIndex + 1).find((t) => !selectedTrackIds.has(t.id));

    try {
      await markIncomingReviewed(libraryPath, done);
    } catch (e) {
      toast({ variant: "error", message: "Could not mark reviewed", detail: String(e) });
      return;
    }

    if (next) {
      onSelectionChange(new Set([next.id]));
      onSelect(next);
    } else {
      onSelectionChange(new Set());
    }
    await refresh();
  }, [selectedTrackIds, tracks, libraryPath, onSelectionChange, onSelect, refresh, toast]);

  // Bound here rather than in the action registry because it only means
  // anything while this view is open and something is selected.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "d" || e.metaKey || e.ctrlKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        return;
      }
      e.preventDefault();
      void handleSelectedDone();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [handleSelectedDone]);

  const handleArchiveSelected = async () => {
    if (selectedTrackIds.size === 0) return;
    await archiveTracks(libraryPath, [...selectedTrackIds]);
    onSelectionChange(new Set());
    toast({ variant: "success", message: `Archived ${selectedTrackIds.size} track(s).` });
    await refresh();
  };

  return (
    <div className="flex min-w-0 flex-1 flex-col bg-base animate-in fade-in duration-200">
      <header className="flex shrink-0 items-start justify-between border-b border-edge/60 px-6 py-5">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-ink">Incoming</h1>
          <p className="mt-1 text-[13px] text-ink-secondary">
            {loading
              ? "Loading…"
              : `${tracks.length} new track${tracks.length === 1 ? "" : "s"} since you last cleared.`}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => void handleSelectedDone()}
            disabled={selectedTrackIds.size === 0}
            title="Mark reviewed and jump to the next track (D)"
            className="rounded bg-elevated px-3 py-1 text-sm text-ink hover:bg-edge disabled:opacity-50"
          >
            Selected done ({selectedTrackIds.size})
          </button>
          <button
            onClick={handleArchiveSelected}
            disabled={selectedTrackIds.size === 0}
            className="rounded bg-elevated px-3 py-1 text-sm text-ink hover:bg-edge disabled:opacity-50"
          >
            Archive selected ({selectedTrackIds.size})
          </button>
          <button
            onClick={() => setDeleting(true)}
            disabled={selectedTrackIds.size === 0}
            className="rounded border border-red-500/40 px-3 py-1 text-sm font-medium text-red-500 hover:bg-red-500/10 disabled:opacity-50"
          >
            Delete from disk
          </button>
          <button
            onClick={handleClear}
            disabled={tracks.length === 0}
            className="rounded bg-accent px-3 py-1 text-sm font-medium text-base hover:opacity-90 disabled:opacity-50"
          >
            Mark all reviewed
          </button>
        </div>
      </header>

      <TrackTable
        libraryPath={libraryPath}
        filters={EMPTY_FILTERS}
        filterCtx={filterCtx}
        selectedTrackIds={selectedTrackIds}
        onSelectionChange={onSelectionChange}
        onSelect={onSelect}
        onTrackContextMenu={onTrackContextMenu}
        tracksOverride={tracks}
      />

      {deleting && (
        <DeleteFromDiskDialog
          libraryPath={libraryPath}
          trackIds={[...selectedTrackIds]}
          reason="Incoming triage"
          onClose={() => setDeleting(false)}
          onDeleted={async (receipt) => {
            toast({
              variant: "success",
              message: `Moved ${receipt.manifest.entries.length} file(s) to the deleted-audio folder.`,
              detail: "Restore or empty the batch in Settings.",
            });
            onSelectionChange(new Set());
            await refresh();
          }}
        />
      )}
    </div>
  );
}
