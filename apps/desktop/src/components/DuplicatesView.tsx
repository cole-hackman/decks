import { useCallback, useEffect, useMemo, useState } from "react";
import {
  listLibraryDuplicateGroups,
  planDuplicateResolution,
  preselectKeepers,
  resolveDuplicates,
} from "../ipc";
import { useDialog } from "../hooks/useDialog";
import type { DuplicateCandidate, PreferRule } from "../types";
import { useToast } from "./Toast";
import { DeleteFromDiskDialog } from "./DeleteFromDiskDialog";
import type { DuplicateGroup, DuplicateKind, Track } from "../types";

interface Props {
  libraryPath: string;
  onOpenInspector?: (track: Track) => void;
}

const KIND_LABEL: Record<DuplicateKind, string> = {
  ExactTitleArtist: "Exact title + artist",
  FuzzyTitle: "Fuzzy title match",
  AudioFingerprint: "Audio fingerprint match",
};

const KIND_TINT: Record<DuplicateKind, string> = {
  ExactTitleArtist: "bg-accent/15 text-accent-hover",
  FuzzyTitle: "bg-amber-500/15 text-amber-500",
  AudioFingerprint: "bg-violet-500/15 text-violet-400",
};

function groupId(g: DuplicateGroup, idx: number): string {
  const kind = g.kind ?? "ExactTitleArtist";
  return `${kind}:${idx}:${g.tracks.map((t) => t.id).join(",")}`;
}

/** The facts the keeper heuristic looks at, from the tracks we already have. */
function candidatesOf(
  g: DuplicateGroup,
  cueIds: Set<string>,
  playlistCounts: Map<string, number>,
): DuplicateCandidate[] {
  return g.tracks.map((t) => ({
    track_id: t.id,
    bit_rate: t.bit_rate,
    duration_secs: t.duration_secs,
    has_cues: cueIds.has(t.id),
    rating: t.rating,
    play_count: t.dj_play_count,
    in_playlists: playlistCounts.get(t.id) ?? 0,
  }));
}

const PREFER_RULES: { value: PreferRule; label: string }[] = [
  { value: "best", label: "Best (cues, then bitrate)" },
  { value: "highest_bitrate", label: "Highest bitrate" },
  { value: "has_cues", label: "Has cue points" },
  { value: "most_playlists", label: "In the most playlists" },
  { value: "longest", label: "Longest" },
];

