import { useCallback, useEffect, useMemo, useState } from "react";
import {
  csvImportApply,
  csvImportFields,
  csvImportHeaders,
  csvImportPreview,
} from "../ipc";
import { readTextFile } from "../lib/read-file";
import { useToast } from "./Toast";
import type {
  CsvImportColumns,
  CsvImportPreview,
  CsvPlannedRow,
} from "../types";

interface Props {
  libraryPath: string;
}

/** `null` rather than `""` so the backend sees "not configured", not "named ''". */
const NONE = "";

function describe(row: CsvPlannedRow): string {
  switch (row.outcome.kind) {
    case "matched":
      return `${row.outcome.track_title} — ${row.outcome.changes
        .map(([field, , after]) => `${field} → ${after}`)
        .join(", ")}`;
    case "already_current":
      return "already up to date";
    case "unmatched":
      return "no matching track";
    case "ambiguous":
      return `${row.outcome.count} tracks match — cannot tell which`;
  }
}

function toneFor(row: CsvPlannedRow): string {
  switch (row.outcome.kind) {
    case "matched":
      return "text-ink";
    case "ambiguous":
      return "text-amber-500";
    case "unmatched":
      return "text-red-400";
    default:
      return "text-ink-muted";
  }
}

/**
 * Import Tags From CSV.
 *
 * Bulk metadata from a spreadsheet. Rows match on a Location column (a file
 * path) or on Artist + Title together — at least one, because a mapping with
 * neither matches nothing and "0 rows matched" reads as a broken file.
 *
 * Preview-then-stage like everything else that edits in bulk. Rows that matched
 * nothing or matched several tracks are shown with their reason rather than
 * dropped: an import that silently skipped a third of the file would look like
 * it worked.
 */
