import { useCallback, useEffect, useState } from "react";
import { FolderPlusIcon, RotateCcwIcon, Trash2Icon, XIcon } from "lucide-react";
import {
  listDeletedBatches,
  musicRoots,
  purgeDeletedBatch,
  restoreDeletedBatch,
  setMusicRoots,
  suggestMusicRoots,
} from "../ipc";
import { useDialog } from "../hooks/useDialog";
import { useToast } from "./Toast";
import { formatBytes } from "../lib/bytes";
import type { DeleteBatch, MusicRootSuggestion } from "../types";

interface Props {
  /** Needed to suggest roots from the library's own paths. */
  libraryPath: string | null;
  className?: string;
}

function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}

/**
 * Deleted audio — the two halves of the only irreversible feature in `decks`.
 *
 * **Music folders** are the guard. Deleting from disk refuses every file
 * outside them, and with none configured it refuses everything: the feature is
 * off until the user says where their music lives. That is deliberate — a bad
 * path mapping, or a library whose paths point somewhere unexpected, must not
 * be able to walk a bulk delete out of the music collection.
 *
 * **Batches** are the undo. Deleting moves files here with a plain-JSON
 * manifest recording where each came from; Restore puts them back, and only
 * Empty removes them for good. Emptying is per batch and always named, because
 * "empty everything" is the shape of a mistake.
 */
