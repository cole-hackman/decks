import { useCallback, useState } from "react";
import { createBackup, pickAndInspectBackup, restoreBackup } from "../ipc";
import { useDialog } from "../hooks/useDialog";
import { useToast } from "./Toast";
import type { BackupSummary } from "../types";

interface Props {
  /** Matches the padding the surrounding settings sections use. */
  className?: string;
}

/** `staged_changes` → `Staged changes`, for the contents list. */
function label(table: string): string {
  const words = table.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/**
 * Database Backup.
 *
 * Sync already takes a timestamped copy of `master.db` before its first write.
 * This is the other half: everything `decks` knows that Rekordbox does not —
 * custom tags, the archive, smartlists, staged changes, path mappings, watch
 * folders — which lived only in the local cache and could not be moved to
 * another machine or recovered after a mistake.
 *
 * **Restoring replaces.** The spec flags that loudly and so does this: the file
 * is inspected and its contents shown *before* the confirm, so the user knows
 * what they are swapping in rather than what they are swapping out.
 */
export function BackupSection({ className }: Props) {
  const { toast } = useToast();
  const dialog = useDialog();
  const [busy, setBusy] = useState(false);

  const create = useCallback(async () => {
    setBusy(true);
    try {
      const summary = await createBackup();
      // A cancelled dialog is a decision, not a failure.
      if (!summary) return;
      toast({
        variant: "success",
        message: `Backed up ${summary.rows} row(s).`,
        detail: summary.path,
      });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [toast]);

  const restore = useCallback(async () => {
    setBusy(true);
    try {
      let summary: BackupSummary | null;
      try {
        summary = await pickAndInspectBackup();
      } catch (e) {
        // A file that is not a backup is caught on read, before anything is
        // deleted — which is the whole reason inspect is a separate step.
        toast({ variant: "error", message: String(e) });
        return;
      }
      if (!summary) return;

      const contents = summary.tables
        .map(([table, rows]) => `${label(table)}: ${rows}`)
        .join(" · ");
      const ok = await dialog.confirm({
        title: "Replace local data with this backup?",
        body: `This deletes the tags, archive, smartlists, staged changes and settings currently on this computer and replaces them with the backup's (${contents || "empty"}). Your music files and Rekordbox library are not touched. This cannot be undone.`,
        confirmLabel: "Replace",
        destructive: true,
      });
      if (!ok) return;

      const report = await restoreBackup(summary.path);
      const rows = report.restored.reduce((n, [, count]) => n + count, 0);
      const notes: string[] = [];
      if (report.unknown_tables.length > 0) {
        notes.push(`${report.unknown_tables.length} unknown table(s) skipped`);
      }
      if (report.dropped_columns.length > 0) {
        notes.push(`${report.dropped_columns.length} unknown column(s) dropped`);
      }
      toast({
        variant: "success",
        message: `Restored ${rows} row(s).`,
        detail: notes.length > 0 ? notes.join(", ") : undefined,
      });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [dialog, toast]);

  return (
    <section className={className} aria-label="Database backup">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
        Database Backup
      </h3>
      <p className="mb-3 text-[11px] text-ink-faint">
        Saves what only <code>decks</code> holds — custom tags, the archive,
        smartlists, staged changes and settings. Not your music files, and not
        the Rekordbox library, which Sync backs up separately before writing.
        Restoring <strong>replaces</strong> what is on this computer.
      </p>

      <div className="flex flex-wrap items-center gap-2 text-xs">
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-edge-strong px-3 py-1 hover:bg-elevated disabled:opacity-50"
          onClick={() => void create()}
        >
          Create backup…
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-edge-strong px-3 py-1 text-red-400 hover:bg-elevated disabled:opacity-50"
          onClick={() => void restore()}
        >
          Restore from backup…
        </button>
      </div>

      <p className="mt-2 text-[11px] text-ink-faint" data-testid="backup-retention-note">
        Backups are kept wherever you save them and are never deleted
        automatically — unlike Lexicon, which removes its own after a month.
      </p>
    </section>
  );
}
