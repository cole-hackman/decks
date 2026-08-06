import { useCallback, useEffect, useMemo, useState } from "react";
import { recipeApply, recipeFields, recipePreview } from "../ipc";
import {
  RECIPE_DEFS,
  buildRecipe,
  describeRecipe,
  initialParams,
  type ParamValues,
  type RecipeDef,
} from "../lib/recipe-forms";
import { OtherRecipesSection } from "./OtherRecipesSection";
import { TagRecipesSection } from "./TagRecipesSection";
import { useToast } from "./Toast";
import type { Recipe, RecipeProposal } from "../types";

const DELIMITER_OPTIONS = [
  { value: "parentheses", label: "( )" },
  { value: "brackets", label: "[ ]" },
  { value: "braces", label: "{ }" },
  { value: "angles", label: "< >" },
  { value: "double_quotes", label: '" "' },
  { value: "single_quotes", label: "' '" },
];

const CATEGORIES = ["Casing", "Field", "Text", "Number"] as const;

interface Props {
  libraryPath: string;
  trackIds: string[];
}

/**
 * Recipes — parameterized bulk edits, built up and run over a selection.
 *
 * Preview-then-apply throughout, matching Smart Fixes: every proposed change is
 * a deselectable row, and only what survives review gets staged. Nothing here
 * writes to `master.db`; proposals become staged changes and go through Sync.
 */