export function DeletedAudioSection({ libraryPath, className }: Props) {
  const { toast } = useToast();
  const dialog = useDialog();
  const [roots, setRoots] = useState<string[]>([]);
  const [batches, setBatches] = useState<DeleteBatch[]>([]);
  const [suggestions, setSuggestions] = useState<MusicRootSuggestion[]>([]);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [r, b] = await Promise.all([musicRoots(), listDeletedBatches()]);
      setRoots(r);
      setBatches(b);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  }, [toast]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const save = async (next: string[]) => {
    await setMusicRoots(next);
    setRoots(next);
  };

  const addRoot = async () => {
    const path = await dialog.prompt({
      title: "Add a music folder",
      body: "The full path to a folder that holds your music. Deleting from disk will refuse anything outside the folders listed here.",
      placeholder: "/Users/you/Music",
    });
    if (!path?.trim()) return;
    try {
      await save([...roots, path.trim()]);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  };

  const removeRoot = async (path: string) => {
    await save(roots.filter((r) => r !== path));
  };

  const suggest = async () => {
    if (!libraryPath) return;
    setBusy(true);
    try {
      const found = await suggestMusicRoots(libraryPath);
      setSuggestions(found.filter((s) => !roots.includes(s.path)));
      if (found.length === 0) {
        toast({
          variant: "info",
          message: "No folders to suggest — the library has no usable paths.",
        });
      }
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const acceptSuggestion = async (path: string) => {
    await save([...roots, path]);
    setSuggestions((prev) => prev.filter((s) => s.path !== path));
  };

  const restore = async (batch: DeleteBatch) => {
    setBusy(true);
    try {
      const report = await restoreDeletedBatch(batch.manifest.batch_id);
      const stuck = report.results.length - report.restored;
      toast({
        variant: report.restored > 0 ? "success" : "info",
        message: `Restored ${report.restored} file(s).`,
        detail:
          stuck > 0
            ? `${stuck} could not be put back — something is already at their original path.`
            : undefined,
      });
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const purge = async (batch: DeleteBatch) => {
    const ok = await dialog.confirm({
      title: `Permanently delete ${batch.file_count} file(s)?`,
      body: `This is the step that cannot be undone. ${formatBytes(batch.total_bytes)} from ${formatDate(batch.manifest.created_at)} will be removed from your computer.`,
      confirmLabel: "Delete permanently",
      destructive: true,
    });
    if (!ok) return;
    setBusy(true);
    try {
      const freed = await purgeDeletedBatch(batch.manifest.batch_id);
      toast({ variant: "success", message: `Freed ${formatBytes(freed)}.` });
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className={className}>
      <h3 className="text-sm font-semibold text-ink">Deleted audio</h3>
      <p className="mt-1 text-[13px] text-ink-secondary">
        Deleting a track from disk moves it here instead of removing it. Nothing
        leaves your computer until you empty a batch.
      </p>

      {/* ── Music folders ── */}
      <div className="mt-4">
        <div className="flex items-center justify-between">
          <h4 className="text-[13px] font-medium text-ink">Music folders</h4>
          <div className="flex gap-2">
            {libraryPath && (
              <button
                type="button"
                onClick={suggest}
                disabled={busy}
                className="rounded border border-edge px-2 py-1 text-xs text-ink-secondary hover:border-edge-strong hover:text-ink disabled:opacity-50"
              >
                Suggest from library
              </button>
            )}
            <button
              type="button"
              onClick={addRoot}
              className="flex items-center gap-1 rounded bg-accent px-2 py-1 text-xs font-medium text-base hover:bg-accent-hover"
            >
              <FolderPlusIcon className="h-3 w-3" />
              Add folder
            </button>
          </div>
        </div>

        {roots.length === 0 ? (
          <p
            className="mt-2 rounded border border-amber-500/40 bg-amber-500/10 p-2 text-[12px] text-ink"
            data-testid="no-music-roots"
          >
            Deleting from disk is off. Add at least one folder to turn it on.
          </p>
        ) : (
          <ul className="mt-2 flex flex-col gap-1">
            {roots.map((root) => (
              <li
                key={root}
                className="flex items-center justify-between rounded border border-edge bg-base px-2 py-1 text-[12px] text-ink"
              >
                <span className="truncate">{root}</span>
                <button
                  type="button"
                  onClick={() => removeRoot(root)}
                  aria-label={`Remove ${root}`}
                  className="text-ink-faint hover:text-red-500"
                >
                  <XIcon className="h-3 w-3" />
                </button>
              </li>
            ))}
          </ul>
        )}

        {suggestions.length > 0 && (
          <ul className="mt-2 flex flex-col gap-1" data-testid="root-suggestions">
            {suggestions.map((s) => (
              <li
                key={s.path}
                className="flex items-center justify-between rounded border border-dashed border-edge px-2 py-1 text-[12px] text-ink-secondary"
              >
                <span className="truncate">
                  {s.path}
                  <span className="ml-2 text-ink-faint">
                    {s.track_count} track{s.track_count === 1 ? "" : "s"}
                  </span>
                </span>
                <button
                  type="button"
                  onClick={() => acceptSuggestion(s.path)}
                  className="rounded border border-edge px-2 py-0.5 text-[11px] hover:border-edge-strong hover:text-ink"
                >
                  Add
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* ── Batches ── */}
      <div className="mt-5">
        <h4 className="text-[13px] font-medium text-ink">Deleted batches</h4>
        {batches.length === 0 ? (
          <p className="mt-2 text-[12px] text-ink-muted">
            Nothing has been deleted from disk.
          </p>
        ) : (
          <ul className="mt-2 flex flex-col gap-2">
            {batches.map((batch) => (
              <li
                key={batch.manifest.batch_id}
                className="rounded border border-edge bg-base p-2"
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="min-w-0">
                    <div className="text-[13px] text-ink">
                      {batch.file_count} file{batch.file_count === 1 ? "" : "s"} ·{" "}
                      {formatBytes(batch.total_bytes)}
                    </div>
                    <div className="text-[11px] text-ink-faint">
                      {formatDate(batch.manifest.created_at)} ·{" "}
                      {batch.manifest.reason}
                    </div>
                  </div>
                  <div className="flex shrink-0 gap-2">
                    <button
                      type="button"
                      onClick={() => restore(batch)}
                      disabled={busy}
                      className="flex items-center gap-1 rounded border border-edge px-2 py-1 text-xs text-ink-secondary hover:border-edge-strong hover:text-ink disabled:opacity-50"
                    >
                      <RotateCcwIcon className="h-3 w-3" />
                      Restore
                    </button>
                    <button
                      type="button"
                      onClick={() => purge(batch)}
                      disabled={busy}
                      className="flex items-center gap-1 rounded bg-red-500/10 px-2 py-1 text-xs font-medium text-red-500 hover:bg-red-500/20 disabled:opacity-50"
                    >
                      <Trash2Icon className="h-3 w-3" />
                      Empty
                    </button>
                  </div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
