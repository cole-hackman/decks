import { useCallback, useState } from "react";
import { writeTagsBulk } from "../ipc";
import { useToast } from "./Toast";
import type { TagFieldSelection } from "../types";

const FIELDS: { key: keyof TagFieldSelection; label: string }[] = [
  { key: "title", label: "Title" },
  { key: "artist", label: "Artist" },
  { key: "album", label: "Album" },
  { key: "genre", label: "Genre" },
  { key: "bpm", label: "BPM" },
  { key: "musical_key", label: "Key" },
  { key: "comment", label: "Comment" },
  { key: "year", label: "Year" },
];

const NONE: TagFieldSelection = {
  title: false,
  artist: false,
  album: false,
  genre: false,
  bpm: false,
  musical_key: false,
  comment: false,
  year: false,
};

interface Props {
  libraryPath: string;
  trackIds: string[];
}

/**
 * Bulk Write Tags.
 *
 * Writes the library's values into the audio files themselves, so they look
 * right in any other program. Per-field selection is the point: writing only
 * titles and leaving everything else alone is the common case for a library
 * whose files already have better tags than the database for some fields.
 *
 * Nothing is selected by default — this writes to files that cannot be rolled
 * back from the staged-change pipeline, so it should take a deliberate click.
 */
export function WriteTagsPanel({ libraryPath, trackIds }: Props) {
  const { toast } = useToast();
  const [selection, setSelection] = useState<TagFieldSelection>(NONE);
  const [busy, setBusy] = useState(false);

  const anySelected = FIELDS.some((f) => selection[f.key]);

  const run = useCallback(async () => {
    setBusy(true);
    try {
      const result = await writeTagsBulk(libraryPath, trackIds, selection);
      const parts = [`Wrote ${result.written.length} file(s)`];
      if (result.skipped.length > 0) {
        parts.push(`${result.skipped.length} had nothing to write`);
      }
      if (result.failed.length > 0) {
        parts.push(`${result.failed.length} failed: ${result.failed[0][1]}`);
      }
      toast({
        variant: result.failed.length > 0 ? "error" : "success",
        message: `${parts.join(", ")}.`,
      });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, trackIds, selection, toast]);

  return (
    <section
      className="border-t border-border px-4 py-3"
      aria-label="Write tags"
    >
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Write Tags
      </h3>
      <p className="mb-2 text-[11px] text-muted">
        Writes the library's values into the files' own tags. Separate from Sync,
        which updates Rekordbox's database. Unticked fields are left untouched, and
        a field the library does not know is never written as blank.
      </p>

      <div className="mb-2 flex flex-wrap gap-x-4 gap-y-1 text-xs">
        {FIELDS.map((f) => (
          <label key={f.key} className="flex items-center gap-1">
            <input
              type="checkbox"
              checked={selection[f.key]}
              onChange={(e) =>
                setSelection({ ...selection, [f.key]: e.target.checked })
              }
            />
            {f.label}
          </label>
        ))}
      </div>

      <div className="flex gap-2 text-xs">
        <button
          type="button"
          disabled={busy || !anySelected || trackIds.length === 0}
          className="rounded bg-accent px-3 py-1 text-white hover:bg-accent-hover disabled:opacity-50"
          onClick={() => void run()}
        >
          Write tags to {trackIds.length} file(s)
        </button>
        {anySelected && (
          <button
            type="button"
            className="rounded border border-border px-2 py-1 hover:bg-surface-hover"
            onClick={() => setSelection(NONE)}
          >
            Clear
          </button>
        )}
      </div>
    </section>
  );
}
