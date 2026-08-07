import { useCallback, useEffect, useState } from "react";
import {
  createFieldMapping,
  deleteFieldMapping,
  listFieldMappings,
  mappableTagTargets,
} from "../ipc";
import { useToast } from "./Toast";
import type { FieldMappingRow, MappingSource } from "../types";

const SOURCES: { value: string; label: string; source: MappingSource }[] = [
  { value: "energy", label: "Energy", source: { kind: "energy" } },
  { value: "danceability", label: "Danceability", source: { kind: "danceability" } },
  { value: "popularity", label: "Popularity", source: { kind: "popularity" } },
  { value: "happiness", label: "Happiness", source: { kind: "happiness" } },
  { value: "all_custom_tags", label: "All custom tags", source: { kind: "all_custom_tags" } },
  { value: "colour", label: "Colour", source: { kind: "colour" } },
];

function sourceLabel(source: MappingSource): string {
  if (source.kind === "tag_category") return `Tag category: ${source.name}`;
  return SOURCES.find((s) => s.value === source.kind)?.label ?? source.kind;
}

interface Props {
  className?: string;
}

/**
 * Field Mappings for ID3 tag writing.
 *
 * Energy and custom tags have no frame of their own; a mapping writes them
 * somewhere that exists. Several sources can share a target and combine.
 */
export function FieldMappingsSection({ className }: Props) {
  const { toast } = useToast();
  const [rows, setRows] = useState<FieldMappingRow[]>([]);
  const [targets, setTargets] = useState<string[]>([]);
  const [source, setSource] = useState("energy");
  const [target, setTarget] = useState("");
  const [overwrite, setOverwrite] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const list = await listFieldMappings();
      setRows(Array.isArray(list) ? list : []);
    } catch {
      setRows([]);
    }
  }, []);

  useEffect(() => {
    void refresh();
    // Wrapped rather than chained: a host that does not know this command at
    // all throws synchronously, which `.catch` would not catch — and taking
    // the whole settings panel down over a missing target list is not a
    // trade worth making.
    void (async () => {
      try {
        const t = await mappableTagTargets();
        const list = Array.isArray(t) ? t : [];
        setTargets(list);
        setTarget((prev) => prev || list[0] || "");
      } catch {
        setTargets([]);
      }
    })();
  }, [refresh]);

  const add = useCallback(async () => {
    const chosen = SOURCES.find((s) => s.value === source);
    if (!chosen || target === "") return;
    try {
      await createFieldMapping(chosen.source, target, overwrite);
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  }, [source, target, overwrite, refresh, toast]);

  return (
    <section className={className} aria-label="Field mappings">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
        Field Mappings (file tags)
      </h3>
      <p className="mb-3 text-[11px] text-ink-faint">
        Energy and custom tags have no tag frame of their own. A mapping writes
        them into one that exists — <code>Energy → Comment</code> yields{" "}
        <code>Energy 08</code>. Several sources can share a target and join with
        a comma. Mappings never overwrite a field you ticked in Write Tags.
      </p>

      {rows.length === 0 ? (
        <p className="mb-3 text-xs text-ink-secondary" data-testid="no-field-mappings">
          No mappings. Energy and custom tags are not written to files.
        </p>
      ) : (
        <ul className="mb-3 space-y-1 text-xs">
          {rows.map((r) => (
            <li key={r.id} className="flex items-center gap-2">
              <span>{sourceLabel(r.source)}</span>
              <span className="text-ink-faint">→</span>
              <span className="font-mono">{r.target}</span>
              <span className="text-[11px] text-ink-faint">
                {r.overwrite ? "replaces" : "appends"}
              </span>
              <button
                type="button"
                aria-label={`Remove mapping ${sourceLabel(r.source)}`}
                className="ml-auto text-ink-muted hover:text-red-400"
                onClick={async () => {
                  await deleteFieldMapping(r.id);
                  await refresh();
                }}
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-wrap items-end gap-2 text-xs">
        <label>
          <span className="mb-1 block text-ink-secondary">Source</span>
          <select
            aria-label="Mapping source"
            className="rounded-md border border-edge-strong bg-surface px-2 py-1 text-xs"
            value={source}
            onChange={(e) => setSource(e.target.value)}
          >
            {SOURCES.map((s) => (
              <option key={s.value} value={s.value}>
                {s.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span className="mb-1 block text-ink-secondary">Target</span>
          <select
            aria-label="Mapping target"
            className="rounded-md border border-edge-strong bg-surface px-2 py-1 text-xs"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
          >
            {targets.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            checked={overwrite}
            onChange={(e) => setOverwrite(e.target.checked)}
          />
          Replace existing value
        </label>
        <button
          type="button"
          disabled={target === ""}
          className="rounded-md border border-edge-strong px-3 py-1 hover:bg-elevated disabled:opacity-50"
          onClick={() => void add()}
        >
          Add
        </button>
      </div>
    </section>
  );
}
