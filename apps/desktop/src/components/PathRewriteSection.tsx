import { useCallback, useMemo, useState } from "react";
import { applyPathRewrite, previewPathRewrite } from "../ipc";
import { useToast } from "./Toast";
import type { RewritePreview, RewriteSkipReason } from "../types";

interface Props {
  libraryPath: string;
}

function why(reason: RewriteSkipReason): string {
  switch (reason.kind) {
    case "no_match":
      return "does not start with that prefix";
    case "not_missing":
      return "the file is fine";
    case "unchanged":
      return "the rewrite would change nothing";
    case "taken":
      return `another track is already at ${reason.detail}`;
  }
}

/**
 * Find Lost Tracks — prefix rewriting.
 *
 * The fuzzy relocate answers "where did this one file go?". This answers a
 * different question: "the drive letter changed, rewrite all four thousand of
 * them."
 *
 * **Nothing is inferred.** The user states both prefixes. A tool that guessed
 * the rewrite would eventually guess wrong across an entire library, which is
 * why the spec calls this the deterministic path and why there is no "detect"
 * button here.
 *
 * Rewrites stage as `TrackRelocate` and go through review and Sync — whose
 * write guard takes the backup the spec recommends, except that it is not
 * optional.
 */
export function PathRewriteSection({ libraryPath }: Props) {
  const { toast } = useToast();
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [extension, setExtension] = useState("");
  const [allTracks, setAllTracks] = useState(false);
  const [preview, setPreview] = useState<RewritePreview | null>(null);
  const [busy, setBusy] = useState(false);

  const spec = useMemo(
    () => ({
      from_prefix: from,
      to_prefix: to,
      new_extension: extension.trim() === "" ? null : extension.trim(),
      all_tracks: allTracks,
    }),
    [from, to, extension, allTracks],
  );

  const runPreview = useCallback(async () => {
    setBusy(true);
    try {
      setPreview(await previewPathRewrite(libraryPath, spec));
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, spec, toast]);

  const stage = useCallback(async () => {
    if (!preview || preview.plan.rewrites.length === 0) return;
    setBusy(true);
    try {
      const ids = await applyPathRewrite(libraryPath, preview.plan.rewrites);
      toast({
        variant: "success",
        message: `Staged ${ids.length} relocation(s) for review.`,
      });
      setPreview(null);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, preview, toast]);

  return (
    <section
      className="shrink-0 border-t border-border px-4 py-3"
      aria-label="Rewrite paths"
    >
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Rewrite Paths
      </h3>
      <p className="mb-2 text-[11px] text-muted">
        For known changes — a new drive letter, a moved music folder. Nothing is
        guessed: every path starting with the source is rewritten to the target.
        Changes are staged for review.
      </p>

      <div className="mb-2 flex flex-wrap items-end gap-2 text-xs">
        <label className="flex-1">
          <span className="mb-1 block text-muted">Replace paths starting with</span>
          <input
            aria-label="Source prefix"
            className="w-full rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
            placeholder="D:\\Music"
            value={from}
            onChange={(e) => {
              setFrom(e.target.value);
              setPreview(null);
            }}
          />
        </label>
        <label className="flex-1">
          <span className="mb-1 block text-muted">With</span>
          <input
            aria-label="Target prefix"
            className="w-full rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
            placeholder="/Volumes/Music"
            value={to}
            onChange={(e) => {
              setTo(e.target.value);
              setPreview(null);
            }}
          />
        </label>
        <label>
          <span className="mb-1 block text-muted">New extension</span>
          <input
            aria-label="New extension"
            className="w-20 rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
            placeholder="mp3"
            value={extension}
            onChange={(e) => {
              setExtension(e.target.value);
              setPreview(null);
            }}
          />
        </label>
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            aria-label="Include tracks that are not missing"
            checked={allTracks}
            onChange={(e) => {
              setAllTracks(e.target.checked);
              setPreview(null);
            }}
          />
          Include working paths
        </label>
        <button
          type="button"
          disabled={busy || from.trim() === ""}
          className="rounded border border-border px-3 py-1 hover:bg-surface-hover disabled:opacity-50"
          onClick={() => void runPreview()}
        >
          Preview rewrite
        </button>
        <button
          type="button"
          disabled={busy || !preview || preview.plan.rewrites.length === 0}
          className="rounded bg-accent px-3 py-1 text-white hover:bg-accent-hover disabled:opacity-50"
          onClick={() => void stage()}
        >
          Stage {preview?.plan.rewrites.length ?? 0} relocation(s)
        </button>
      </div>

      {allTracks && (
        <p className="mb-2 text-[11px] text-amber-500" data-testid="all-tracks-warning">
          Working paths will be rewritten too. Only do this when you know the
          whole folder moved.
        </p>
      )}

      {preview != null && (
        <div data-testid="rewrite-preview">
          <p className="mb-1 text-[11px] text-muted">
            {preview.plan.rewrites.length} of {preview.considered} track(s) would
            be rewritten.
          </p>
          {preview.plan.rewrites.length === 0 ? (
            <p className="text-xs text-muted">
              No path starts with that prefix.
            </p>
          ) : (
            <ul className="max-h-40 space-y-0.5 overflow-auto font-mono text-[11px]">
              {preview.plan.rewrites.map((r) => (
                <li key={r.track_id}>
                  <span className="text-muted line-through">{r.from}</span>{" "}
                  <span>→ {r.to}</span>
                </li>
              ))}
            </ul>
          )}
          {/* Only collisions are worth listing: "does not start with that
              prefix" over 4,000 tracks is noise, not information. */}
          {preview.plan.skipped.some(([, , r]) => r.kind === "taken") && (
            <ul
              className="mt-1 space-y-0.5 text-[11px] text-amber-500"
              data-testid="rewrite-collisions"
            >
              {preview.plan.skipped
                .filter(([, , r]) => r.kind === "taken")
                .map(([id, path, reason]) => (
                  <li key={id}>
                    {path} — {why(reason)}
                  </li>
                ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
