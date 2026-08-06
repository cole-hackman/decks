import { useCallback, useMemo, useState } from "react";
import { saveShareFile, sharePlaylist } from "../ipc";
import { useToast } from "./Toast";
import type { Playlist, ShareColumn, ShareExport, ShareFormat } from "../types";

interface Props {
  libraryPath: string;
  playlists: Playlist[];
}

const FORMATS: { id: ShareFormat; label: string; blurb: string }[] = [
  {
    id: "quick_copy",
    label: "Quick copy",
    blurb: "Artist – title per line, to the clipboard.",
  },
  {
    id: "quick_copy_numbered",
    label: "Quick copy (numbered)",
    blurb: "The same, line-numbered.",
  },
  {
    id: "csv",
    label: "CSV",
    blurb: "Exactly the columns ticked, in the order shown.",
  },
  {
    id: "m3u",
    label: "M3U",
    blurb: "File paths with extended artist and title info.",
  },
  {
    id: "html",
    label: "HTML",
    blurb: "Printer-friendly. Use the browser's Save to PDF for a PDF.",
  },
];

const COLUMNS: { id: ShareColumn; label: string }[] = [
  { id: "title", label: "Title" },
  { id: "artist", label: "Artist" },
  { id: "album", label: "Album" },
  { id: "genre", label: "Genre" },
  { id: "key", label: "Key" },
  { id: "bpm", label: "BPM" },
  { id: "duration", label: "Duration" },
  { id: "rating", label: "Rating" },
  { id: "year", label: "Year" },
  { id: "comment", label: "Comment" },
  { id: "bitrate", label: "Bitrate" },
  { id: "play_count", label: "Plays" },
  { id: "energy", label: "Energy" },
  { id: "path", label: "Path" },
];

/** Mirrors `share::default_columns()` — what the dj-setlist-builder skill reads. */
const DEFAULT_COLUMNS: ShareColumn[] = ["title", "artist", "bpm", "key", "duration"];

/** Formats where the column choice has any effect. */
const COLUMNAR: ShareFormat[] = ["csv", "html"];

/**
 * Share / export a playlist.
 *
 * Per `docs/lexicon/08-streaming.md §Share / export`. The spec is explicit that
 * this is **not** Sync: sharing produces a file, syncing updates a DJ app.
 * Nothing here stages a change or touches `master.db`.
 *
 * Column order follows the order they were ticked, per the spec's "exactly the
 * columns selected, in the order shown". Quick copy goes to the clipboard and
 * never becomes a file.
 */