export function CsvImportSection({ libraryPath }: Props) {
  const { toast } = useToast();
  const [csv, setCsv] = useState("");
  const [headers, setHeaders] = useState<string[]>([]);
  const [fields, setFields] = useState<string[]>([]);
  const [location, setLocation] = useState(NONE);
  const [artist, setArtist] = useState(NONE);
  const [title, setTitle] = useState(NONE);
  const [mappings, setMappings] = useState<[string, string][]>([]);
  const [preview, setPreview] = useState<CsvImportPreview | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    csvImportFields()
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

  const onUpload = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const text = await readTextFile(file);
      try {
        const cols = await csvImportHeaders(text);
        setCsv(text);
        setHeaders(cols);
        setPreview(null);
        // Guess the obvious ones so the common file needs no mapping at all.
        const find = (want: string) =>
          cols.find((c) => c.toLowerCase() === want) ?? NONE;
        setLocation(find("location"));
        setArtist(find("artist"));
        setTitle(find("title"));
        setMappings([]);
      } catch (err) {
        toast({ variant: "error", message: String(err) });
      }
    },
    [toast],
  );

  const columns = useMemo(
    (): CsvImportColumns => ({
      location: location || null,
      artist: artist || null,
      title: title || null,
      fields: mappings,
    }),
    [location, artist, title, mappings],
  );

  // The backend refuses these too; saying so here means the user is not told
  // about it only after clicking Preview.
  const canMatch = Boolean(location) || (Boolean(artist) && Boolean(title));
  const ready = csv !== "" && canMatch && mappings.length > 0;

  const runPreview = useCallback(async () => {
    setBusy(true);
    try {
      setPreview(await csvImportPreview(libraryPath, csv, columns));
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, csv, columns, toast]);

  const stage = useCallback(async () => {
    if (!preview) return;
    setBusy(true);
    try {
      const ids = await csvImportApply(libraryPath, preview.rows);
      toast({
        variant: "success",
        message: `Staged ${ids.length} change(s) for review.`,
      });
      setPreview(null);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, preview, toast]);

  const picker = (
    label: string,
    value: string,
    onChange: (v: string) => void,
  ) => (
    <label>
      <span className="mb-1 block text-muted">{label}</span>
      <select
        aria-label={label}
        className="rounded border border-border bg-surface px-2 py-1 text-xs"
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          setPreview(null);
        }}
      >
        <option value={NONE}>—</option>
        {headers.map((h) => (
          <option key={h} value={h}>
            {h}
          </option>
        ))}
      </select>
    </label>
  );

  return (
    <section
      className="shrink-0 border-t border-border px-4 py-3"
      aria-label="CSV import"
    >
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Import Tags From CSV
      </h3>
      <p className="mb-2 text-[11px] text-muted">
        Rows match on a Location column, or on Artist and Title together. An
        empty cell leaves the field alone rather than clearing it.
      </p>

      <input
        type="file"
        accept=".csv,text/csv"
        aria-label="CSV file"
        className="mb-2 block text-xs"
        onChange={(e) => void onUpload(e)}
      />

      {headers.length > 0 && (
        <>
          <div className="mb-2 flex flex-wrap items-end gap-2 text-xs">
            {picker("Location column", location, setLocation)}
            {picker("Artist column", artist, setArtist)}
            {picker("Title column", title, setTitle)}
          </div>
          {!canMatch && (
            <p className="mb-2 text-[11px] text-amber-500" data-testid="csv-no-match-strategy">
              Choose a Location column, or Artist and Title together — otherwise
              no row can be matched to a track.
            </p>
          )}

          <div className="mb-2 space-y-1 text-xs">
            {mappings.map(([header, field], i) => (
              <div key={i} className="flex items-center gap-2">
                <span className="font-mono">{header}</span>
                <span className="text-muted">→</span>
                <span className="font-mono">{field}</span>
                <button
                  type="button"
                  aria-label={`Remove ${header} mapping`}
                  className="ml-auto text-muted hover:text-red-400"
                  onClick={() => {
                    setMappings(mappings.filter((_, j) => j !== i));
                    setPreview(null);
                  }}
                >
                  Remove
                </button>
              </div>
            ))}
            <ColumnMapper
              headers={headers}
              fields={fields}
              onAdd={(pair) => {
                setMappings([...mappings, pair]);
                setPreview(null);
              }}
            />
          </div>

          <div className="mb-2 flex gap-2 text-xs">
            <button
              type="button"
              disabled={busy || !ready}
              className="rounded border border-border px-3 py-1 hover:bg-surface-hover disabled:opacity-50"
              onClick={() => void runPreview()}
            >
              Preview import
            </button>
            <button
              type="button"
              disabled={busy || !preview || preview.report.changes === 0}
              className="rounded bg-accent px-3 py-1 text-white hover:bg-accent-hover disabled:opacity-50"
              onClick={() => void stage()}
            >
              Stage {preview?.report.changes ?? 0} change(s)
            </button>
          </div>
        </>
      )}

      {preview != null && (
        <div data-testid="csv-import-preview">
          <p className="mb-1 text-[11px] text-muted">
            {preview.report.rows} row(s): {preview.report.matched} to change,{" "}
            {preview.report.already_current} already up to date,{" "}
            {preview.report.unmatched} unmatched, {preview.report.ambiguous}{" "}
            ambiguous.
          </p>
          <ul className="max-h-48 space-y-0.5 overflow-auto text-xs">
            {preview.rows.map((r) => (
              <li key={r.row.line} className="flex items-baseline gap-2">
                <span className="w-8 shrink-0 text-right font-mono text-muted">
                  {r.row.line}
                </span>
                <span className={toneFor(r)}>{describe(r)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

function ColumnMapper({
  headers,
  fields,
  onAdd,
}: {
  headers: string[];
  fields: string[];
  onAdd: (pair: [string, string]) => void;
}) {
  const [header, setHeader] = useState(headers[0] ?? "");
  const [field, setField] = useState(fields[0] ?? "");

  useEffect(() => {
    if (headers.length > 0 && !headers.includes(header)) setHeader(headers[0]);
  }, [headers, header]);
  useEffect(() => {
    if (fields.length > 0 && !fields.includes(field)) setField(fields[0]);
  }, [fields, field]);

  return (
    <div className="flex items-end gap-2">
      <label>
        <span className="mb-1 block text-muted">Column</span>
        <select
          aria-label="Import column"
          className="rounded border border-border bg-surface px-2 py-1 text-xs"
          value={header}
          onChange={(e) => setHeader(e.target.value)}
        >
          {headers.map((h) => (
            <option key={h} value={h}>
              {h}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span className="mb-1 block text-muted">Into field</span>
        <select
          aria-label="Import into field"
          className="rounded border border-border bg-surface px-2 py-1 text-xs"
          value={field}
          onChange={(e) => setField(e.target.value)}
        >
          {fields.map((f) => (
            <option key={f} value={f}>
              {f}
            </option>
          ))}
        </select>
      </label>
      <button
        type="button"
        disabled={header === "" || field === ""}
        className="rounded border border-border px-3 py-1 text-xs hover:bg-surface-hover disabled:opacity-50"
        onClick={() => onAdd([header, field])}
      >
        Add column
      </button>
    </div>
  );
}
