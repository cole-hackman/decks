import { useCallback, useEffect, useState } from "react";
import {
  createFieldMapping,
  deleteFieldMapping,
  listFieldMappings,
  listTagCategories,
  mappableLibraryTargets,
  mappableTagTargets,
} from "../ipc";
import type { MappingProfile } from "../ipc";
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
  /**
   * Which destination these mappings target.
   *
   * The two genuinely differ — an audio file has no Rating frame worth writing
   * and `djmdContent` has no album-art column — so they are separate profiles
   * rather than one list applied twice, and the target lists differ with them.
   */
  const [profile, setProfile] = useState<MappingProfile>("id3");
  const [source, setSource] = useState("energy");
  /**
   * Categories offered as sources in their own right.
   *
   * Per `docs/lexicon/02-library.md §Custom Tags`: "a single category can be
   * the source instead" of all tags. Exporting only Genre into the comment is
   * a different intent from exporting everything, and one the engine has
   * always supported — it was just never offered.
   */
  const [categories, setCategories] = useState<{ id: string; name: string }[]>([]);
  const [target, setTarget] = useState("");
  const [overwrite, setOverwrite] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const list = await listFieldMappings(profile);
      setRows(Array.isArray(list) ? list : []);
    } catch {
      setRows([]);
    }
  }, [profile]);

  useEffect(() => {
    void refresh();
    // Wrapped rather than chained: a host that does not know this command at
    // all throws synchronously, which `.catch` would not catch — and taking
    // the whole settings panel down over a missing target list is not a
    // trade worth making.
    void (async () => {
      try {
        const t =
          profile === "rekordbox"
            ? await mappableLibraryTargets()
            : await mappableTagTargets();
        const list = Array.isArray(t) ? t : [];
        setTargets(list);
        // Reset rather than preserve: a target valid for one destination may
        // not exist in the other, and keeping a stale one would offer a
        // mapping that silently does nothing.
        setTarget(list[0] ?? "");
      } catch {
        setTargets([]);
      }
    })();
    void (async () => {
      try {
        const cats = await listTagCategories();
        setCategories(Array.isArray(cats) ? cats : []);
      } catch {
        // A tag tree we cannot read just means no per-category sources, not a
        // broken settings panel.
        setCategories([]);
      }
    })();
  }, [refresh, profile]);

  const add = useCallback(async () => {
    // Category sources carry the category *name*, not its id — the mapping is
    // stored and matched by name, so a renamed category stops matching rather
    // than silently exporting the wrong set under the old label.
    const categoryName = source.startsWith("category:")
      ? categories.find((c) => c.id === source.slice("category:".length))?.name
      : undefined;
    const chosen: MappingSource | undefined = categoryName
      ? { kind: "tag_category", name: categoryName }
      : SOURCES.find((s) => s.value === source)?.source;
    if (!chosen || target === "") return;
    try {
      await createFieldMapping(chosen, target, overwrite, profile);
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  }, [source, target, overwrite, categories, profile, refresh, toast]);

  return (
    <section className={className} aria-label="Field mappings">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
        Field Mappings
      </h3>
      <div className="mb-3 flex gap-1" role="group" aria-label="Mapping destination">
        {(
          [
            ["id3", "File tags"],
            ["rekordbox", "Rekordbox library"],
          ] as [MappingProfile, string][]
        ).map(([value, label]) => (
          <button
            key={value}
            type="button"
            aria-pressed={profile === value}
            onClick={() => setProfile(value)}
            className={[
              "rounded px-2 py-0.5 text-[11px]",
              profile === value
                ? "bg-accent text-base"
                : "border border-edge text-ink-muted hover:text-ink",
            ].join(" ")}
          >
            {label}
          </button>
        ))}
      </div>
      <p className="mb-3 text-[11px] text-ink-faint">
        Energy and custom tags have no field of their own. A mapping writes them
        into one that exists — <code>Energy → Comment</code> yields{" "}
        <code>Energy 08</code>. Several sources can share a target and join with
        a comma.{" "}
        {profile === "id3"
          ? "Mappings never overwrite a field you ticked in Write Tags."
          : "Library mappings are previewed and staged for review before Sync writes them."}
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
            {categories.map((c) => (
              <option key={c.id} value={`category:${c.id}`}>
                Tag category: {c.name}
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
