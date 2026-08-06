import { useCallback, useState } from "react";
import { brokenTracksReport, scanBrokenTracks, saveTextFile } from "../ipc";
import { useToast } from "./Toast";
import type { BrokenScan, CheckDepth } from "../types";

interface Props {
  libraryPath: string;
}

function describe(status: BrokenScan["broken"][number]["status"]): string {
  switch (status.kind) {
    case "ok":
      return "plays";
    case "missing":
      return "the file is not there";
    case "unreadable":
      return `cannot be opened: ${status.detail}`;
    case "undecodable":
      return `does not decode: ${status.detail}`;
    case "truncated":
      return `incomplete: ${status.detail}`;
    case "damaged":
      return `plays with ${status.detail.bad_packets} damaged section(s)`;
  }
}

/**
 * Find Broken Tracks.
 *
 * The existing broken-link scan asks whether a path exists. This asks whether
 * the file actually decodes — a truncated download, a half-copied file, a
 * `.mp3` that is really an HTML error page are all present and all unplayable.
 *
 * The two depths cost very different amounts, so the UI names the trade rather
 * than picking silently: a header check is fast and misses late corruption; a
 * full decode catches truncation and costs about what analysing the track
 * costs.
 *
 * Nothing here deletes anything, on disk or in the library.
 */
export function BrokenTracksPanel({ libraryPath }: Props) {
  const { toast } = useToast();
  const [depth, setDepth] = useState<CheckDepth>("header");
  const [scan, setScan] = useState<BrokenScan | null>(null);
  const [busy, setBusy] = useState(false);

  const run = useCallback(async () => {
    setBusy(true);
    setScan(null);
    try {
      setScan(await scanBrokenTracks(libraryPath, [], depth));
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, depth, toast]);

  const saveReport = useCallback(async () => {
    if (!scan || scan.broken.length === 0) return;
    try {
      const text = await brokenTracksReport(scan.broken);
      const path = await saveTextFile("broken-tracks.txt", text);
      if (path) toast({ variant: "success", message: `Saved to ${path}.` });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  }, [scan, toast]);

  return (
    <section
      className="rounded-lg border border-edge bg-surface p-4"
      aria-label="Find broken tracks"
    >
      <h2 className="text-sm font-semibold text-ink">Find Broken Tracks</h2>
      <p className="mb-3 mt-1 text-[12px] leading-relaxed text-ink-secondary">
        Checks that files actually decode, not just that they exist. Nothing is
        deleted — removing a track is still a staged change you review.
      </p>

      <div className="mb-3 flex flex-wrap items-end gap-2 text-xs">
        <label>
          <span className="mb-1 block text-ink-secondary">How thorough</span>
          <select
            aria-label="Check depth"
            className="rounded-md border border-edge-strong bg-surface px-2 py-1 text-xs"
            value={depth}
            onChange={(e) => {
              setDepth(e.target.value as CheckDepth);
              setScan(null);
            }}
          >
            <option value="header">Quick — read the header only</option>
            <option value="full">Full — decode every file</option>
          </select>
        </label>
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-edge-strong px-3 py-1 hover:bg-elevated disabled:opacity-50"
          onClick={() => void run()}
        >
          {busy ? "Scanning…" : "Scan"}
        </button>
        <button
          type="button"
          disabled={!scan || scan.broken.length === 0}
          className="rounded-md border border-edge-strong px-3 py-1 hover:bg-elevated disabled:opacity-50"
          onClick={() => void saveReport()}
        >
          Save report
        </button>
      </div>

      <p className="mb-3 text-[11px] text-ink-faint" data-testid="broken-depth-note">
        {depth === "full"
          ? "A full decode is the only way to catch a truncated download, and takes about as long as analysing each track."
          : "A header check is fast, but a file that is fine until the last ten seconds will pass it."}
      </p>

      {scan != null && (
        <div data-testid="broken-scan-result">
          <p className="mb-2 text-xs text-ink-secondary">
            Checked {scan.checked} track(s): {scan.broken.length} broken
            {scan.no_path > 0
              ? `, ${scan.no_path} with no file path to check`
              : ""}
            .
          </p>
          {scan.broken.length === 0 ? (
            <p className="text-xs text-status-ok" data-testid="no-broken-tracks">
              Everything checked plays.
            </p>
          ) : (
            <ul className="max-h-64 space-y-2 overflow-auto text-xs">
              {scan.broken.map((t) => (
                <li key={t.track_id} className="border-l-2 border-red-500/50 pl-2">
                  <div className="text-ink">
                    {t.artist ? `${t.artist} — ` : ""}
                    {t.title}
                  </div>
                  <div className="font-mono text-[11px] text-ink-muted">
                    {t.path}
                  </div>
                  <div className="text-[11px] text-amber-500">
                    {describe(t.status)}
                  </div>
                  {t.playlists.length > 0 && (
                    // The reason the report exists: sourcing a replacement
                    // means knowing which set is now short a track.
                    <div className="text-[11px] text-ink-faint">
                      in: {t.playlists.join(", ")}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
