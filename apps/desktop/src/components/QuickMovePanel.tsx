import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyOrganize,
  deleteQuickMoveFolder,
  listQuickMoveFolders,
  previewOrganize,
  recordQuickMoveFolder,
  toggleQuickMoveFavourite,
} from "../ipc";
import { useToast } from "./Toast";
import type { QuickMoveFolder } from "../types";

/** Favourites get hotkeys 1–9; anything past the ninth is click-only. */
const MAX_HOTKEYS = 9;

interface Props {
  libraryPath: string;
  trackIds: string[];
  /** Applied on the way, so a quick move can also tidy the filename. */
  renamePattern?: string | null;
  onMoved?: () => void;
}

/**
 * Quick move — send the selection to a remembered folder in one action.
 *
 * Favourited folders get hotkeys 1–9, which is the point of the feature: a DJ
 * filing a night's downloads wants one keystroke per track, not a folder
 * browser per track. Recently-used folders are remembered automatically.
 *
 * The move itself is the same planner as Move & Rename, so collisions and the
 * relocation staging behave identically.
 */
export function QuickMovePanel({
  libraryPath,
  trackIds,
  renamePattern,
  onMoved,
}: Props) {
  const { toast } = useToast();
  const [folders, setFolders] = useState<QuickMoveFolder[]>([]);
  const [newPath, setNewPath] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const list = await listQuickMoveFolders();
      setFolders(Array.isArray(list) ? list : []);
    } catch {
      setFolders([]);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const hotkeyed = useMemo(
    () => folders.filter((f) => f.favourite).slice(0, MAX_HOTKEYS),
    [folders],
  );

  const moveTo = useCallback(
    async (folder: string) => {
      if (trackIds.length === 0 || busy) return;
      setBusy(true);
      try {
        const rows = await previewOrganize(libraryPath, trackIds, {
          target_folder: folder,
          filename_pattern: renamePattern?.trim() ? renamePattern : null,
          subfolders: { levels: [] },
        });
        const changing = rows.filter((r) => r.destination != null);
        if (changing.length === 0) {
          toast({ variant: "info", message: "Everything is already there." });
          return;
        }
        const result = await applyOrganize(libraryPath, changing);
        await recordQuickMoveFolder(folder);
        await refresh();
        if (result.failed.length > 0) {
          toast({
            variant: "error",
            message: `Moved ${result.moved.length}, failed ${result.failed.length}: ${result.failed[0][1]}`,
          });
        } else {
          toast({
            variant: "success",
            message: `Moved ${result.moved.length} file(s) to ${folder}. A full sync clears the old locations.`,
          });
        }
        onMoved?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [libraryPath, trackIds, renamePattern, busy, refresh, onMoved, toast],
  );

  // Hotkeys 1–9 pick a favourite. Ignored while typing, so the folder field
  // does not fire a move on every digit.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        return;
      }
      const n = Number(e.key);
      if (!Number.isInteger(n) || n < 1 || n > MAX_HOTKEYS) return;
      const folder = hotkeyed[n - 1];
      if (!folder) return;
      e.preventDefault();
      void moveTo(folder.path);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hotkeyed, moveTo]);

  return (
    <section className="border-t border-border px-4 py-3" aria-label="Quick move">
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Quick Move
      </h3>
      <p className="mb-2 text-[11px] text-muted">
        Sends the {trackIds.length} selected track(s) straight to a folder.
        Favourites get hotkeys 1–9. After moving, a <em>full</em> sync is needed —
        a partial one leaves the old locations behind.
      </p>

      {folders.length === 0 ? (
        <p className="mb-2 text-xs text-muted" data-testid="no-quick-move-folders">
          No folders yet. Add one below and it will be remembered.
        </p>
      ) : (
        <ul className="mb-2 space-y-0.5 text-xs">
          {folders.map((f) => {
            const index = hotkeyed.findIndex((h) => h.id === f.id);
            return (
              <li key={f.id} className="flex items-center gap-2">
                <span className="w-4 text-center font-mono text-muted">
                  {index >= 0 ? index + 1 : ""}
                </span>
                <button
                  type="button"
                  disabled={busy || trackIds.length === 0}
                  className="truncate font-mono hover:underline disabled:opacity-50"
                  onClick={() => void moveTo(f.path)}
                >
                  {f.path}
                </button>
                <button
                  type="button"
                  aria-label={
                    f.favourite
                      ? `Unfavourite ${f.path}`
                      : `Favourite ${f.path}`
                  }
                  className={`ml-auto ${f.favourite ? "text-amber-400" : "text-muted hover:text-amber-400"}`}
                  onClick={async () => {
                    await toggleQuickMoveFavourite(f.id);
                    await refresh();
                  }}
                >
                  ★
                </button>
                <button
                  type="button"
                  aria-label={`Forget ${f.path}`}
                  className="text-muted hover:text-red-400"
                  onClick={async () => {
                    await deleteQuickMoveFolder(f.id);
                    await refresh();
                  }}
                >
                  ✕
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <div className="flex gap-2 text-xs">
        <input
          aria-label="New quick move folder"
          className="flex-1 rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
          placeholder="/Users/you/Music/House"
          value={newPath}
          onChange={(e) => setNewPath(e.target.value)}
        />
        <button
          type="button"
          disabled={newPath.trim() === ""}
          className="rounded border border-border px-2 py-1 hover:bg-surface-hover disabled:opacity-50"
          onClick={async () => {
            try {
              await recordQuickMoveFolder(newPath.trim());
              setNewPath("");
              await refresh();
            } catch (e) {
              toast({ variant: "error", message: String(e) });
            }
          }}
        >
          Remember
        </button>
      </div>
    </section>
  );
}
