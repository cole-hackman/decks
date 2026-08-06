import { useCallback, useEffect, useState } from "react";
import { listUndoRuns, undoRun, undoRunEntries } from "../ipc";
import { useToast } from "./Toast";
import type { UndoEntry, UndoRun } from "../types";

interface Props {
  libraryPath: string;
  /** Called after an undo stages, so the change list can refresh. */
  onStaged?: () => void;
}

function when(secs: number): string {
  return new Date(secs * 1000).toLocaleString();
}

/**
 * Undo History.
 *
 * `decks` gates hard *before* a write, and had no answer for the change you
 * accept and then regret. Undoing stages the **inverse** of a Sync run as
 * ordinary proposed changes, so it goes back through review and the same
 * guarded Sync — two steps rather than one, which is the right trade for a
 * program whose first rule is that the library is read-only.
 *
 * Not every change can be inverted. A run says up front how many of its changes
 * can be put back and how many cannot, and every unreversible entry carries its
 * reason — an undo that quietly restored eight of twelve would be worse than
 * one that restored none.
 */
export function UndoHistoryPanel({ libraryPath, onStaged }: Props) {
  const { toast } = useToast();
  const [runs, setRuns] = useState<UndoRun[]>([]);
  const [openRun, setOpenRun] = useState<string | null>(null);
  const [entries, setEntries] = useState<UndoEntry[]>([]);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const got = await listUndoRuns(libraryPath);
      // Coerced rather than trusted: this panel sits inside the change review
      // view, and a malformed response must not take that whole screen down
      // with it. The history is the least important thing on the page.
      setRuns(Array.isArray(got) ? got : []);
    } catch {
      // A missing history is not worth a toast — the empty state says it.
      setRuns([]);
    }
  }, [libraryPath]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const toggle = useCallback(
    async (runId: string) => {
      if (openRun === runId) {
        setOpenRun(null);
        setEntries([]);
        return;
      }
      setOpenRun(runId);
      try {
        const got = await undoRunEntries(runId);
        setEntries(Array.isArray(got) ? got : []);
      } catch (e) {
        toast({ variant: "error", message: String(e) });
        setEntries([]);
      }
    },
    [openRun, toast],
  );

  const run = useCallback(
    async (runId: string) => {
      setBusy(true);
      try {
        const result = await undoRun(libraryPath, runId);
        const parts = [`Staged ${result.staged.length} change(s) for review`];
        if (result.blocked.length > 0) {
          parts.push(`${result.blocked.length} could not be reversed`);
        }
        toast({ variant: "success", message: `${parts.join(" — ")}.` });
        setOpenRun(null);
        setEntries([]);
        await refresh();
        onStaged?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [libraryPath, refresh, onStaged, toast],
  );

  return (
    <section
      className="shrink-0 border-t border-edge px-4 py-3"
      aria-label="Undo history"
    >
      <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-ink-muted">
        Undo History
      </h3>
      <p className="mb-2 text-[11px] text-ink-muted">
        Undoing a sync stages the reverse of what it wrote, for review — nothing
        is written back without going through Sync again.
      </p>

      {runs.length === 0 ? (
        <p className="text-xs text-ink-muted" data-testid="no-undo-runs">
          No syncs yet. Runs appear here once Sync has written to the library.
        </p>
      ) : (
        <ul className="space-y-1 text-xs">
          {runs.map((r) => (
            <li key={r.id} className="rounded border border-edge">
              <div className="flex flex-wrap items-center gap-2 px-2 py-1.5">
                <button
                  type="button"
                  className="text-left hover:text-accent-hover"
                  aria-label={`Sync of ${when(r.applied_at)}`}
                  onClick={() => void toggle(r.id)}
                >
                  {when(r.applied_at)}
                </button>
                <span className="text-ink-muted">
                  {r.reversible} reversible
                  {r.blocked > 0 ? `, ${r.blocked} not` : ""}
                </span>
                {r.undone_at != null ? (
                  <span className="ml-auto rounded bg-elevated px-1.5 py-0.5 text-[10px] text-ink-muted">
                    Undone
                  </span>
                ) : (
                  <button
                    type="button"
                    disabled={busy || r.reversible === 0}
                    className="ml-auto rounded border border-edge-strong px-2 py-0.5 hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
                    onClick={() => void run(r.id)}
                  >
                    Undo {r.reversible}
                  </button>
                )}
              </div>

              {openRun === r.id && (
                <ul
                  className="max-h-40 space-y-0.5 overflow-auto border-t border-edge px-2 py-1"
                  data-testid="undo-entries"
                >
                  {entries.map((e) => (
                    <li key={e.id} className="flex flex-wrap items-baseline gap-2">
                      <span className="font-mono text-[11px]">{e.description}</span>
                      {e.blocked_reason != null && (
                        <span className="text-[11px] text-amber-500">
                          {e.blocked_reason}
                        </span>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
