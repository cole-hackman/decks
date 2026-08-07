import { useCallback, useState } from "react";
import { otherRecipeApply } from "../ipc";
import { useToast } from "./Toast";
import type { OtherRecipe } from "../types";

const OPS: { value: OtherRecipe; label: string; detail: string }[] = [
  {
    value: "mark_as_incoming",
    label: "Mark as Incoming",
    detail: "Puts the tracks back on the Incoming page, to use it as a to-do list.",
  },
  {
    value: "remove_from_all_playlists",
    label: "Remove from all playlists",
    detail:
      "Stages a removal per playlist. Smartlists are untouched — they are derived, and would simply re-add the track.",
  },
  {
    value: "import_date_from_filesystem",
    label: "Import date from filesystem",
    detail:
      "Takes the file's modification time as the release year. Modification, not creation: creation time is not portable and a copied file loses it.",
  },
];

interface Props {
  libraryPath: string;
  trackIds: string[];
}

/**
 * The spec's "Other" recipes.
 *
 * Each reaches into a different subsystem — Incoming state, playlists, the
 * filesystem — so they run one at a time rather than joining the ordered
 * recipe list.
 */
export function OtherRecipesSection({ libraryPath, trackIds }: Props) {
  const { toast } = useToast();
  const [op, setOp] = useState<OtherRecipe>("mark_as_incoming");
  const [busy, setBusy] = useState(false);

  const chosen = OPS.find((o) => o.value === op) ?? OPS[0];

  const run = useCallback(async () => {
    setBusy(true);
    try {
      const result = await otherRecipeApply(libraryPath, trackIds, op);
      const parts = [`${result.changed.length} track(s)`];
      if (result.staged.length > 0) {
        parts.push(`${result.staged.length} change(s) staged for review`);
      }
      if (result.skipped.length > 0) {
        parts.push(`${result.skipped.length} skipped: ${result.skipped[0][1]}`);
      }
      toast({
        variant: result.skipped.length > 0 ? "warn" : "success",
        message: `${parts.join(" — ")}.`,
      });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, trackIds, op, toast]);

  return (
    <section className="border-t border-border px-4 py-3" aria-label="Other recipes">
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Other
      </h3>

      <div className="mb-1 flex flex-wrap items-end gap-2 text-xs">
        <label>
          <span className="mb-1 block text-muted">Other recipe</span>
          <select
            aria-label="Other recipe"
            className="rounded border border-border bg-surface px-2 py-1 text-xs"
            value={op}
            onChange={(e) => setOp(e.target.value as OtherRecipe)}
          >
            {OPS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          disabled={busy || trackIds.length === 0}
          className="rounded bg-accent px-3 py-1 text-white hover:bg-accent-hover disabled:opacity-50"
          onClick={() => void run()}
        >
          Run on {trackIds.length} track(s)
        </button>
      </div>

      <p className="text-[11px] text-muted" data-testid="other-recipe-detail">
        {chosen.detail}
      </p>
    </section>
  );
}
