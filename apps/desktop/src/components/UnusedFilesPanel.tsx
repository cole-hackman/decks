import { useCallback, useMemo, useState } from "react";
import { deleteUnusedFiles, scanUnusedFiles } from "../ipc";
import { useToast } from "./Toast";
import type { ExtensionMode, UnusedScan } from "../types";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

interface Props {
  libraryPath: string;
}

/**
 * Find Unused Files — the inverse of a missing-file scan.
 *
 * Its output is a list of deletion candidates, so nothing is pre-selected,
 * deletion is behind an explicit confirmation, and the panel says what the scan
 * did not look at rather than implying it was exhaustive.
 */
export function UnusedFilesPanel({ libraryPath }: Props) {
  const { toast } = useToast();
  const [root, setRoot] = useState("");
  const [mode, setMode] = useState<ExtensionMode>("exclude");
  const [extensions, setExtensions] = useState("");
  const [scan, setScan] = useState<UnusedScan | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const runScan = useCallback(async () => {
    setBusy(true);
    try {
      const result = await scanUnusedFiles(libraryPath, [root.trim()], {
        mode,
        extensions: extensions
          .split(",")
          .map((s) => s.trim().replace(/^\./, "").toLowerCase())
          .filter((s) => s !== ""),
      });
      setScan(result);
      // Nothing is pre-selected: a pre-ticked delete list over someone's music
      // folder is how accidents happen.
      setSelected(new Set());
      setConfirming(false);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, root, mode, extensions, toast]);

  const selectedBytes = useMemo(
    () =>
      (scan?.files ?? [])
        .filter((f) => selected.has(f.path))
        .reduce((sum, f) => sum + f.size_bytes, 0),
    [scan, selected],
  );

  const copyPaths = useCallback(async () => {
    const list = (scan?.files ?? []).map((f) => f.path).join("\n");
    try {
      await navigator.clipboard.writeText(list);
      toast({ variant: "success", message: "Paths copied — nothing deleted." });
    } catch {
      toast({ variant: "error", message: "Could not access the clipboard." });
    }
  }, [scan, toast]);

  const runDelete = useCallback(async () => {
    setBusy(true);
    try {
      const report = await deleteUnusedFiles(libraryPath, [...selected]);
      const suffix = report.report_path ? ` Record: ${report.report_path}` : "";
      if (report.failed.length > 0) {
        toast({
          variant: "error",
          message: `Deleted ${report.deleted.length}, failed ${report.failed.length}: ${report.failed[0][1]}.${suffix}`,
        });
      } else {
        toast({
          variant: "success",
          message: `Deleted ${report.deleted.length} file(s).${suffix}`,
        });
      }
      setScan(null);
      setSelected(new Set());
      setConfirming(false);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, selected, toast]);

  return (
    <section
      className="border-t border-border px-4 py-3"
      aria-label="Find unused files"
    >
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Find Unused Files
      </h3>
      <p className="mb-2 text-[11px] text-muted">
        Lists files under a folder that the library does not reference. Deleting
        them cannot be undone.
      </p>

      <div className="mb-2 grid gap-2 md:grid-cols-3">
        <label className="text-xs md:col-span-2">
          <span className="mb-1 block font-medium uppercase tracking-wide text-muted">
            Folder to scan
          </span>
          <input
            className="w-full rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
            placeholder="/Users/you/Music"
            value={root}
            onChange={(e) => setRoot(e.target.value)}
          />
        </label>
        <label className="text-xs">
          <span className="mb-1 block font-medium uppercase tracking-wide text-muted">
            Extensions
          </span>
          <div className="flex gap-1">
            <select
              aria-label="Extension mode"
              className="rounded border border-border bg-surface px-1 py-1 text-xs"
              value={mode}
              onChange={(e) => setMode(e.target.value as ExtensionMode)}
            >
              <option value="exclude">Exclude</option>
              <option value="include">Include</option>
            </select>
            <input
              aria-label="Extension list"
              className="w-full rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
              placeholder="PNG,JPG"
              value={extensions}
              onChange={(e) => setExtensions(e.target.value)}
            />
          </div>
        </label>
      </div>

      <div className="mb-2 flex flex-wrap gap-2 text-xs">
        <button
          type="button"
          disabled={busy || root.trim() === ""}
          className="rounded border border-border px-2 py-0.5 hover:bg-surface-hover disabled:opacity-50"
          onClick={() => void runScan()}
        >
          Scan
        </button>
        {scan != null && scan.files.length > 0 && (
          <>
            <button
              type="button"
              className="rounded border border-border px-2 py-0.5 hover:bg-surface-hover"
              onClick={() =>
                setSelected(new Set(scan.files.map((f) => f.path)))
              }
            >
              Select all
            </button>
            <button
              type="button"
              className="rounded border border-border px-2 py-0.5 hover:bg-surface-hover"
              onClick={() => void copyPaths()}
            >
              Copy paths
            </button>
            {confirming ? (
              <>
                <button
                  type="button"
                  disabled={busy}
                  className="rounded bg-red-500 px-2 py-0.5 text-white hover:bg-red-600 disabled:opacity-50"
                  onClick={() => void runDelete()}
                >
                  Permanently delete {selected.size} file(s)
                </button>
                <button
                  type="button"
                  className="rounded border border-border px-2 py-0.5 hover:bg-surface-hover"
                  onClick={() => setConfirming(false)}
                >
                  Cancel
                </button>
              </>
            ) : (
              <button
                type="button"
                disabled={selected.size === 0}
                className="rounded border border-red-500/40 px-2 py-0.5 text-red-400 hover:bg-red-500/10 disabled:opacity-50"
                onClick={() => setConfirming(true)}
              >
                Delete {selected.size} file(s)…
              </button>
            )}
          </>
        )}
      </div>

      {confirming && (
        <p className="mb-2 text-xs text-red-400" role="alert">
          This deletes {selected.size} file(s) ({formatBytes(selectedBytes)}) from
          disk. It cannot be undone. A record of what was deleted is written to the
          app's data folder.
        </p>
      )}

      {scan != null && (
        <div data-testid="unused-scan">
          <p className="mb-1 text-[11px] text-muted">
            {scan.files.length} unused file(s), {formatBytes(scan.total_bytes)}.
            Skipped: {scan.skipped_directories.join(", ")}.
          </p>
          {scan.errors.length > 0 && (
            <p className="mb-1 text-[11px] text-amber-500">
              {scan.errors.length} path(s) could not be read.
            </p>
          )}
          <ul className="max-h-64 space-y-0.5 overflow-auto text-xs">
            {scan.files.map((f) => (
              <li key={f.path} className="flex items-center gap-2">
                <input
                  type="checkbox"
                  aria-label={`Select ${f.path}`}
                  checked={selected.has(f.path)}
                  onChange={(e) => {
                    const next = new Set(selected);
                    if (e.target.checked) next.add(f.path);
                    else next.delete(f.path);
                    setSelected(next);
                  }}
                />
                <span className="w-16 text-right font-mono text-muted">
                  {formatBytes(f.size_bytes)}
                </span>
                <span className="truncate font-mono">{f.path}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
