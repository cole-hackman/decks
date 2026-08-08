import { useState } from "react";
import { FolderSearch, Check } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  applyMergeRelocate,
  classifyRelocateTarget,
  planMergeRelocate,
  relocateScan,
  stageChange,
} from "../ipc";
import type { RelocateCandidate } from "../types";
import { useQueryClient } from "@tanstack/react-query";

interface Props {
  libraryPath: string;
}

export function RelocateBanner({ libraryPath }: Props) {
  const [scanning, setScanning] = useState(false);
  const [candidates, setCandidates] = useState<RelocateCandidate[]>([]);
  const [hasScanned, setHasScanned] = useState(false);
  /**
   * A relocate whose target file is already claimed by another library entry.
   *
   * Per `docs/lexicon/07-health.md`, that is not a refusal — it is a
   * choose-which-to-keep merge, and it doubles as the mechanism for turning a
   * streaming entry into a local file.
   */
  const [merge, setMerge] = useState<{
    trackId: string;
    newPath: string;
    existingId: string;
    existingLabel: string;
  } | null>(null);
  const queryClient = useQueryClient();

  const handleScan = async () => {
    const selected = await open({
      directory: true,
      multiple: true,
      title: "Select music folders to scan",
    });

    if (!selected || selected.length === 0) return;

    setScanning(true);
    setHasScanned(false);
    try {
      const roots = Array.isArray(selected) ? selected : [selected];
      const results = await relocateScan(libraryPath, roots as string[]);
      setCandidates(results);
      setHasScanned(true);
    } catch (err) {
      console.error("Relocate scan failed:", err);
    } finally {
      setScanning(false);
    }
  };

  const afterStage = async () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: ["staged-changes", libraryPath] }),
      queryClient.invalidateQueries({ queryKey: ["library", libraryPath] }),
      queryClient.invalidateQueries({
        queryKey: ["tracks-with-missing-files", libraryPath],
      }),
    ]);

  const runMerge = async (keepMoving: boolean) => {
    if (!merge) return;
    try {
      const plan = await planMergeRelocate(
        libraryPath,
        merge.trackId,
        merge.existingId,
        merge.newPath,
        keepMoving,
      );
      await applyMergeRelocate(libraryPath, plan);
      setCandidates((prev) => prev.filter((c) => c.track_id !== merge.trackId));
      setMerge(null);
      await afterStage();
    } catch (err) {
      console.error("Failed to merge relocation:", err);
    }
  };

  const handleApply = async (
    trackId: string,
    oldPath: string,
    newPath: string,
  ) => {
    try {
      // Ask before staging. Two library rows pointing at one file is the state
      // the spec's constraint exists to prevent, and the merge is the way out
      // of it rather than an error message.
      const target = await classifyRelocateTarget(libraryPath, trackId, newPath);
      if (typeof target === "object" && "Occupied" in target) {
        const { track_id, title, artist } = target.Occupied;
        setMerge({
          trackId,
          newPath,
          existingId: track_id,
          existingLabel: artist ? `${artist} — ${title}` : title,
        });
        return;
      }
      await stageChange({
        library_path: libraryPath,
        // Not a TrackMetadataEdit: "folder_path" is not a djmdContent column
        // and was rejected by the applier's allowlist, so relocations staged
        // here never actually applied. TrackRelocate writes FolderPath (and the
        // filename columns, when the database has them).
        kind: "TrackRelocate",
        target_id: trackId,
        field: null,
        old_value: oldPath,
        new_value: {
          folder_path: newPath,
          file_name: newPath.split(/[\\/]/).pop() ?? null,
        },
        reason: "Relocated missing file via manual UI selection",
        confidence: 1.0,
      });
      setCandidates((prev) => prev.filter((c) => c.track_id !== trackId));
      await afterStage();
    } catch (err) {
      console.error("Failed to stage relocation:", err);
    }
  };

  return (
    <div className="flex shrink-0 flex-col border-b border-edge bg-surface/50">
      {merge && (
        <div
          role="dialog"
          aria-label="File already in library"
          data-testid="relocate-merge"
          className="border-b border-edge bg-base px-4 py-3 text-[12px]"
        >
          <p className="text-ink">
            That file is already in your library as{" "}
            <strong>{merge.existingLabel}</strong>. Keeping both would leave two
            entries pointing at one file, so one of them has to go — the other
            is replaced everywhere it appears in a playlist.
          </p>
          <div className="mt-2 flex gap-2">
            <button
              type="button"
              onClick={() => runMerge(true)}
              className="rounded bg-accent px-2 py-1 font-medium text-base hover:bg-accent-hover"
            >
              Keep the missing entry
            </button>
            <button
              type="button"
              onClick={() => runMerge(false)}
              className="rounded border border-edge px-2 py-1 text-ink hover:border-edge-strong"
            >
              Keep {merge.existingLabel}
            </button>
            <button
              type="button"
              onClick={() => setMerge(null)}
              className="ml-auto text-ink-muted hover:text-ink"
            >
              Cancel
            </button>
          </div>
        </div>
      )}
      <div className="flex items-center justify-between px-4 py-3">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-status-warn/20 text-status-warn">
            <FolderSearch className="h-4 w-4" />
          </div>
          <div>
            <h3 className="text-[13px] font-semibold text-ink">
              Missing Files Relocation
            </h3>
            <p className="text-[11px] text-ink-secondary">
              Select your music folders to scan for missing tracks.
            </p>
          </div>
        </div>
        <button
          onClick={handleScan}
          disabled={scanning}
          className="flex h-8 items-center gap-2 rounded-md bg-accent px-3 text-[12px] font-medium text-base hover:bg-accent-hover disabled:opacity-50 transition-colors"
        >
          {scanning ? (
            <>
              <span className="h-3 w-3 animate-spin rounded-full border border-base border-t-transparent" />
              Scanning...
            </>
          ) : (
            "Scan Folders"
          )}
        </button>
      </div>

      {hasScanned && candidates.length === 0 && (
        <div className="border-t border-edge/50 px-4 py-3 text-center text-[12px] text-ink-secondary">
          No missing files were found in the selected directories.
        </div>
      )}

      {candidates.length > 0 && (
        <div className="max-h-64 overflow-y-auto border-t border-edge/50">
          <ul className="divide-y divide-edge/30">
            {candidates.map((cand) => {
              const bestMatch = cand.matches[0];
              if (!bestMatch) return null;

              return (
                <li key={cand.track_id} className="flex items-center justify-between px-4 py-3 hover:bg-elevated/30">
                  <div className="flex min-w-0 flex-1 flex-col gap-1 pr-4">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-mono text-[10px] text-status-warn line-through">
                        {cand.original_path}
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <svg viewBox="0 0 16 16" fill="currentColor" className="h-3 w-3 shrink-0 text-accent-hover">
                        <path d="M4.715 6.542L3.343 7.914a3 3 0 104.243 4.243l1.828-1.829A3 3 0 008.586 5.5L8 6.086a1.002 1.002 0 00-.154.199 2 2 0 01.861 3.337L6.88 11.45a2 2 0 11-2.83-2.83l.793-.792a4.018 4.018 0 01-.128-1.287z" />
                      </svg>
                      <span className="truncate font-mono text-[11px] text-accent-hover">
                        {bestMatch.path}
                      </span>
                    </div>
                    <div className="flex items-center gap-1.5 mt-0.5">
                      <span className="text-[10px] text-ink-faint">Matched on:</span>
                      {bestMatch.reasons.map((reason) => (
                        <span key={reason} className="rounded-full border border-edge bg-base px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-ink-muted">
                          {reason}
                        </span>
                      ))}
                    </div>
                  </div>
                  <button
                    onClick={() =>
                      handleApply(cand.track_id, cand.original_path, bestMatch.path)
                    }
                    className="flex h-7 shrink-0 items-center gap-1.5 rounded-md border border-accent/40 bg-accent/10 px-2.5 text-[11px] font-medium text-accent-hover transition-colors hover:bg-accent hover:text-base"
                  >
                    <Check className="h-3 w-3" />
                    Accept
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  );
}