export function SharePlaylistSection({ libraryPath, playlists }: Props) {
  const { toast } = useToast();
  const [playlistId, setPlaylistId] = useState("");
  const [format, setFormat] = useState<ShareFormat>("csv");
  const [columns, setColumns] = useState<ShareColumn[]>(DEFAULT_COLUMNS);
  const [preview, setPreview] = useState<ShareExport | null>(null);
  const [busy, setBusy] = useState(false);

  const leaves = useMemo(
    () => playlists.filter((p) => p.kind !== "Folder"),
    [playlists],
  );

  const active = FORMATS.find((f) => f.id === format)!;
  const columnar = COLUMNAR.includes(format);

  const toggleColumn = useCallback((id: ShareColumn) => {
    // Order is the tick order, which is what "in the order shown" means for a
    // list the user is building rather than dragging.
    setColumns((prev) =>
      prev.includes(id) ? prev.filter((c) => c !== id) : [...prev, id],
    );
    setPreview(null);
  }, []);

  const render = useCallback(async () => {
    if (playlistId === "") return;
    setBusy(true);
    try {
      setPreview(await sharePlaylist(libraryPath, playlistId, format, columns));
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, playlistId, format, columns, toast]);

  const copy = useCallback(async () => {
    if (!preview) return;
    try {
      await navigator.clipboard.writeText(preview.content);
      toast({
        variant: "success",
        message: `Copied ${preview.track_count} track(s).`,
      });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  }, [preview, toast]);

  const saveFile = useCallback(async () => {
    if (!preview) return;
    setBusy(true);
    try {
      const path = await saveShareFile(format, preview.filename, preview.content);
      if (path != null) {
        toast({ variant: "success", message: `Exported to ${path}` });
      }
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [format, preview, toast]);

  const isClipboard = format.startsWith("quick_copy");

  return (
    <section
      className="shrink-0 border-t border-border px-4 py-3"
      aria-label="Share playlist"
    >
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Share
      </h3>
      <p className="mb-2 text-[11px] text-muted">
        Sharing produces a file. Syncing updates Rekordbox — this does neither
        to your library.
      </p>

      <div className="mb-2 flex flex-wrap items-end gap-2 text-xs">
        <label>
          <span className="mb-1 block text-muted">Playlist</span>
          <select
            aria-label="Playlist to share"
            className="rounded border border-border bg-surface px-2 py-1 text-xs"
            value={playlistId}
            onChange={(e) => {
              setPlaylistId(e.target.value);
              setPreview(null);
            }}
          >
            <option value="">Choose…</option>
            {leaves.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span className="mb-1 block text-muted">Format</span>
          <select
            aria-label="Export format"
            className="rounded border border-border bg-surface px-2 py-1 text-xs"
            value={format}
            onChange={(e) => {
              setFormat(e.target.value as ShareFormat);
              setPreview(null);
            }}
          >
            {FORMATS.map((f) => (
              <option key={f.id} value={f.id}>
                {f.label}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          disabled={busy || playlistId === ""}
          className="rounded border border-border px-3 py-1 hover:bg-surface-hover disabled:opacity-50"
          onClick={() => void render()}
        >
          Preview export
        </button>
        {preview != null &&
          (isClipboard ? (
            <button
              type="button"
              className="rounded bg-accent px-3 py-1 text-white"
              onClick={() => void copy()}
            >
              Copy to clipboard
            </button>
          ) : (
            <button
              type="button"
              disabled={busy}
              className="rounded bg-accent px-3 py-1 text-white disabled:opacity-50"
              onClick={() => void saveFile()}
            >
              Save file
            </button>
          ))}
      </div>

      <p className="mb-2 text-[11px] text-muted" data-testid="share-format-blurb">
        {active.blurb}
      </p>

      {columnar && (
        <div className="mb-2" data-testid="share-columns">
          <span className="mb-1 block text-[11px] text-muted">
            Columns, in the order ticked
          </span>
          <div className="flex flex-wrap gap-1">
            {COLUMNS.map((c) => {
              const index = columns.indexOf(c.id);
              return (
                <button
                  key={c.id}
                  type="button"
                  aria-pressed={index !== -1}
                  className={`rounded border px-2 py-0.5 text-[11px] ${
                    index !== -1
                      ? "border-accent bg-accent/10 text-accent"
                      : "border-border hover:bg-surface-hover"
                  }`}
                  onClick={() => toggleColumn(c.id)}
                >
                  {c.label}
                  {index !== -1 && (
                    <span className="ml-1 tabular-nums opacity-70">
                      {index + 1}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
          {columns.length === 0 && (
            <p className="mt-1 text-[11px] text-muted">
              No columns ticked — the export falls back to title, artist, BPM,
              key and duration.
            </p>
          )}
        </div>
      )}

      {preview != null && (
        <div data-testid="share-preview">
          <p className="mb-1 text-[11px] text-muted">
            {preview.track_count} track(s) · {preview.filename}
          </p>
          {preview.skipped.length > 0 && (
            <p className="mb-1 text-[11px] text-amber-500" data-testid="share-skipped">
              {preview.skipped.length} track(s) have no file path and are not in
              the M3U: {preview.skipped.slice(0, 5).join(", ")}
              {preview.skipped.length > 5 && "…"}
            </p>
          )}
          <pre className="max-h-48 overflow-auto rounded bg-surface p-2 font-mono text-[11px]">
            {preview.content.slice(0, 4000)}
          </pre>
        </div>
      )}
    </section>
  );
}
