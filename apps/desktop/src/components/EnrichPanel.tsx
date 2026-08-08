import { useState } from "react";
import { enrichPreview, enrichStage } from "../ipc";
import { useToast } from "./Toast";
import type { EnrichPreview, TrackProposal } from "../types";

interface Props {
  libraryPath: string;
  trackIds: string[];
  onClose: () => void;
}

/**
 * What the user has checked for one track: indices into that track's
 * `proposals` array, and the subset of `tags` chips that are on.
 *
 * Indices rather than field names because a provider can legitimately propose
 * the same field twice (e.g. Discogs and the embedded tag both offer a
 * `Genre`), and the user needs to be able to pick one without the other.
 */
interface Selection {
  proposals: Set<number>;
  tags: Set<string>;
}

function allChecked(track: TrackProposal): Selection {
  return {
    proposals: new Set(track.proposals.map((_, i) => i)),
    tags: new Set(track.tags),
  };
}

/**
 * Preview → pick → stage panel for the metadata enrichment lookup.
 *
 * Mirrors RelocateBanner's shape (scan, let the user choose, stage only what
 * they chose) but as a modal rather than a docked banner, because — unlike a
 * relocate scan — this one is launched from a specific selection rather than
 * running over the whole library.
 */
export function EnrichPanel({ libraryPath, trackIds, onClose }: Props) {
  const { toast } = useToast();
  const [originalRelease, setOriginalRelease] = useState(false);
  const [useDiscogs, setUseDiscogs] = useState(false);
  const [running, setRunning] = useState(false);
  const [staging, setStaging] = useState(false);
  const [preview, setPreview] = useState<EnrichPreview | null>(null);
  const [selection, setSelection] = useState<Map<string, Selection>>(
    new Map(),
  );

  const handleFindTags = async () => {
    setRunning(true);
    try {
      const result = await enrichPreview({
        library_path: libraryPath,
        track_ids: trackIds,
        original_release: originalRelease,
        use_discogs: useDiscogs,
      });
      setPreview(result);
      // Everything starts checked — the whole point of a preview is that the
      // user thins it out, not that they build it up from nothing.
      setSelection(
        new Map(result.tracks.map((t) => [t.track_id, allChecked(t)])),
      );
    } catch (e) {
      toast({
        variant: "error",
        message: "Find tags failed",
        detail: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setRunning(false);
    }
  };

  const toggleProposal = (trackId: string, index: number) => {
    setSelection((prev) => {
      const next = new Map(prev);
      const cur = next.get(trackId) ?? { proposals: new Set(), tags: new Set() };
      const proposals = new Set(cur.proposals);
      if (proposals.has(index)) proposals.delete(index);
      else proposals.add(index);
      next.set(trackId, { ...cur, proposals });
      return next;
    });
  };

  const toggleTag = (trackId: string, tag: string) => {
    setSelection((prev) => {
      const next = new Map(prev);
      const cur = next.get(trackId) ?? { proposals: new Set(), tags: new Set() };
      const tags = new Set(cur.tags);
      if (tags.has(tag)) tags.delete(tag);
      else tags.add(tag);
      next.set(trackId, { ...cur, tags });
      return next;
    });
  };

  const handleAccept = async () => {
    if (!preview) return;
    // Send back exactly what was shown, thinned to what's checked — not a
    // re-run of the lookup, so a provider answering differently a second
    // later can never stage something the user never saw.
    const accepted: TrackProposal[] = preview.tracks
      .map((t) => {
        const sel = selection.get(t.track_id);
        return {
          ...t,
          proposals: t.proposals.filter((_, i) => sel?.proposals.has(i) ?? false),
          tags: t.tags.filter((tag) => sel?.tags.has(tag) ?? false),
        };
      })
      .filter((t) => t.proposals.length > 0 || t.tags.length > 0);

    setStaging(true);
    try {
      await enrichStage(libraryPath, accepted);
      onClose();
    } catch (e) {
      toast({
        variant: "error",
        message: "Could not stage enrichment",
        detail: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setStaging(false);
    }
  };

  const matched =
    preview?.tracks.filter(
      (t) => !t.no_match && (t.proposals.length > 0 || t.tags.length > 0),
    ) ?? [];
  const noMatch = preview?.tracks.filter((t) => t.no_match) ?? [];

  const acceptedCount = matched.reduce((sum, t) => {
    const sel = selection.get(t.track_id);
    return sum + (sel ? sel.proposals.size + sel.tags.size : 0);
  }, 0);

  return (
    <div
      role="dialog"
      aria-label="Find tags"
      aria-modal="true"
      data-testid="enrich-panel"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      <div className="flex max-h-[85vh] w-[36rem] flex-col overflow-hidden rounded-lg border border-edge bg-base shadow-xl">
        <header className="border-b border-edge px-4 py-3">
          <h2 className="text-[14px] font-semibold text-ink">
            Find tags
          </h2>
          <p className="text-[11px] text-ink-muted">
            Looks up missing genre, year, label and album for {trackIds.length} track
            {trackIds.length === 1 ? "" : "s"}. Nothing is written until you
            accept.
          </p>
        </header>

        <div className="flex-1 overflow-y-auto p-4">
          {!preview && (
            <div className="space-y-3 text-[12px]">
              <label className="flex items-start gap-2">
                <input
                  type="checkbox"
                  className="mt-0.5"
                  checked={originalRelease}
                  onChange={(e) => setOriginalRelease(e.target.checked)}
                />
                <span>
                  <span className="block text-ink">Original release</span>
                  <span className="block text-ink-muted">
                    Strips remix/remaster text from the title and looks for
                    the earliest release instead of whatever reissue turns up
                    first.
                  </span>
                </span>
              </label>
              <label className="flex items-start gap-2">
                <input
                  type="checkbox"
                  className="mt-0.5"
                  checked={useDiscogs}
                  onChange={(e) => setUseDiscogs(e.target.checked)}
                />
                <span className="text-ink">Use Discogs</span>
              </label>
            </div>
          )}

          {preview && (
            <div className="space-y-3">
              {preview.errors.length > 0 && (
                <div
                  data-testid="enrich-errors"
                  className="rounded border border-status-warn/40 bg-status-warn/10 px-2 py-1.5 text-[11px] text-status-warn"
                >
                  <p className="font-semibold">
                    Some providers did not respond
                  </p>
                  {/* Verbatim, not summarised — one source being down is not
                      "no match", and the user should be able to tell which
                      provider to distrust. */}
                  <ul className="mt-0.5 list-disc pl-4">
                    {preview.errors.map((err, i) => (
                      <li key={i}>{err}</li>
                    ))}
                  </ul>
                </div>
              )}

              {matched.length === 0 &&
                noMatch.length === 0 &&
                preview.unsearchable.length === 0 && (
                  <p className="text-[12px] text-ink-muted">
                    No proposals came back for this selection.
                  </p>
                )}

              <ul className="divide-y divide-edge/30">
                {matched.map((t) => {
                  const sel = selection.get(t.track_id);
                  return (
                    <li key={t.track_id} className="py-2">
                      <div className="mb-1 font-mono text-[10px] text-ink-faint">
                        {t.track_id}
                      </div>
                      <div className="space-y-1">
                        {t.proposals.map((p, i) => (
                          <label
                            key={`${p.field}-${i}`}
                            className="flex items-center gap-2 rounded px-1 py-1 text-[11px] hover:bg-elevated/40"
                          >
                            <input
                              type="checkbox"
                              checked={sel?.proposals.has(i) ?? false}
                              onChange={() => toggleProposal(t.track_id, i)}
                            />
                            <span className="w-20 shrink-0 text-ink-secondary">
                              {p.field}
                            </span>
                            <span className="flex-1 truncate text-ink">
                              {p.after}
                            </span>
                            {/* Which provider claimed this — ADR-0008 requires
                                the source stay on screen, not just in a
                                tooltip or a log line. */}
                            <span className="shrink-0 rounded-full border border-edge bg-surface px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-ink-muted">
                              {p.source}
                            </span>
                          </label>
                        ))}
                      </div>
                      {t.tags.length > 0 && (
                        <div className="mt-1 flex flex-wrap items-center gap-1.5 pl-1">
                          <span className="text-[9px] uppercase tracking-wider text-ink-faint">
                            New custom tags (not the Genre field):
                          </span>
                          {t.tags.map((tag) => (
                            <label
                              key={tag}
                              className="flex items-center gap-1 rounded-full border border-edge bg-surface px-2 py-0.5 text-[10px] text-ink"
                            >
                              <input
                                type="checkbox"
                                className="h-3 w-3"
                                checked={sel?.tags.has(tag) ?? false}
                                onChange={() => toggleTag(t.track_id, tag)}
                              />
                              {tag}
                            </label>
                          ))}
                        </div>
                      )}
                    </li>
                  );
                })}
              </ul>

              {noMatch.length > 0 && (
                <section className="rounded border border-edge/60 bg-surface/60 p-2 text-[11px] text-ink-muted">
                  <h3 className="mb-1 font-semibold text-ink-secondary">
                    Nothing found ({noMatch.length})
                  </h3>
                  <p>
                    No provider matched these tracks against their current
                    tags. Try running Smart Fixes on the tags first — cleaner
                    artist/title text is what these lookups key on.
                  </p>
                  <ul className="mt-1 font-mono text-[10px] text-ink-faint">
                    {noMatch.map((t) => (
                      <li key={t.track_id}>{t.track_id}</li>
                    ))}
                  </ul>
                </section>
              )}

              {preview.unsearchable.length > 0 && (
                <section className="rounded border border-edge/60 bg-surface/60 p-2 text-[11px] text-ink-muted">
                  <h3 className="mb-1 font-semibold text-ink-secondary">
                    No usable title ({preview.unsearchable.length})
                  </h3>
                  <p>
                    These tracks had nothing to search with, so they were
                    skipped rather than sent to a provider.
                  </p>
                  <ul className="mt-1 font-mono text-[10px] text-ink-faint">
                    {preview.unsearchable.map((id) => (
                      <li key={id}>{id}</li>
                    ))}
                  </ul>
                </section>
              )}
            </div>
          )}
        </div>

        <footer className="flex items-center gap-2 border-t border-edge px-4 py-3">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md border border-edge-strong px-3 py-1.5 text-[12px] text-ink hover:bg-elevated"
          >
            Cancel
          </button>
          <div className="flex-1" />
          {!preview ? (
            <button
              type="button"
              onClick={() => void handleFindTags()}
              disabled={running}
              className="flex h-8 items-center gap-2 rounded-md bg-accent px-3 text-[12px] font-medium text-base hover:bg-accent-hover disabled:opacity-50"
            >
              {running ? (
                <>
                  <span className="h-3 w-3 animate-spin rounded-full border border-base border-t-transparent" />
                  Finding tags…
                </>
              ) : (
                "Find tags"
              )}
            </button>
          ) : (
            <button
              type="button"
              onClick={() => void handleAccept()}
              disabled={staging || acceptedCount === 0}
              className="rounded-md bg-accent px-3 py-1.5 text-[12px] font-medium text-base hover:bg-accent-hover disabled:opacity-50"
            >
              {staging
                ? "Staging…"
                : `Accept selected (${acceptedCount})`}
            </button>
          )}
        </footer>
      </div>
    </div>
  );
}
