import { useCallback, useEffect, useMemo, useState } from "react";
import { tagRecipeApply, tagRecipePreview } from "../ipc";
import { useToast } from "./Toast";
import type { TagProposal, TagRecipe } from "../types";

type Op = TagRecipe["op"];

const OPS: { value: Op; label: string }[] = [
  { value: "import_from_text", label: "Import tags from text" },
  { value: "add_tags", label: "Add tags" },
  { value: "remove_tags", label: "Remove tags" },
  { value: "replace_tag", label: "Replace tag" },
  { value: "clear_tags", label: "Clear all tags" },
];

function splitTags(input: string): string[] {
  return input
    .split(",")
    .map((t) => t.trim())
    .filter((t) => t !== "");
}

interface Props {
  libraryPath: string;
  trackIds: string[];
  /** Fields a recipe may read, for the import source picker. */
  fields: string[];
}

/**
 * Tag recipes.
 *
 * Separate from the field recipes because tags live in the local cache rather
 * than `master.db`: there is no sync step to carry them, so these apply
 * directly. The preview still exists — running anything over a few hundred
 * tracks deserves a look first regardless of where it lands.
 */
export function TagRecipesSection({ libraryPath, trackIds, fields }: Props) {
  const { toast } = useToast();
  const [op, setOp] = useState<Op>("import_from_text");
  // Comment is the spec's default source, but only if it is actually on
  // offer — a select whose value matches no option silently shows the first
  // one, so the form would lie about what it is about to do.
  const [field, setField] = useState("comment");
  useEffect(() => {
    if (fields.length > 0 && !fields.includes(field)) {
      setField(fields.includes("comment") ? "comment" : fields[0]);
    }
  }, [fields, field]);
  const [separator, setSeparator] = useState("#");
  const [tagList, setTagList] = useState("");
  const [replaceFrom, setReplaceFrom] = useState("");
  const [replaceTo, setReplaceTo] = useState("");
  const [proposals, setProposals] = useState<TagProposal[] | null>(null);
  const [busy, setBusy] = useState(false);

  const recipe = useMemo((): TagRecipe => {
    switch (op) {
      case "import_from_text":
        return { op, field, separator };
      case "add_tags":
        return { op, tags: splitTags(tagList) };
      case "remove_tags":
        return { op, tags: splitTags(tagList) };
      case "replace_tag":
        return { op, from: replaceFrom, to: replaceTo };
      case "clear_tags":
        return { op };
    }
  }, [op, field, separator, tagList, replaceFrom, replaceTo]);

  const runPreview = useCallback(async () => {
    setBusy(true);
    try {
      setProposals(await tagRecipePreview(libraryPath, trackIds, recipe));
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, trackIds, recipe, toast]);

  const apply = useCallback(async () => {
    if (!proposals || proposals.length === 0) return;
    setBusy(true);
    try {
      const result = await tagRecipeApply(libraryPath, proposals);
      const parts = [`${result.tracks_changed} track(s) updated`];
      if (result.tags_created.length > 0) {
        parts.push(`created ${result.tags_created.join(", ")}`);
      }
      toast({ variant: "success", message: `${parts.join(" — ")}.` });
      setProposals(null);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, proposals, toast]);

  return (
    <section className="border-t border-border px-4 py-3" aria-label="Tag recipes">
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Tag Recipes
      </h3>
      <p className="mb-2 text-[11px] text-muted">
        Tags live in the local cache, so these apply directly rather than staging
        for sync. Importing from text is safe to re-run — existing tags are kept.
      </p>

      <div className="mb-2 flex flex-wrap items-end gap-2 text-xs">
        <label>
          <span className="mb-1 block text-muted">Tag operation</span>
          <select
            aria-label="Tag operation"
            className="rounded border border-border bg-surface px-2 py-1 text-xs"
            value={op}
            onChange={(e) => {
              setOp(e.target.value as Op);
              setProposals(null);
            }}
          >
            {OPS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>

        {op === "import_from_text" && (
          <>
            <label>
              <span className="mb-1 block text-muted">From field</span>
              <select
                aria-label="Import source field"
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
            <label>
              <span className="mb-1 block text-muted">Marker</span>
              <input
                aria-label="Tag marker"
                className="w-16 rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
                value={separator}
                onChange={(e) => setSeparator(e.target.value)}
              />
            </label>
          </>
        )}

        {(op === "add_tags" || op === "remove_tags") && (
          <label className="flex-1">
            <span className="mb-1 block text-muted">Tags (comma-separated)</span>
            <input
              aria-label="Tags"
              className="w-full rounded border border-border bg-surface px-2 py-1 text-xs"
              placeholder="Techno, Vocals"
              value={tagList}
              onChange={(e) => setTagList(e.target.value)}
            />
          </label>
        )}

        {op === "replace_tag" && (
          <>
            <label>
              <span className="mb-1 block text-muted">Replace</span>
              <input
                aria-label="Replace tag"
                className="rounded border border-border bg-surface px-2 py-1 text-xs"
                value={replaceFrom}
                onChange={(e) => setReplaceFrom(e.target.value)}
              />
            </label>
            <label>
              <span className="mb-1 block text-muted">With</span>
              <input
                aria-label="With tag"
                className="rounded border border-border bg-surface px-2 py-1 text-xs"
                value={replaceTo}
                onChange={(e) => setReplaceTo(e.target.value)}
              />
            </label>
          </>
        )}

        <button
          type="button"
          disabled={busy || trackIds.length === 0}
          className="rounded border border-border px-3 py-1 hover:bg-surface-hover disabled:opacity-50"
          onClick={() => void runPreview()}
        >
          Preview
        </button>
        <button
          type="button"
          disabled={busy || !proposals || proposals.length === 0}
          className="rounded bg-accent px-3 py-1 text-white hover:bg-accent-hover disabled:opacity-50"
          onClick={() => void apply()}
        >
          Apply to {proposals?.length ?? 0} track(s)
        </button>
      </div>

      {proposals != null && (
        <div data-testid="tag-recipe-preview">
          {proposals.length === 0 ? (
            <p className="text-xs text-muted">
              No tag changes — nothing to do on the selected tracks.
            </p>
          ) : (
            <ul className="max-h-48 space-y-0.5 overflow-auto text-xs">
              {proposals.map((p) => (
                <li key={p.track_id} className="flex flex-wrap items-center gap-2">
                  <span className="truncate">{p.track_title}</span>
                  {p.added.map((t) => (
                    <span
                      key={`+${t}`}
                      className="rounded bg-emerald-500/15 px-1 text-emerald-400"
                    >
                      +{t}
                    </span>
                  ))}
                  {p.removed.map((t) => (
                    <span
                      key={`-${t}`}
                      className="rounded bg-red-500/15 px-1 text-red-400"
                    >
                      −{t}
                    </span>
                  ))}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
