import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyOrganize,
  patternFields,
  previewOrganize,
  validatePattern,
} from "../ipc";
import { useToast } from "./Toast";
import { QuickMovePanel } from "./QuickMovePanel";
import { WatchFolderPanel } from "./WatchFolderPanel";
import { UnusedFilesPanel } from "./UnusedFilesPanel";
import { WriteTagsPanel } from "./WriteTagsPanel";
import type {
  OrganizeRow,
  PatternField,
  SubfolderPattern,
  Track,
} from "../types";

/** The subfolder levels the user can pick, in the order the manual lists them. */
const SUBFOLDER_CHOICES: { value: string; label: string; pattern: SubfolderPattern }[] = [
  { value: "genre", label: "Genre", pattern: { kind: "field", name: "genre" } },
  { value: "artist", label: "Artist", pattern: { kind: "field", name: "artist" } },
  { value: "albumTitle", label: "Album", pattern: { kind: "field", name: "albumTitle" } },
  { value: "key", label: "Key", pattern: { kind: "field", name: "key" } },
  { value: "bpm", label: "BPM", pattern: { kind: "field", name: "bpm" } },
  { value: "rating", label: "Rating", pattern: { kind: "field", name: "rating" } },
  { value: "bitrate_bucket", label: "Bitrate (320+ / 320−)", pattern: { kind: "bitrate_bucket" } },
  { value: "first_tag", label: "First tag", pattern: { kind: "first_tag" } },
  { value: "current_year", label: "Current year", pattern: { kind: "current_year" } },
  { value: "current_month", label: "Current month", pattern: { kind: "current_month" } },
  { value: "current_decade", label: "Current decade", pattern: { kind: "current_decade" } },
  { value: "release_decade", label: "Release decade", pattern: { kind: "release_decade" } },
];

/** Lexicon allows three nested levels; so do we. */
const LEVELS = [0, 1, 2];

interface Props {
  libraryPath: string;
  /** The tracks the action applies to — the current selection, or the visible list. */
  tracks: Track[];
  selectedTrackIds: Set<string>;
}

/**
 * Move & Rename.
 *
 * Preview first, always. Bulk file moves over someone's music library are the
 * least forgiving thing this app does, so the plan is shown in full — including
 * the rows that would not change — before anything runs. Applying moves the
 * files and stages a relocation per track; `master.db` only learns the new
 * paths when the user syncs.
 */
