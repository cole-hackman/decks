import { useCallback, useEffect, useMemo, useState } from "react";
import {
  listPlaylists,
  syncCheck,
  syncExecute,
  syncPreview,
  type CueDestination,
  type KeyFormat,
  type PendingChange,
  type SyncMode,
  type SyncOptions,
} from "../ipc";
import type { Playlist } from "../types";
import { useDialog } from "../hooks/useDialog";
import { useToast } from "./Toast";

interface Props {
  libraryPath: string;
}

export function SyncPanel({ libraryPath }: Props) {
  const dialog = useDialog();
  const { toast } = useToast();

  const [mode, setMode] = useState<SyncMode>("full");
  const [playlistId, setPlaylistId] = useState<string | null>(null);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [pending, setPending] = useState<PendingChange[]>([]);
  const [excluded, setExcluded] = useState<Set<string>>(new Set());
  const [locked, setLocked] = useState(false);
  const [loading, setLoading] = useState(false);
  const [applying, setApplying] = useState(false);

  // Sub-Plan 6: wired end-to-end through `changes::applier::SyncOptions`.
  const [cueDestination, setCueDestination] = useState<CueDestination>("hot");
  const [keepGrids, setKeepGrids] = useState(false);
  const [convertKeys, setConvertKeys] = useState<KeyFormat>("original");
  const [addLeadingZero, setAddLeadingZero] = useState(false);

  const options = useMemo<SyncOptions>(
    () => ({
      playlist_id: mode === "playlist" ? playlistId : null,
      cue_destination: cueDestination,
      keep_grids: keepGrids,
      convert_keys: convertKeys,
      add_leading_zero: addLeadingZero,
    }),
    [mode, playlistId, cueDestination, keepGrids, convertKeys, addLeadingZero],
  );

  const refresh = useCallback(async () => {
    if (!libraryPath) return;
    setLoading(true);
    try {
      const [check, rows] = await Promise.all([
        syncCheck(libraryPath),
        syncPreview(libraryPath, mode, options),
      ]);
      setLocked(check.locked);
      setPending(rows);
      setExcluded(new Set());
    } catch (e) {
      toast({ variant: "error", message: "Failed to load pending changes", detail: String(e) });
    } finally {
      setLoading(false);
    }
  }, [libraryPath, mode, options, toast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (mode !== "playlist") return;
    listPlaylists(libraryPath)
      .then(setPlaylists)
      .catch((e) => toast({ variant: "error", message: "Failed to load playlists", detail: String(e) }));
  }, [libraryPath, mode, toast]);

  const toggleRow = (id: string) => {
    setExcluded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAll = () => setExcluded(new Set());
  const deselectAll = () => setExcluded(new Set(pending.map((p) => p.change_id)));

  const includedCount = pending.length - excluded.size;

  const handleApply = async () => {
    if (includedCount === 0) return;
    if (locked) {
      toast({ variant: "error", message: "Rekordbox is running — close it to apply." });
      return;
    }
    const ok = await dialog.confirm({
      title: `Apply ${includedCount} change(s) to master.db?`,
      body: "A timestamped backup will be created beside master.db on the first write of this session.",
      confirmLabel: "Apply",
    });
    if (!ok) return;
    setApplying(true);
    try {
      const includedIds = pending
        .filter((p) => !excluded.has(p.change_id))
        .map((p) => p.change_id);
      const res = await syncExecute(libraryPath, mode, options, includedIds);
      const failed = res.failed.length;
      const applied = res.applied.length;
      const warnings = res.warnings ?? [];
      // Surface non-fatal warnings (e.g. unconvertible key values that were
      // written as the original string instead of Camelot/OpenKey). These
      // changes are still counted in `applied`, so the user otherwise would
      // see no signal that K of N keys silently failed to convert.
      const warningCount = warnings.length;
      const messageSuffix =
        (failed > 0 ? `, ${failed} failed` : "") +
        (warningCount > 0 ? `, ${warningCount} warning${warningCount === 1 ? "" : "s"}` : "");
      const detailParts: string[] = [];
      if (failed > 0) {
        detailParts.push(`First failure: ${res.failed[0]?.[1] ?? "unknown"}`);
      }
      if (warningCount > 0) {
        const shown = warnings.slice(0, 3).join("; ");
        const more = warningCount > 3 ? ` (+${warningCount - 3} more)` : "";
        detailParts.push(`Warnings: ${shown}${more}`);
      }
      toast({
        variant: failed > 0 || warningCount > 0 ? "warn" : "success",
        message: `Applied ${applied} change(s)${messageSuffix}.`,
        detail: detailParts.length > 0 ? detailParts.join(" • ") : undefined,
      });
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: "Apply failed", detail: String(e) });
    } finally {
      setApplying(false);
    }
  };

  return (
    <div className="flex h-full flex-col bg-surface p-4 text-sm">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-ink">Sync to master.db</h2>
        <button
          onClick={refresh}
          className="rounded bg-elevated px-3 py-1 text-ink hover:bg-edge"
          disabled={loading}
        >
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>

      {locked && (
        <div className="mb-3 rounded border border-red-500/30 bg-red-500/10 px-3 py-2 text-red-500">
          Rekordbox appears to be running. Close it before applying changes.
        </div>
      )}

      <div className="mb-4 grid grid-cols-2 gap-3 rounded-lg border border-edge bg-base p-3">
        <label className="flex flex-col gap-1">
          <span className="text-xs uppercase tracking-wide text-ink-muted">Mode</span>
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value as SyncMode)}
            className="rounded border border-edge bg-surface px-2 py-1 text-ink"
          >
            <option value="full">Full — all accepted changes</option>
            <option value="playlist">Playlist — tracks in a playlist</option>
            <option value="modified">Modified since last sync</option>
          </select>
        </label>

        {mode === "playlist" && (
          <label className="flex flex-col gap-1">
            <span className="text-xs uppercase tracking-wide text-ink-muted">Playlist</span>
            <select
              value={playlistId ?? ""}
              onChange={(e) => setPlaylistId(e.target.value || null)}
              className="rounded border border-edge bg-surface px-2 py-1 text-ink"
            >
              <option value="">— pick —</option>
              {playlists.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
        )}

        <label className="flex flex-col gap-1">
          <span className="text-xs uppercase tracking-wide text-ink-muted">Cue destination</span>
          <select
            value={cueDestination}
            onChange={(e) => setCueDestination(e.target.value as CueDestination)}
            className="rounded border border-edge bg-surface px-2 py-1 text-ink"
          >
            <option value="hot">Hot cues (1–8)</option>
            <option value="memory">Memory cues</option>
            <option value="both">Both</option>
          </select>
        </label>

        <label className="flex flex-col gap-1">
          <span className="text-xs uppercase tracking-wide text-ink-muted">Convert keys</span>
          <select
            value={convertKeys}
            onChange={(e) => setConvertKeys(e.target.value as KeyFormat)}
            className="rounded border border-edge bg-surface px-2 py-1 text-ink"
          >
            <option value="original">Original</option>
            <option value="camelot">Camelot</option>
            <option value="open_key">Open Key</option>
          </select>
        </label>

        <label className="col-span-2 flex items-center gap-2 text-ink">
          <input
            type="checkbox"
            checked={addLeadingZero}
            onChange={(e) => setAddLeadingZero(e.target.checked)}
          />
          {/* Sorted as text, an unpadded library reads 1A, 10A, 11A, 2A — the
              only reason this option exists. */}
          Add a leading zero to keys (01A, not 1A) so apps sort them correctly
        </label>

        <label className="col-span-2 flex items-center gap-2 text-ink">
          <input
            type="checkbox"
            checked={keepGrids}
            onChange={(e) => setKeepGrids(e.target.checked)}
          />
          Don&apos;t touch my grids (skip BPM writes)
        </label>
      </div>

      <div className="mb-2 flex items-center justify-between">
        <div className="text-ink-muted">
          {pending.length === 0
            ? "No pending changes"
            : `${includedCount} of ${pending.length} included`}
        </div>
        <div className="flex gap-2">
          <button
            onClick={selectAll}
            className="rounded bg-elevated px-2 py-1 text-xs text-ink hover:bg-edge"
            disabled={pending.length === 0}
          >
            Select all
          </button>
          <button
            onClick={deselectAll}
            className="rounded bg-elevated px-2 py-1 text-xs text-ink hover:bg-edge"
            disabled={pending.length === 0}
          >
            Deselect all
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-auto rounded-lg border border-edge bg-base">
        {pending.length === 0 ? (
          <div className="flex h-full items-center justify-center text-ink-muted">
            Nothing staged. Run a Cleanup or Smart Fix to populate this list.
          </div>
        ) : (
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-surface text-ink-muted">
              <tr>
                <th className="w-8 py-1 px-2 text-left"> </th>
                <th className="py-1 px-2 text-left">Track</th>
                <th className="py-1 px-2 text-left">Kind / Field</th>
                <th className="py-1 px-2 text-left">Old</th>
                <th className="py-1 px-2 text-left">New</th>
              </tr>
            </thead>
            <tbody>
              {pending.map((p) => {
                const included = !excluded.has(p.change_id);
                return (
                  <tr
                    key={p.change_id}
                    className={`border-t border-edge ${included ? "" : "opacity-40"}`}
                  >
                    <td className="py-1 px-2">
                      <input
                        type="checkbox"
                        checked={included}
                        onChange={() => toggleRow(p.change_id)}
                      />
                    </td>
                    <td className="py-1 px-2 truncate max-w-[280px] text-ink">
                      {p.track_title ?? p.track_id ?? "—"}
                    </td>
                    <td className="py-1 px-2 text-ink-muted">
                      {p.kind}
                      {p.field ? ` / ${p.field}` : ""}
                    </td>
                    <td className="py-1 px-2 truncate max-w-[200px] text-ink-muted">
                      {formatValue(p.old_value)}
                    </td>
                    <td className="py-1 px-2 truncate max-w-[200px] text-ink">
                      {formatValue(p.new_value)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      <div className="mt-4 flex items-center justify-end gap-2">
        <button
          onClick={handleApply}
          disabled={includedCount === 0 || locked || applying}
          className="rounded bg-accent px-4 py-2 font-medium text-base hover:opacity-90 disabled:opacity-50"
        >
          {applying ? "Applying…" : `Apply ${includedCount} change${includedCount === 1 ? "" : "s"}`}
        </button>
      </div>
    </div>
  );
}

function formatValue(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "string") return v;
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}