export function RecipesPanel({ libraryPath, trackIds }: Props) {
  const { toast } = useToast();
  const [fields, setFields] = useState<string[]>([]);
  const [recipes, setRecipes] = useState<Recipe[]>([]);
  const [op, setOp] = useState<RecipeDef["op"]>("to_title_case");
  const [params, setParams] = useState<ParamValues>({});
  const [proposals, setProposals] = useState<RecipeProposal[] | null>(null);
  const [skipped, setSkipped] = useState<[string, string][]>([]);
  const [excluded, setExcluded] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  const def = useMemo(
    () => RECIPE_DEFS.find((d) => d.op === op) ?? RECIPE_DEFS[0],
    [op],
  );

  useEffect(() => {
    let cancelled = false;
    recipeFields()
      .then((f) => {
        if (cancelled) return;
        const list = Array.isArray(f) ? f : [];
        setFields(list);
      })
      .catch(() => {
        if (!cancelled) setFields([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Reset the form whenever the operation changes — carrying a "delimiter"
  // value into an operation that has no delimiter would silently ship junk.
  useEffect(() => {
    setParams(initialParams(def, fields[0] ?? "title"));
  }, [def, fields]);

  const addRecipe = useCallback(() => {
    setRecipes((prev) => [...prev, buildRecipe(op, params)]);
  }, [op, params]);

  const runPreview = useCallback(async () => {
    setBusy(true);
    try {
      const result = await recipePreview(libraryPath, trackIds, recipes);
      setProposals(result.proposals);
      setSkipped(result.skipped);
      setExcluded(new Set());
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, trackIds, recipes, toast]);

  const kept = useMemo(
    () => (proposals ?? []).filter((p) => !excluded.has(p.id)),
    [proposals, excluded],
  );

  const stage = useCallback(async () => {
    if (kept.length === 0) return;
    setBusy(true);
    try {
      const ids = await recipeApply(libraryPath, kept);
      toast({
        variant: "success",
        message: `Staged ${ids.length} change(s) for review.`,
      });
      setProposals(null);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, kept, toast]);

  return (
    <div className="flex h-full flex-col overflow-auto p-4" aria-label="Recipes">
      <header className="mb-3">
        <h2 className="text-sm font-semibold">Recipes</h2>
        <p className="text-xs text-muted">
          Parameterized bulk edits over {trackIds.length} track(s). Changes are
          previewed, then staged for review — nothing is written directly.
        </p>
      </header>

      <section className="mb-3 rounded border border-border p-3" aria-label="Add recipe">
        <div className="mb-2 flex flex-wrap items-end gap-2 text-xs">
          <label>
            <span className="mb-1 block text-muted">Operation</span>
            <select
              aria-label="Operation"
              className="rounded border border-border bg-surface px-2 py-1 text-xs"
              value={op}
              onChange={(e) => setOp(e.target.value as RecipeDef["op"])}
            >
              {CATEGORIES.map((category) => (
                <optgroup key={category} label={category}>
                  {RECIPE_DEFS.filter((d) => d.category === category).map((d) => (
                    <option key={d.op} value={d.op}>
                      {d.label}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>

          {def.params.map((p) => {
            const value = params[p.key];
            if (p.kind === "field") {
              return (
                <label key={p.key}>
                  <span className="mb-1 block text-muted">{p.label}</span>
                  <select
                    aria-label={p.label}
                    className="rounded border border-border bg-surface px-2 py-1 text-xs"
                    value={String(value ?? "")}
                    onChange={(e) =>
                      setParams({ ...params, [p.key]: e.target.value })
                    }
                  >
                    {fields.map((f) => (
                      <option key={f} value={f}>
                        {f}
                      </option>
                    ))}
                  </select>
                </label>
              );
            }
            if (p.kind === "bool") {
              return (
                <label key={p.key} className="flex items-center gap-1">
                  <input
                    type="checkbox"
                    aria-label={p.label}
                    checked={Boolean(value)}
                    onChange={(e) =>
                      setParams({ ...params, [p.key]: e.target.checked })
                    }
                  />
                  {p.label}
                </label>
              );
            }
            if (p.kind === "delimiter" || p.kind === "special-mode") {
              const options =
                p.kind === "delimiter"
                  ? DELIMITER_OPTIONS
                  : [
                      { value: "special", label: "Special characters" },
                      { value: "emojis", label: "Emoji" },
                    ];
              return (
                <label key={p.key}>
                  <span className="mb-1 block text-muted">{p.label}</span>
                  <select
                    aria-label={p.label}
                    className="rounded border border-border bg-surface px-2 py-1 text-xs"
                    value={String(value ?? "")}
                    onChange={(e) =>
                      setParams({ ...params, [p.key]: e.target.value })
                    }
                  >
                    {options.map((o) => (
                      <option key={o.value} value={o.value}>
                        {o.label}
                      </option>
                    ))}
                  </select>
                </label>
              );
            }
            return (
              <label key={p.key}>
                <span className="mb-1 block text-muted">{p.label}</span>
                <input
                  aria-label={p.label}
                  type={p.kind === "number" ? "number" : "text"}
                  className="w-40 rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
                  value={String(value ?? "")}
                  onChange={(e) =>
                    setParams({
                      ...params,
                      [p.key]:
                        p.kind === "number" ? Number(e.target.value) : e.target.value,
                    })
                  }
                />
              </label>
            );
          })}

          <button
            type="button"
            className="rounded border border-border px-3 py-1 text-xs hover:bg-surface-hover"
            onClick={addRecipe}
          >
            Add
          </button>
        </div>
      </section>

      <section className="mb-3" aria-label="Recipe list">
        {recipes.length === 0 ? (
          <p className="text-xs text-muted" data-testid="no-recipes">
            No recipes yet. Add one above — they run in the order listed.
          </p>
        ) : (
          <ol className="mb-2 space-y-0.5 text-xs">
            {recipes.map((r, i) => (
              <li key={i} className="flex items-center gap-2">
                <span className="w-5 text-right font-mono text-muted">{i + 1}</span>
                <span>{describeRecipe(r)}</span>
                <button
                  type="button"
                  aria-label={`Remove step ${i + 1}`}
                  className="ml-auto text-muted hover:text-red-400"
                  onClick={() => setRecipes(recipes.filter((_, j) => j !== i))}
                >
                  Remove
                </button>
              </li>
            ))}
          </ol>
        )}

        <div className="flex gap-2 text-xs">
          <button
            type="button"
            disabled={busy || recipes.length === 0 || trackIds.length === 0}
            className="rounded border border-border px-3 py-1 hover:bg-surface-hover disabled:opacity-50"
            onClick={() => void runPreview()}
          >
            Preview
          </button>
          <button
            type="button"
            disabled={busy || kept.length === 0}
            className="rounded bg-accent px-3 py-1 text-white hover:bg-accent-hover disabled:opacity-50"
            onClick={() => void stage()}
          >
            Stage {kept.length} change(s)
          </button>
        </div>
      </section>

      {proposals != null && (
        <section data-testid="recipe-preview" className="min-h-0 shrink-0 overflow-x-auto">
          {proposals.length === 0 ? (
            <p className="text-xs text-muted">
              Nothing would change on the selected tracks.
            </p>
          ) : (
            <table className="w-full text-left text-xs">
              <thead className="text-muted">
                <tr>
                  <th className="px-2 py-1 font-medium" />
                  <th className="px-2 py-1 font-medium">Track</th>
                  <th className="px-2 py-1 font-medium">Field</th>
                  <th className="px-2 py-1 font-medium">Before</th>
                  <th className="px-2 py-1 font-medium">After</th>
                </tr>
              </thead>
              <tbody>
                {proposals.map((p) => (
                  <tr key={p.id} className="border-t border-border">
                    <td className="px-2 py-1">
                      <input
                        type="checkbox"
                        aria-label={`Keep ${p.track_title} ${p.field}`}
                        checked={!excluded.has(p.id)}
                        onChange={(e) => {
                          const next = new Set(excluded);
                          if (e.target.checked) next.delete(p.id);
                          else next.add(p.id);
                          setExcluded(next);
                        }}
                      />
                    </td>
                    <td className="px-2 py-1">{p.track_title}</td>
                    <td className="px-2 py-1 font-mono text-muted">{p.field}</td>
                    <td className="px-2 py-1 text-muted line-through">
                      {p.before ?? <span className="italic">empty</span>}
                    </td>
                    <td className="px-2 py-1">
                      {p.after ?? <span className="italic text-muted">cleared</span>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {skipped.length > 0 && (
            <p className="mt-2 text-[11px] text-amber-500" data-testid="recipe-skipped">
              {skipped.length} step(s) did nothing: {skipped[0][1]}
              {skipped.length > 1 ? ", …" : ""}
            </p>
          )}
        </section>
      )}
      <TagRecipesSection
        libraryPath={libraryPath}
        trackIds={trackIds}
        fields={fields}
      />

      <OtherRecipesSection libraryPath={libraryPath} trackIds={trackIds} />
    </div>
  );
}