export function DuplicatesView({ libraryPath, onOpenInspector }: Props) {
  const { toast } = useToast();
  const dialog = useDialog();
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Per-group "keep" selection (defaults to first track in each group).
  const [keepByGroup, setKeepByGroup] = useState<Record<string, string>>({});
  const [busyGroup, setBusyGroup] = useState<string | null>(null);
  /**
   * The losing copies of one group, staged for a delete-from-disk.
   *
   * Deliberately separate from "archive rest": archiving is reversible in the
   * library and leaves the audio alone, so it stays the default action and the
   * one styled as primary. Deleting the file is the other button, and it goes
   * through the preview dialog like every other irreversible step.
   */
  const [deletingIds, setDeletingIds] = useState<string[] | null>(null);

  const refresh = useCallback(async () => {
    if (!libraryPath) return;
    setLoading(true);
    setError(null);
    try {
      const rows = await listLibraryDuplicateGroups(libraryPath);
      setGroups(rows);
      const defaults: Record<string, string> = {};
      rows.forEach((g, idx) => {
        const id = groupId(g, idx);
        defaults[id] = g.tracks[0]?.id ?? "";
      });
      setKeepByGroup(defaults);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [libraryPath]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const counts = useMemo(() => {
    const c: Record<DuplicateKind, number> = {
      ExactTitleArtist: 0,
      FuzzyTitle: 0,
      AudioFingerprint: 0,
    };
    for (const g of groups) c[g.kind ?? "ExactTitleArtist"]++;
    return c;
  }, [groups]);

  /**
   * Apply one rule across every group at once — the spec's `Prefer`.
   *
   * The heuristic lives in Rust so it is testable and inspectable rather than
   * buried here, and so the chat panel could use the same one later.
   */
  const applyPrefer = async (rule: PreferRule) => {
    try {
      const cueIds = new Set<string>();
      const playlistCounts = new Map<string, number>();
      const keepers = await preselectKeepers(
        groups.map((g) => candidatesOf(g, cueIds, playlistCounts)),
        rule,
      );
      setKeepByGroup((prev) => {
        const next = { ...prev };
        groups.forEach((g, idx) => {
          const keeper = keepers[idx];
          if (keeper) next[groupId(g, idx)] = keeper;
        });
        return next;
      });
      toast({
        variant: "success",
        message: `Keeper preselected in ${keepers.filter(Boolean).length} group(s).`,
      });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  };

  /**
   * Resolve a group: archive the losers and re-point playlists at the keeper.
   *
   * The re-pointing is the part that makes this safe. Archiving a losing copy
   * without it leaves a hole in every set the loser was in — which the user
   * discovers on stage. The plan is shown before it runs, per the spec's
   * Review step.
   */
  const handleArchiveRest = async (g: DuplicateGroup, gid: string) => {
    const keepId = keepByGroup[gid] ?? g.tracks[0]?.id ?? "";
    const toArchive = g.tracks.filter((t) => t.id !== keepId).map((t) => t.id);
    if (toArchive.length === 0) return;
    setBusyGroup(gid);
    try {
      const plan = await planDuplicateResolution(libraryPath, keepId, toArchive);
      const keeperTitle = g.tracks.find((t) => t.id === keepId)?.title ?? "";
      const playlists = [...new Set(plan.repoint.map(([, name]) => name))];
      const ok = await dialog.confirm({
        title: `Archive ${toArchive.length} duplicate(s)?`,
        body:
          `Keeping “${keeperTitle}”. The rest go to the Archive — nothing is deleted.` +
          (playlists.length > 0
            ? ` ${playlists.length} playlist(s) will be re-pointed at the keeper: ${playlists.join(", ")}.`
            : " No playlist held a duplicate, so nothing needs re-pointing."),
        confirmLabel: "Archive and re-point",
      });
      if (!ok) return;

      const result = await resolveDuplicates(libraryPath, plan);
      toast({
        variant: "success",
        message: `Archived ${result.archived.length} duplicate(s).`,
        detail:
          result.staged.length > 0
            ? `${result.staged.length} playlist change(s) staged for review.`
            : `Kept “${keeperTitle}”.`,
      });
      // Drop this group from the list so the user sees progress.
      setGroups((prev) => prev.filter((_, idx) => groupId(prev[idx], idx) !== gid));
    } catch (e) {
      toast({
        variant: "error",
        message: "Failed to archive duplicates.",
        detail: String(e),
      });
    } finally {
      setBusyGroup(null);
    }
  };

  return (
    <div className="flex min-w-0 flex-1 flex-col bg-base animate-in fade-in duration-200">
      <header className="flex shrink-0 items-start justify-between border-b border-edge/60 px-6 py-5">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-ink">
            Duplicates
          </h1>
          <p className="mt-1 text-[13px] text-ink-secondary">
            {loading
              ? "Scanning library…"
              : `${groups.length} duplicate group${groups.length === 1 ? "" : "s"} found across exact, fuzzy, and audio-fingerprint matches.`}
          </p>
          {!loading && groups.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-2 text-[11px]">
              <span className="rounded bg-accent/15 px-2 py-0.5 text-accent-hover">
                Exact: {counts.ExactTitleArtist}
              </span>
              <span className="rounded bg-amber-500/15 px-2 py-0.5 text-amber-500">
                Fuzzy: {counts.FuzzyTitle}
              </span>
              <span className="rounded bg-violet-500/15 px-2 py-0.5 text-violet-400">
                Fingerprint: {counts.AudioFingerprint}
              </span>
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          {groups.length > 0 && (
            <label className="text-xs text-ink-muted">
              <span className="mr-1">Prefer</span>
              <select
                aria-label="Prefer rule"
                className="rounded border border-edge bg-surface px-2 py-1 text-xs text-ink"
                defaultValue=""
                onChange={(e) => {
                  if (e.target.value) void applyPrefer(e.target.value as PreferRule);
                  e.target.value = "";
                }}
              >
                <option value="">Choose a rule…</option>
                {PREFER_RULES.map((r) => (
                  <option key={r.value} value={r.value}>
                    {r.label}
                  </option>
                ))}
              </select>
            </label>
          )}
          <button
            onClick={() => void refresh()}
            disabled={loading}
            className="rounded border border-edge bg-surface px-3 py-1 text-sm text-ink hover:border-edge-strong disabled:opacity-50"
          >
            {loading ? "Scanning…" : "Re-scan"}
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-6 py-4">
        {loading && (
          <div
            role="status"
            aria-label="Scanning"
            className="flex items-center justify-center py-12"
          >
            <div className="h-6 w-6 animate-spin rounded-full border-2 border-edge-strong border-t-accent-hover" />
          </div>
        )}

        {!loading && error && (
          <div className="rounded border border-red-500/40 bg-red-500/10 p-4 text-sm text-red-400">
            {error}
          </div>
        )}

        {!loading && !error && groups.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <div className="text-base font-medium text-ink">
              No duplicate candidates found.
            </div>
            <p className="mt-1 max-w-md text-sm text-ink-secondary">
              Your library looks clean. Audio-fingerprint matches require the
              chromagram cache to be populated (Phase 20 analysis).
            </p>
          </div>
        )}

        {!loading &&
          !error &&
          groups.map((g, idx) => {
            const gid = groupId(g, idx);
            const kind = g.kind ?? "ExactTitleArtist";
            const confidencePct = Math.round((g.confidence ?? 1) * 100);
            const keepId = keepByGroup[gid] ?? g.tracks[0]?.id ?? "";
            const busy = busyGroup === gid;
            return (
              <section
                key={gid}
                data-testid="duplicate-group"
                className="overflow-hidden rounded-md border border-edge bg-surface"
              >
                <header className="flex items-center justify-between gap-2 border-b border-edge/60 bg-base/40 px-3 py-2">
                  <div className="flex items-center gap-2 text-xs">
                    <span
                      className={`rounded px-2 py-0.5 font-medium ${KIND_TINT[kind]}`}
                    >
                      {KIND_LABEL[kind]}
                    </span>
                    <span className="text-ink-secondary">
                      {g.tracks.length} tracks
                    </span>
                    <span className="text-ink-muted">·</span>
                    <span className="font-mono tabular-nums text-ink-muted">
                      {confidencePct}%
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() =>
                        setDeletingIds(
                          g.tracks.filter((t) => t.id !== keepId).map((t) => t.id),
                        )
                      }
                      disabled={busy || g.tracks.length < 2}
                      className="rounded border border-red-500/40 px-3 py-1 text-xs font-medium text-red-500 hover:bg-red-500/10 disabled:opacity-50"
                      data-testid="delete-rest-from-disk"
                    >
                      Delete rest from disk
                    </button>
                    <button
                      onClick={() => void handleArchiveRest(g, gid)}
                      disabled={busy || g.tracks.length < 2}
                      className="rounded bg-accent px-3 py-1 text-xs font-medium text-base hover:opacity-90 disabled:opacity-50"
                      data-testid="archive-rest"
                    >
                      {busy ? "Archiving…" : "Keep one, archive rest"}
                    </button>
                  </div>
                </header>
                <ul className="divide-y divide-edge/40">
                  {g.tracks.map((t) => {
                    const selected = t.id === keepId;
                    return (
                      <li
                        key={t.id}
                        className="flex items-center gap-3 px-3 py-2 text-sm"
                      >
                        <label className="flex items-center gap-2">
                          <input
                            type="radio"
                            name={`keep-${gid}`}
                            checked={selected}
                            onChange={() =>
                              setKeepByGroup((p) => ({ ...p, [gid]: t.id }))
                            }
                            aria-label={`Keep ${t.title}`}
                          />
                          <span className="text-[10px] uppercase tracking-wider text-ink-muted">
                            Keep
                          </span>
                        </label>
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-ink">{t.title}</div>
                          <div className="truncate text-xs text-ink-secondary">
                            {t.artist ?? "—"}
                            {t.folder_path && (
                              <span className="ml-2 font-mono text-[10px] text-ink-faint">
                                {t.folder_path}
                              </span>
                            )}
                          </div>
                        </div>
                        <div className="hidden font-mono text-[11px] tabular-nums text-ink-muted sm:block">
                          {t.bpm ? `${t.bpm.toFixed(1)} BPM` : ""}{" "}
                          {t.musical_key ?? ""}
                        </div>
                        {onOpenInspector && (
                          <button
                            onClick={() => onOpenInspector(t)}
                            className="rounded border border-edge bg-base px-2 py-1 text-[11px] text-ink-secondary hover:border-edge-strong hover:text-ink"
                            data-testid="open-inspector"
                          >
                            Open in inspector
                          </button>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </section>
            );
          })}
      </div>

      {deletingIds && (
        <DeleteFromDiskDialog
          libraryPath={libraryPath}
          trackIds={deletingIds}
          reason="Duplicate resolution"
          onClose={() => setDeletingIds(null)}
          onDeleted={(receipt) =>
            toast({
              variant: "success",
              message: `Moved ${receipt.manifest.entries.length} file(s) to the deleted-audio folder.`,
              detail: "Restore or empty the batch in Settings.",
            })
          }
        />
      )}
    </div>
  );
}