export function OrganizeFilesView({ libraryPath, tracks, selectedTrackIds }: Props) {
  const { toast } = useToast();
  const [targetFolder, setTargetFolder] = useState("");
  const [pattern, setPattern] = useState("%artist% - %title%");
  const [levels, setLevels] = useState<string[]>(["", "", ""]);
  const [fields, setFields] = useState<PatternField[]>([]);
  const [patternError, setPatternError] = useState<string | null>(null);
  const [rows, setRows] = useState<OrganizeRow[] | null>(null);
  const [busy, setBusy] = useState(false);

  const targetIds = useMemo(
    () =>
      selectedTrackIds.size > 0
        ? tracks.filter((t) => selectedTrackIds.has(t.id)).map((t) => t.id)
        : tracks.map((t) => t.id),
    [tracks, selectedTrackIds],
  );

  useEffect(() => {
    let cancelled = false;
    patternFields()
      .then((f) => {
        if (!cancelled) setFields(Array.isArray(f) ? f : []);
      })
      .catch(() => {
        if (!cancelled) setFields([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Validate as the user types, so a typo surfaces here rather than as a
  // thousand oddly-named files.
  useEffect(() => {
    let cancelled = false;
    if (pattern.trim() === "") {
      setPatternError(null);
      return;
    }
    validatePattern(pattern)
      .then(() => {
        if (!cancelled) setPatternError(null);
      })
      .catch((e) => {
        if (!cancelled) setPatternError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [pattern]);

  const unsupported = useMemo(() => {
    const known = new Map(fields.map((f) => [f.name, f.supported]));
    const used = [...pattern.matchAll(/%([^%]+)%/g)].map((m) => m[1].trim());
    return used.filter((name) => known.get(name) === false);
  }, [pattern, fields]);

  const request = useCallback(
    () => ({
      target_folder: targetFolder.trim() === "" ? null : targetFolder.trim(),
      filename_pattern: pattern.trim() === "" ? null : pattern,
      subfolders: {
        levels: levels
          .map((v) => SUBFOLDER_CHOICES.find((c) => c.value === v)?.pattern)
          .filter((p): p is SubfolderPattern => p != null),
      },
    }),
    [targetFolder, pattern, levels],
  );

  const runPreview = useCallback(async () => {
    setBusy(true);
    try {
      setRows(await previewOrganize(libraryPath, targetIds, request()));
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, targetIds, request, toast]);

  const changing = useMemo(
    () => (rows ?? []).filter((r) => r.destination != null),
    [rows],
  );

  const apply = useCallback(async () => {
    if (changing.length === 0) return;
    setBusy(true);
    try {
      const result = await applyOrganize(libraryPath, changing);
      if (result.failed.length > 0) {
        toast({
          variant: "error",
          message: `Moved ${result.moved.length}, failed ${result.failed.length}: ${result.failed[0][1]}`,
        });
      } else {
        toast({
          variant: "success",
          message: `Moved ${result.moved.length} file(s). Sync to update Rekordbox.`,
        });
      }
      setRows(null);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, changing, toast]);

  return (
    <div className="flex h-full flex-col overflow-auto p-4" aria-label="Files">
      <header className="mb-3">
        <h2 className="text-sm font-semibold">Files</h2>
        <p className="text-xs text-muted">
          {selectedTrackIds.size > 0
            ? `${targetIds.length} selected track(s)`
            : `All ${targetIds.length} track(s)`}
          {" — everything here writes to disk, not to Rekordbox's database."}
        </p>
      </header>

      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Move &amp; Rename
      </h3>

      <div className="mb-3 grid gap-3 md:grid-cols-2">
        <label className="text-xs">
          <span className="mb-1 block font-medium uppercase tracking-wide text-muted">
            Target folder
          </span>
          <input
            className="w-full rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
            placeholder="Leave empty to rename in place"
            value={targetFolder}
            onChange={(e) => setTargetFolder(e.target.value)}
          />
        </label>

        <label className="text-xs">
          <span className="mb-1 block font-medium uppercase tracking-wide text-muted">
            Filename pattern
          </span>
          <input
            className="w-full rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
            placeholder="Leave empty to keep filenames"
            value={pattern}
            onChange={(e) => setPattern(e.target.value)}
          />
        </label>
      </div>

      {patternError && (
        <p className="mb-2 text-xs text-red-400" role="alert">
          {patternError}
        </p>
      )}
      {unsupported.length > 0 && (
        <p className="mb-2 text-xs text-amber-500">
          decks cannot fill {unsupported.join(", ")} yet — those will render empty.
        </p>
      )}

      <p className="mb-2 text-[11px] text-muted">
        <code>%field%</code> inserts a field. <code>{"{ }"}</code> marks an optional
        segment that disappears entirely when a field inside it is empty — so{" "}
        <code>%artist% - %title% {"{(%key%)}"}</code> leaves no stray brackets on a
        keyless track.
      </p>

      <div className="mb-3">
        <p className="mb-1 text-[11px] font-medium uppercase tracking-wide text-muted">
          Subfolders
        </p>
        <div className="flex flex-wrap gap-2">
          {LEVELS.map((i) => (
            <label key={i} className="text-xs">
              <span className="sr-only">Subfolder level {i + 1}</span>
              <select
                aria-label={`Subfolder level ${i + 1}`}
                className="rounded border border-border bg-surface px-2 py-1 text-xs"
                value={levels[i]}
                onChange={(e) => {
                  const next = [...levels];
                  next[i] = e.target.value;
                  setLevels(next);
                }}
              >
                <option value="">(none)</option>
                {SUBFOLDER_CHOICES.map((c) => (
                  <option key={c.value} value={c.value}>
                    {c.label}
                  </option>
                ))}
              </select>
            </label>
          ))}
        </div>
        <p className="mt-1 text-[11px] text-muted">
          A track missing one of these still moves — it just skips that level.
        </p>
      </div>

      <div className="mb-3 flex gap-2 text-xs">
        <button
          type="button"
          disabled={busy || patternError != null || targetIds.length === 0}
          className="rounded border border-border px-3 py-1 hover:bg-surface-hover disabled:opacity-50"
          onClick={() => void runPreview()}
        >
          Preview
        </button>
        <button
          type="button"
          disabled={busy || changing.length === 0}
          className="rounded bg-accent px-3 py-1 text-white hover:bg-accent-hover disabled:opacity-50"
          onClick={() => void apply()}
        >
          Move {changing.length} file(s)
        </button>
      </div>

      {rows != null && (
        <div data-testid="organize-preview" className="shrink-0 overflow-x-auto">
          {rows.length === 0 ? (
            <p className="text-xs text-muted">No tracks with a file path to move.</p>
          ) : (
            <table className="w-full text-left text-xs">
              <thead className="sticky top-0 bg-surface text-muted">
                <tr>
                  <th className="px-2 py-1 font-medium">Track</th>
                  <th className="px-2 py-1 font-medium">From</th>
                  <th className="px-2 py-1 font-medium">To</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => (
                  <tr key={r.track_id} className="border-t border-border">
                    <td className="px-2 py-1">
                      {r.artist ? `${r.artist} — ` : ""}
                      {r.title}
                    </td>
                    <td className="px-2 py-1 font-mono text-muted">{r.source}</td>
                    <td className="px-2 py-1 font-mono">
                      {r.destination ?? (
                        <span className="text-muted">already in place</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      <div className="mt-4">
        <WatchFolderPanel libraryPath={libraryPath} />
        <QuickMovePanel
          libraryPath={libraryPath}
          trackIds={targetIds}
          renamePattern={pattern}
          onMoved={() => setRows(null)}
        />
        <WriteTagsPanel libraryPath={libraryPath} trackIds={targetIds} />
        <UnusedFilesPanel libraryPath={libraryPath} />
      </div>
    </div>
  );
}
