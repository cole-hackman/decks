import { useEffect, useState } from "react";
import { AlertTriangleIcon, Trash2Icon, UndoIcon } from "lucide-react";
import { planDeleteFromDisk, deleteFromDisk } from "../ipc";
import type { DeletePlanView, DeleteReceipt } from "../types";
import { formatBytes } from "../lib/bytes";


interface Props {
  libraryPath: string;
  trackIds: string[];
  /** Recorded in the manifest — "Duplicates", "Broken tracks", "Archive cleanup". */
  reason: string;
  onClose: () => void;
  /** Called with the receipt once files have moved. */
  onDeleted?: (receipt: DeleteReceipt) => void;
}

/**
 * The one dialog in `decks` that touches audio.
 *
 * Everything else in the program is safe because it stages: a change is
 * proposed, reviewed and applied, and applying is reversible. Deleting a file
 * is not, so this is built as two operations rather than one — the audio moves
 * into a quarantine folder with a manifest, and emptying that folder is a
 * separate, explicit act in Settings.
 *
 * The dialog is therefore *preview-first*, like the rest of the app. It plans
 * before it asks, and shows every track it will refuse to touch alongside the
 * reason, so agreeing to it means agreeing to a known list rather than to a
 * count.
 */
export function DeleteFromDiskDialog({
  libraryPath,
  trackIds,
  reason,
  onClose,
  onDeleted,
}: Props) {
  const [plan, setPlan] = useState<DeletePlanView | null>(null);
  const [allowPlaylistMembers, setAllowPlaylistMembers] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    let live = true;
    setPlan(null);
    setError(null);
    planDeleteFromDisk(libraryPath, trackIds, reason, allowPlaylistMembers)
      .then((p) => live && setPlan(p))
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [libraryPath, trackIds, reason, allowPlaylistMembers]);

  // Turning the playlist override off must un-arm the confirmation: the list
  // the user agreed to is no longer the list that would be deleted.
  const setOverride = (next: boolean) => {
    setConfirming(false);
    setAllowPlaylistMembers(next);
  };

  const handleDelete = async () => {
    setBusy(true);
    setError(null);
    try {
      const receipt = await deleteFromDisk(
        libraryPath,
        trackIds,
        reason,
        allowPlaylistMembers,
      );
      onDeleted?.(receipt);
      onClose();
    } catch (e) {
      setError(String(e));
      setConfirming(false);
    } finally {
      setBusy(false);
    }
  };

  const count = plan?.deletable.length ?? 0;
  const label = (id: string) => plan?.labels[id] ?? id;
  const blockedByPlaylist =
    plan?.refused.filter((r) => r.reason.kind === "still_in_playlists") ?? [];

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      role="dialog"
      aria-modal="true"
      aria-label="Delete from disk"
    >
      <div className="flex max-h-full w-full max-w-2xl flex-col overflow-hidden rounded-lg border border-edge bg-surface shadow-xl">
        <header className="shrink-0 border-b border-edge px-5 py-4">
          <h2 className="flex items-center gap-2 text-lg font-semibold text-ink">
            <Trash2Icon className="h-4 w-4 text-red-500" />
            Delete from disk
          </h2>
          <p className="mt-1 text-[13px] text-ink-secondary">
            Audio moves to the deleted-audio folder. It stays there, restorable,
            until you empty that batch in Settings.
          </p>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4 text-sm">
          {!plan && !error && (
            <div className="text-ink-muted">Checking {trackIds.length} track(s)…</div>
          )}

          {plan?.no_roots_configured && (
            <div className="rounded border border-amber-500/40 bg-amber-500/10 p-3 text-[13px] text-ink">
              <p className="flex items-center gap-2 font-medium">
                <AlertTriangleIcon className="h-4 w-4 text-amber-500" />
                No music folders are set up yet.
              </p>
              <p className="mt-1 text-ink-secondary">
                Deleting from disk is off until you say where your music lives.
                Add your folders under Settings → Deleted audio, then try again.
              </p>
            </div>
          )}

          {plan && !plan.no_roots_configured && (
            <>
              <p className="text-ink">
                <strong>{count}</strong> file{count === 1 ? "" : "s"} will move to
                the deleted-audio folder
                {count > 0 && <> ({formatBytes(plan.total_bytes)})</>}.
              </p>

              {count > 0 && (
                <ul className="mt-2 max-h-48 overflow-y-auto rounded border border-edge bg-base p-2">
                  {plan.deletable.map((d) => (
                    <li key={d.track_id} className="py-0.5">
                      <div className="text-ink">{label(d.track_id)}</div>
                      <div className="truncate text-[11px] text-ink-faint">
                        {d.source}
                      </div>
                    </li>
                  ))}
                </ul>
              )}

              {plan.refused.length > 0 && (
                <div className="mt-4">
                  <h3 className="text-[13px] font-medium text-ink">
                    {plan.refused.length} will not be touched
                  </h3>
                  <ul
                    className="mt-1 max-h-48 overflow-y-auto rounded border border-edge bg-base p-2"
                    data-testid="refused-list"
                  >
                    {plan.refused.map((r) => (
                      <li key={r.track_id} className="py-0.5">
                        <div className="text-ink">{label(r.track_id)}</div>
                        <div className="text-[11px] text-ink-muted">
                          {r.message}
                        </div>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {(blockedByPlaylist.length > 0 || allowPlaylistMembers) && (
                <label className="mt-4 flex items-start gap-2 text-[13px] text-ink-secondary">
                  <input
                    type="checkbox"
                    checked={allowPlaylistMembers}
                    onChange={(e) => setOverride(e.target.checked)}
                    className="mt-0.5"
                  />
                  <span>
                    Also delete tracks that playlists still use.
                    <span className="block text-[11px] text-ink-faint">
                      Those playlists will point at missing files until you
                      remove the tracks from the library too.
                    </span>
                  </span>
                </label>
              )}
            </>
          )}

          {error && (
            <div className="mt-3 rounded border border-red-500/40 bg-red-500/10 p-2 text-[13px] text-red-400">
              {error}
            </div>
          )}
        </div>

        <footer className="flex shrink-0 items-center justify-between gap-3 border-t border-edge px-5 py-3">
          <span className="flex items-center gap-1 text-[11px] text-ink-faint">
            <UndoIcon className="h-3 w-3" />
            Restorable from Settings → Deleted audio
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded border border-edge px-3 py-1 text-sm text-ink-secondary hover:border-edge-strong hover:text-ink"
            >
              Cancel
            </button>
            {confirming ? (
              <button
                type="button"
                onClick={handleDelete}
                disabled={busy}
                className="rounded bg-red-500 px-3 py-1 text-sm font-medium text-white hover:bg-red-600 disabled:opacity-50"
              >
                {busy
                  ? "Moving…"
                  : `Yes, move ${count} file${count === 1 ? "" : "s"}`}
              </button>
            ) : (
              <button
                type="button"
                onClick={() => setConfirming(true)}
                disabled={count === 0 || busy}
                className="rounded bg-red-500/10 px-3 py-1 text-sm font-medium text-red-500 hover:bg-red-500/20 disabled:opacity-50"
              >
                Delete {count} from disk
              </button>
            )}
          </div>
        </footer>
      </div>
    </div>
  );
}
