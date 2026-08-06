import { useCallback, useEffect, useMemo, useState } from "react";
import {
  deleteHistorySet,
  historySetTracks,
  importHistory,
  listHistorySets,
  previewHistoryAsPlaylist,
  removeHistoryTrack,
  saveHistoryAsPlaylist,
  setHistoryMetadata,
} from "../ipc";
import { useToast } from "./Toast";
import { useDialog } from "../hooks/useDialog";
import type {
  HistoryMatchReport,
  HistorySet,
  HistoryTrack,
  MatchKind,
} from "../types";

interface Props {
  libraryPath: string;
}

/** How strong a re-match is, said plainly. Per ADR-0008 a weaker match is
 *  never presented as if it were a stronger one. */
const MATCH_LABEL: Record<MatchKind, string> = {
  content_id: "same track",
  path: "same file",
  filename: "same filename — the file moved",
  none: "not in the library any more",
};

function formatPlayed(iso: string | null): string {
  if (!iso) return "date unknown";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}

function duration(secs: number | null): string {
  if (secs == null || secs < 0) return "";
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/**
 * Play history — the gig log.
 *
 * Per `docs/lexicon/09-history-backup.md §History`. Sessions import from
 * Rekordbox into snapshot tables, and **the snapshot is what you see**: editing
 * a track later does not rewrite what history says was played. That is the
 * spec's central design decision and the reason a set survives its tracks being
 * deleted from the library.
 *
 * Deleting a set **sticks** — the source id is remembered, so a re-import does
 * not resurrect the practice sessions and false starts you cleared out.
 */
export function HistoryView({ libraryPath }: Props) {
  const { toast } = useToast();
  const { confirm, prompt } = useDialog();
  const [sets, setSets] = useState<HistorySet[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tracks, setTracks] = useState<HistoryTrack[]>([]);
  const [report, setReport] = useState<HistoryMatchReport | null>(null);
  const [busy, setBusy] = useState(false);

  const refreshSets = useCallback(() => {
    if (!libraryPath) return;
    listHistorySets(libraryPath)
      .then((got) => setSets(Array.isArray(got) ? got : []))
      .catch((e: unknown) => toast({ variant: "error", message: String(e) }));
  }, [libraryPath, toast]);

  useEffect(refreshSets, [refreshSets]);

  useEffect(() => {
    if (selectedId == null) {
      setTracks([]);
      return;
    }
    setReport(null);
    historySetTracks(selectedId)
      .then((got) => setTracks(Array.isArray(got) ? got : []))
      .catch(() => setTracks([]));
  }, [selectedId]);

  const selected = useMemo(
    () => sets.find((s) => s.id === selectedId) ?? null,
    [sets, selectedId],
  );

  const run = useCallback(
    async (fn: () => Promise<void>) => {
      setBusy(true);
      try {
        await fn();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [toast],
  );

  const doImport = useCallback(
    () =>
      void run(async () => {
        const r = await importHistory(libraryPath);
        const parts = [`${r.imported} imported`];
        if (r.already_known > 0) parts.push(`${r.already_known} already known`);
        // Say it, so "why is my deleted set not back?" is never a mystery.
        if (r.previously_deleted > 0) {
          parts.push(`${r.previously_deleted} skipped (deleted before)`);
        }
        toast({ variant: "success", message: parts.join(" · ") });
        refreshSets();
      }),
    [libraryPath, refreshSets, run, toast],
  );

  const doDelete = useCallback(
    (set: HistorySet) =>
      void run(async () => {
        const ok = await confirm({
          title: `Delete “${set.name}”?`,
          body: "The set is removed and remembered, so importing again will not bring it back. Your audio files and library are untouched.",
          confirmLabel: "Delete set",
          destructive: true,
        });
        if (!ok) return;
        await deleteHistorySet(libraryPath, set.id);
        if (selectedId === set.id) setSelectedId(null);
        refreshSets();
      }),
    [confirm, libraryPath, refreshSets, run, selectedId],
  );

  const doSaveAsPlaylist = useCallback(
    () =>
      void run(async () => {
        if (!selected) return;
        const got = await previewHistoryAsPlaylist(libraryPath, selected.id);
        setReport(got);
        if (got.matched === 0) {
          toast({
            variant: "info",
            message: "None of these tracks are in the library any more.",
          });
          return;
        }
        const name = await prompt({
          title: "Save as playlist",
          body: `${got.matched} of ${got.matches.length} track(s) matched.`,
          defaultValue: selected.name,
          confirmLabel: "Stage playlist",
        });
        if (name == null || name.trim() === "") return;
        const ids = got.matches
          .map((m) => m.track_id)
          .filter((id): id is string => id != null);
        const staged = await saveHistoryAsPlaylist(libraryPath, name, ids);
        toast({
          variant: "success",
          message: `Staged “${name}” with ${ids.length} track(s).`,
          detail: `${staged.length} change(s) for review.`,
        });
      }),
    [libraryPath, prompt, run, selected, toast],
  );

  return (
    <div className="flex h-full overflow-hidden" aria-label="Play history">
      <div className="flex w-72 shrink-0 flex-col overflow-hidden border-r border-border">
        <div className="flex items-center justify-between border-b border-border px-3 py-2">
          <h2 className="text-xs font-semibold uppercase tracking-wide text-muted">
            History
          </h2>
          <button
            type="button"
            disabled={busy}
            className="rounded border border-border px-2 py-1 text-xs disabled:opacity-50"
            onClick={doImport}
          >
            Import
          </button>
        </div>
        <div className="flex-1 overflow-auto p-2 text-xs">
          {sets.length === 0 ? (
            <p className="text-muted" data-testid="history-empty">
              No sessions yet. Import brings in every set Rekordbox has logged;
              running it again never duplicates them.
            </p>
          ) : (
            <ul className="space-y-0.5" data-testid="history-sets">
              {sets.map((set) => (
                <li key={set.id}>
                  <button
                    type="button"
                    className={`w-full rounded px-2 py-1 text-left ${
                      selectedId === set.id
                        ? "bg-accent/10 text-accent"
                        : "hover:bg-surface-hover"
                    }`}
                    onClick={() => setSelectedId(set.id)}
                  >
                    <span className="block truncate">{set.name}</span>
                    <span className="block truncate text-[11px] text-muted">
                      {formatPlayed(set.played_at)} · {set.track_count} track(s)
                      {set.location != null && ` · ${set.location}`}
                      {set.rating != null && ` · ${set.rating}★`}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-auto p-4 text-xs">
        {selected == null ? (
          <p className="text-muted">Pick a session to see what you played.</p>
        ) : (
          <>
            <div className="mb-3 flex flex-wrap items-end gap-2">
              <div className="mr-auto">
                <h3 className="text-sm font-medium">{selected.name}</h3>
                <p className="text-[11px] text-muted">
                  {formatPlayed(selected.played_at)} · {tracks.length} track(s)
                </p>
              </div>
              <label>
                <span className="mb-1 block text-muted">Rating</span>
                <select
                  aria-label="Set rating"
                  className="rounded border border-border bg-surface px-2 py-1 text-xs"
                  value={selected.rating ?? ""}
                  onChange={(e) =>
                    void run(async () => {
                      await setHistoryMetadata(
                        selected.id,
                        e.target.value === "" ? null : Number(e.target.value),
                        selected.location,
                      );
                      refreshSets();
                    })
                  }
                >
                  <option value="">—</option>
                  {[1, 2, 3, 4, 5].map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span className="mb-1 block text-muted">Location</span>
                <input
                  aria-label="Set location"
                  className="w-40 rounded border border-border bg-surface px-2 py-1 text-xs"
                  defaultValue={selected.location ?? ""}
                  key={selected.id}
                  onBlur={(e) =>
                    void run(async () => {
                      const value = e.target.value.trim();
                      await setHistoryMetadata(
                        selected.id,
                        selected.rating,
                        value === "" ? null : value,
                      );
                      refreshSets();
                    })
                  }
                />
              </label>
              <button
                type="button"
                disabled={busy}
                className="rounded border border-border px-3 py-1 disabled:opacity-50"
                onClick={doSaveAsPlaylist}
              >
                Save as playlist
              </button>
              <button
                type="button"
                disabled={busy}
                className="rounded border border-border px-3 py-1 text-red-500 disabled:opacity-50"
                onClick={() => doDelete(selected)}
              >
                Delete set
              </button>
            </div>

            <p className="mb-2 text-[11px] text-muted">
              {/* The snapshot rule, said out loud — it explains why a row here
                  can differ from the library. */}
              This is what the tracks looked like when you played them. Editing
              them since has not changed this record.
            </p>

            {report != null && (
              <div className="mb-2" data-testid="history-match-report">
                <p className="mb-1">
                  {report.matched} of {report.matches.length} track(s) are still
                  in the library.
                </p>
                <ul className="space-y-0.5 text-[11px]">
                  {report.matches
                    .filter((m) => m.kind !== "content_id")
                    .map((m) => (
                      <li
                        key={m.history_track_id}
                        className={
                          m.kind === "none" ? "text-amber-500" : "text-muted"
                        }
                      >
                        {m.title ?? "Unknown"} — {MATCH_LABEL[m.kind]}
                      </li>
                    ))}
                </ul>
              </div>
            )}

            <ul className="space-y-0.5" data-testid="history-tracks">
              {tracks.map((t) => (
                <li key={t.id} className="flex items-baseline gap-2">
                  <span className="w-6 shrink-0 tabular-nums text-muted">
                    {t.seq}
                  </span>
                  <span className="truncate">
                    {t.title ?? "Unknown title"}
                    <span className="text-muted">
                      {" "}
                      — {t.artist ?? "Unknown artist"}
                    </span>
                  </span>
                  <span className="ml-auto shrink-0 tabular-nums text-muted">
                    {t.bpm != null && `${t.bpm.toFixed(1)} `}
                    {t.musical_key ?? ""} {duration(t.duration_secs)}
                  </span>
                  <button
                    type="button"
                    aria-label={`Remove ${t.title ?? "track"} from this set`}
                    className="shrink-0 text-muted hover:text-red-500"
                    onClick={() =>
                      void run(async () => {
                        await removeHistoryTrack(t.id);
                        setTracks((prev) => prev.filter((x) => x.id !== t.id));
                        refreshSets();
                      })
                    }
                  >
                    ✕
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </div>
  );
}
