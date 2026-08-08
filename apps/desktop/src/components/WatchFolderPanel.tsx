import { useCallback, useEffect, useRef, useState } from "react";
import {
  addWatchFolder,
  clearDismissedArrivals,
  dismissArrivals,
  listWatchFolders,
  removeWatchFolder,
  scanArrivals,
  stageArrivalImports,
} from "../ipc";
import { useToast } from "./Toast";
import type { Arrival, WatchFolderRow, WatchScan } from "../types";

/**
 * How often the folder is re-scanned while this panel is open.
 *
 * Scanning rather than a native filesystem watcher, so this interval is what
 * "continuous observation" actually means. Fifteen seconds is well under the
 * time it takes to notice a download finishing, and a scan of a watch folder is
 * cheap — it is one directory, not the whole library.
 */
const POLL_MS = 15_000;

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

interface Props {
  libraryPath: string;
}

/**
 * Watch folders — music dropped into a folder, waiting to join the library.
 *
 * Importing stages a `TrackCreate` change rather than writing to `master.db`,
 * which sync cannot do for a new track. The panel says so, because "staged"
 * without "and here is how it reaches Rekordbox" is not useful.
 */
export function WatchFolderPanel({ libraryPath }: Props) {
  const { toast } = useToast();
  const [folders, setFolders] = useState<WatchFolderRow[]>([]);
  const [newPath, setNewPath] = useState("");
  const [scan, setScan] = useState<WatchScan | null>(null);
  const [busy, setBusy] = useState(false);
  // A scan can outlive the component; a ref avoids setting state after unmount.
  const alive = useRef(true);

  const refreshFolders = useCallback(async () => {
    try {
      const list = await listWatchFolders();
      if (alive.current) setFolders(Array.isArray(list) ? list : []);
    } catch {
      if (alive.current) setFolders([]);
    }
  }, []);

  const rescan = useCallback(async () => {
    try {
      const result = await scanArrivals(libraryPath);
      if (alive.current) setScan(result);
    } catch (e) {
      // Polling errors are not toasted — a folder unplugged mid-session would
      // otherwise produce a notification every fifteen seconds.
      if (alive.current) console.error("watch scan failed:", e);
    }
  }, [libraryPath]);

  useEffect(() => {
    alive.current = true;
    void refreshFolders();
    void rescan();
    const timer = setInterval(() => void rescan(), POLL_MS);
    return () => {
      alive.current = false;
      clearInterval(timer);
    };
  }, [refreshFolders, rescan]);

  const importAll = useCallback(
    async (arrivals: Arrival[]) => {
      if (arrivals.length === 0) return;
      setBusy(true);
      try {
        const result = await stageArrivalImports(
          libraryPath,
          arrivals.map((a) => a.path),
        );
        if (result.failed.length > 0) {
          toast({
            variant: "error",
            message: `Staged ${result.staged.length}, failed ${result.failed.length}: ${result.failed[0][1]}`,
          });
        } else {
          // Analysed and tagged are reported separately: one reads the file,
          // the other rewrites it, and a summary that blurs them would hide
          // the fact that files on disk changed.
          // Guard the *shape*, not just a rejection. A shell built before
          // these fields existed resolves successfully without them, and
          // `.length` on undefined would take the whole import down — the same
          // failure the cue-presets null once caused.
          const analysed = result.analysed ?? [];
          const tagged = result.tagged ?? [];
          const tagSkipped = result.tag_skipped ?? [];
          const extra = [
            analysed.length > 0 ? `${analysed.length} analysed` : null,
            tagged.length > 0 ? `${tagged.length} tagged` : null,
          ].filter(Boolean);
          toast({
            variant: "success",
            message:
              `Staged ${result.staged.length} new track(s)` +
              (extra.length > 0 ? ` (${extra.join(", ")})` : "") +
              ". Export the XML and import it in Rekordbox — sync cannot add tracks.",
            // A skip on a setting the user turned on has to say why, or the
            // setting looks broken.
            detail:
              tagSkipped.length > 0
                ? `Tags not written for ${tagSkipped.length} file(s): ${tagSkipped[0][1]}`
                : undefined,
          });
        }
        await rescan();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [libraryPath, rescan, toast],
  );

  const ignore = useCallback(
    async (paths: string[]) => {
      try {
        await dismissArrivals(paths);
        await rescan();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [rescan, toast],
  );

  const arrivals = scan?.arrivals ?? [];
  const pending = scan?.pending ?? [];

  return (
    <section className="border-t border-border px-4 py-3" aria-label="Watch folders">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-muted">
          Watch Folders
        </h3>
        <button
          type="button"
          className="rounded border border-border px-2 py-0.5 text-xs hover:bg-surface-hover"
          onClick={() => void rescan()}
        >
          Rescan
        </button>
      </div>
      <p className="mb-2 text-[11px] text-muted">
        Music dropped into these folders shows up here. Importing stages the track
        for the XML export — sync cannot add new tracks to Rekordbox's database,
        only change existing ones.
      </p>

      {folders.length === 0 ? (
        <p className="mb-2 text-xs text-muted" data-testid="no-watch-folders">
          No watch folders. Add one below.
        </p>
      ) : (
        <ul className="mb-2 space-y-0.5 text-xs">
          {folders.map((f) => (
            <li key={f.id} className="flex items-center gap-2">
              <span className="truncate font-mono">{f.path}</span>
              <button
                type="button"
                aria-label={`Stop watching ${f.path}`}
                className="ml-auto text-muted hover:text-red-400"
                onClick={async () => {
                  await removeWatchFolder(f.id);
                  await refreshFolders();
                  await rescan();
                }}
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="mb-3 flex gap-2 text-xs">
        <input
          aria-label="New watch folder"
          className="flex-1 rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
          placeholder="/Users/you/Music/Watch Folder"
          value={newPath}
          onChange={(e) => setNewPath(e.target.value)}
        />
        <button
          type="button"
          disabled={newPath.trim() === ""}
          className="rounded border border-border px-2 py-1 hover:bg-surface-hover disabled:opacity-50"
          onClick={async () => {
            try {
              await addWatchFolder(newPath.trim());
              setNewPath("");
              await refreshFolders();
              await rescan();
            } catch (e) {
              toast({ variant: "error", message: String(e) });
            }
          }}
        >
          Watch
        </button>
      </div>

      {scan != null && (
        <div data-testid="watch-arrivals">
          <div className="mb-1 flex items-center gap-2">
            <p className="text-[11px] font-medium uppercase tracking-wide text-muted">
              {arrivals.length} new file(s)
            </p>
            {arrivals.length > 0 && (
              <>
                <button
                  type="button"
                  disabled={busy}
                  className="rounded bg-accent px-2 py-0.5 text-xs text-white hover:bg-accent-hover disabled:opacity-50"
                  onClick={() => void importAll(arrivals)}
                >
                  Import all
                </button>
                <button
                  type="button"
                  className="rounded border border-border px-2 py-0.5 text-xs hover:bg-surface-hover"
                  onClick={() => void ignore(arrivals.map((a) => a.path))}
                >
                  Ignore all
                </button>
              </>
            )}
            <button
              type="button"
              className="ml-auto text-[11px] text-muted hover:text-ink"
              onClick={async () => {
                await clearDismissedArrivals();
                await rescan();
              }}
            >
              Un-ignore everything
            </button>
          </div>

          {pending.length > 0 && (
            <p className="mb-1 text-[11px] text-amber-500" data-testid="watch-pending">
              {pending.length} file(s) still being written — held back until they
              stop changing, so their tags are read intact.
            </p>
          )}

          <ul className="max-h-56 space-y-0.5 overflow-auto text-xs">
            {arrivals.map((a) => (
              <li key={a.path} className="flex items-center gap-2">
                <span className="w-16 text-right font-mono text-muted">
                  {formatBytes(a.size_bytes)}
                </span>
                <span className="truncate">{basename(a.path)}</span>
                <button
                  type="button"
                  disabled={busy}
                  aria-label={`Import ${basename(a.path)}`}
                  className="ml-auto text-accent hover:underline disabled:opacity-50"
                  onClick={() => void importAll([a])}
                >
                  Import
                </button>
                <button
                  type="button"
                  aria-label={`Ignore ${basename(a.path)}`}
                  className="text-muted hover:text-red-400"
                  onClick={() => void ignore([a.path])}
                >
                  Ignore
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
