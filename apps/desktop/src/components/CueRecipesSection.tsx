import { useCallback, useMemo, useState } from "react";
import { cueRecipeApply, cueRecipePreview } from "../ipc";
import { useToast } from "./Toast";
import type {
  CueColourScheme,
  CueDeleteMode,
  CueMirrorTarget,
  CueRecipe,
  CueRecipeTrack,
  CueSortOrder,
} from "../types";

type Op = CueRecipe["op"];

const OPS: { value: Op; label: string }[] = [
  { value: "delete_cues", label: "Delete cues" },
  { value: "change_colours", label: "Change colours" },
  { value: "find_and_replace", label: "Find and replace" },
  { value: "sort_cues", label: "Sort cues" },
  { value: "replace_cue_text", label: "Replace cue text" },
  { value: "remove_cue_text", label: "Remove cue text" },
  { value: "remove_cues_by_label", label: "Remove cues by label" },
  { value: "shift_cues", label: "Shift cues" },
  { value: "quantize_cues", label: "Quantize cues" },
  { value: "mirror_cues", label: "Copy between hot and memory cues" },
];

/**
 * Per `docs/lexicon/01-interop.md §Cue Destination` — the sync options
 * "All to hot cue / All to memory cue / All to hot and memory cue".
 *
 * `both` is the one people actually want: hot cues do not show on every
 * player, memory cues do.
 */
const MIRROR_TARGETS: { value: CueMirrorTarget; label: string }[] = [
  { value: "both", label: "Both — keep each cue as hot and memory" },
  { value: "hot", label: "All to hot cues" },
  { value: "memory", label: "All to memory cues" },
];

const DELETE_MODES: { value: CueDeleteMode; label: string }[] = [
  { value: "all", label: "All cues" },
  { value: "first", label: "First cue only" },
  { value: "last", label: "Last cue only" },
  { value: "keep_first", label: "All but the first" },
  { value: "keep_last", label: "All but the last" },
  { value: "loops_only", label: "Loops only" },
  { value: "without_colour", label: "Cues without a colour" },
  { value: "without_text", label: "Cues without a name" },
  { value: "memory_cues", label: "Memory cues" },
];

const SCHEMES: { value: CueColourScheme; label: string }[] = [
  { value: "basic", label: "Basic" },
  { value: "grayscale", label: "Grayscale" },
  { value: "cold", label: "Cold" },
  { value: "warm", label: "Warm" },
  { value: "cycle", label: "Cycle (never repeats)" },
  { value: "none", label: "Remove all colours" },
  { value: "first_cue_colour", label: "First cue's colour" },
];

const SORT_ORDERS: { value: CueSortOrder; label: string }[] = [
  { value: "time_asc", label: "Time, earliest first" },
  { value: "time_desc", label: "Time, latest first" },
  { value: "label_asc", label: "Name, A–Z" },
  { value: "label_desc", label: "Name, Z–A" },
  { value: "empty_labels_first", label: "Unnamed cues first" },
  { value: "empty_labels_last", label: "Unnamed cues last" },
  { value: "cues_before_loops", label: "Cues before loops" },
  { value: "loops_before_cues", label: "Loops before cues" },
];

/** The resolutions Rekordbox's own quantize offers. */
const RESOLUTIONS = [1, 2, 4, 16, 64];

interface Props {
  libraryPath: string;
  trackIds: string[];
}

/**
 * Cue recipes.
 *
 * These edit a track's cue list rather than its fields, so the preview is a
 * per-track summary rather than a field diff — one recipe can move, rename,
 * recolour and delete in a single pass, and a row-per-field table would bury
 * that. Everything still stages; nothing writes to `master.db`.
 */
export function CueRecipesSection({ libraryPath, trackIds }: Props) {
  const { toast } = useToast();
  const [op, setOp] = useState<Op>("delete_cues");
  const [mode, setMode] = useState<CueDeleteMode>("without_text");
  const [scheme, setScheme] = useState<CueColourScheme>("basic");
  const [order, setOrder] = useState<CueSortOrder>("time_asc");
  const [matchText, setMatchText] = useState("");
  const [newText, setNewText] = useState("");
  const [find, setFind] = useState("");
  const [replace, setReplace] = useState("");
  const [caseInsensitive, setCaseInsensitive] = useState(false);
  const [label, setLabel] = useState("");
  const [offsetMs, setOffsetMs] = useState(0);
  const [resolution, setResolution] = useState(4);
  const [mirrorTarget, setMirrorTarget] = useState<CueMirrorTarget>("both");
  const [preview, setPreview] = useState<CueRecipeTrack[] | null>(null);
  const [busy, setBusy] = useState(false);

  const recipe = useMemo((): CueRecipe => {
    switch (op) {
      case "delete_cues":
        return { op, mode };
      case "change_colours":
        return { op, scheme };
      case "find_and_replace":
        return {
          op,
          match_text: matchText,
          match_colour: null,
          new_text: newText,
          new_colour: null,
        };
      case "sort_cues":
        return { op, order };
      case "replace_cue_text":
        return { op, find, replace, case_insensitive: caseInsensitive };
      case "remove_cue_text":
        return { op };
      case "remove_cues_by_label":
        return { op, text: label };
      case "shift_cues":
        return { op, offset_ms: offsetMs };
      case "quantize_cues":
        return { op, resolution_beats: resolution };
      case "mirror_cues":
        return { op, target: mirrorTarget };
    }
  }, [
    op,
    mode,
    scheme,
    order,
    matchText,
    newText,
    find,
    replace,
    caseInsensitive,
    label,
    offsetMs,
    resolution,
    mirrorTarget,
  ]);

  const runPreview = useCallback(async () => {
    setBusy(true);
    try {
      setPreview(await cueRecipePreview(libraryPath, trackIds, recipe));
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, trackIds, recipe, toast]);

  // Tracks the recipe refused to run on carry nothing to stage, so they are
  // shown but not counted towards the Stage button.
  const actionable = useMemo(
    () =>
      (preview ?? []).filter(
        (t) =>
          t.edits.length > 0 ||
          t.deletions.length > 0 ||
          (t.additions?.length ?? 0) > 0,
      ),
    [preview],
  );

  const stage = useCallback(async () => {
    if (actionable.length === 0) return;
    setBusy(true);
    try {
      const ids = await cueRecipeApply(libraryPath, actionable);
      toast({
        variant: "success",
        message: `Staged ${ids.length} cue change(s) for review.`,
      });
      setPreview(null);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, actionable, toast]);

  // `shrink-0` on the section: this is the fourth panel in a flex column, and
  // without it the preview collapses to zero height once the panels above it
  // fill the view.
  return (
    <section
      className="shrink-0 border-t border-border px-4 py-3"
      aria-label="Cue recipes"
    >
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">
        Cue Recipes
      </h3>
      <p className="mb-2 text-[11px] text-muted">
        Bulk edits to hot cues, memory cues and loops. Sorting reassigns hot-cue
        slots 1–8 in the new order; anything past the eighth keeps its slot.
      </p>

      <div className="mb-2 flex flex-wrap items-end gap-2 text-xs">
        <label>
          <span className="mb-1 block text-muted">Cue operation</span>
          <select
            aria-label="Cue operation"
            className="rounded border border-border bg-surface px-2 py-1 text-xs"
            value={op}
            onChange={(e) => {
              setOp(e.target.value as Op);
              setPreview(null);
            }}
          >
            {OPS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>

        {op === "delete_cues" && (
          <label>
            <span className="mb-1 block text-muted">Which cues</span>
            <select
              aria-label="Which cues"
              className="rounded border border-border bg-surface px-2 py-1 text-xs"
              value={mode}
              onChange={(e) => setMode(e.target.value as CueDeleteMode)}
            >
              {DELETE_MODES.map((m) => (
                <option key={m.value} value={m.value}>
                  {m.label}
                </option>
              ))}
            </select>
          </label>
        )}

        {op === "change_colours" && (
          <label>
            <span className="mb-1 block text-muted">Colour scheme</span>
            <select
              aria-label="Colour scheme"
              className="rounded border border-border bg-surface px-2 py-1 text-xs"
              value={scheme}
              onChange={(e) => setScheme(e.target.value as CueColourScheme)}
            >
              {SCHEMES.map((s) => (
                <option key={s.value} value={s.value}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
        )}

        {op === "sort_cues" && (
          <label>
            <span className="mb-1 block text-muted">Order</span>
            <select
              aria-label="Cue order"
              className="rounded border border-border bg-surface px-2 py-1 text-xs"
              value={order}
              onChange={(e) => setOrder(e.target.value as CueSortOrder)}
            >
              {SORT_ORDERS.map((s) => (
                <option key={s.value} value={s.value}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
        )}

        {op === "find_and_replace" && (
          <>
            <label>
              <span className="mb-1 block text-muted">Match name</span>
              <input
                aria-label="Match cue name"
                className="w-32 rounded border border-border bg-surface px-2 py-1 text-xs"
                placeholder="* for any"
                value={matchText}
                onChange={(e) => setMatchText(e.target.value)}
              />
            </label>
            <label>
              <span className="mb-1 block text-muted">New name</span>
              <input
                aria-label="New cue name"
                className="w-32 rounded border border-border bg-surface px-2 py-1 text-xs"
                value={newText}
                onChange={(e) => setNewText(e.target.value)}
              />
            </label>
            <p className="w-full text-[11px] text-muted">
              An empty match name matches unnamed cues; <code>*</code> matches
              any name. An empty new name keeps the existing one.
            </p>
          </>
        )}

        {op === "replace_cue_text" && (
          <>
            <label>
              <span className="mb-1 block text-muted">Find</span>
              <input
                aria-label="Find in cue name"
                className="w-32 rounded border border-border bg-surface px-2 py-1 text-xs"
                value={find}
                onChange={(e) => setFind(e.target.value)}
              />
            </label>
            <label>
              <span className="mb-1 block text-muted">Replace with</span>
              <input
                aria-label="Replace in cue name"
                className="w-32 rounded border border-border bg-surface px-2 py-1 text-xs"
                value={replace}
                onChange={(e) => setReplace(e.target.value)}
              />
            </label>
            <label className="flex items-center gap-1">
              <input
                type="checkbox"
                aria-label="Ignore case in cue names"
                checked={caseInsensitive}
                onChange={(e) => setCaseInsensitive(e.target.checked)}
              />
              Ignore case
            </label>
          </>
        )}

        {op === "remove_cues_by_label" && (
          <label>
            <span className="mb-1 block text-muted">Name contains</span>
            <input
              aria-label="Cue label to remove"
              className="w-40 rounded border border-border bg-surface px-2 py-1 text-xs"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
            />
          </label>
        )}

        {op === "shift_cues" && (
          <label>
            <span className="mb-1 block text-muted">Offset (ms)</span>
            <input
              aria-label="Shift offset"
              type="number"
              className="w-24 rounded border border-border bg-surface px-2 py-1 font-mono text-xs"
              value={offsetMs}
              onChange={(e) => setOffsetMs(Number(e.target.value))}
            />
          </label>
        )}

        {op === "quantize_cues" && (
          <label>
            <span className="mb-1 block text-muted">Snap to</span>
            <select
              aria-label="Quantize resolution"
              className="rounded border border-border bg-surface px-2 py-1 text-xs"
              value={resolution}
              onChange={(e) => setResolution(Number(e.target.value))}
            >
              {RESOLUTIONS.map((r) => (
                <option key={r} value={r}>
                  {r === 1 ? "1 beat" : `${r} beats`}
                </option>
              ))}
            </select>
          </label>
        )}

        {op === "mirror_cues" && (
          <label>
            <span className="mb-1 block text-muted">Cues should exist as</span>
            <select
              aria-label="Cue destination"
              className="rounded border border-border bg-surface px-2 py-1 text-xs"
              value={mirrorTarget}
              onChange={(e) =>
                setMirrorTarget(e.target.value as CueMirrorTarget)
              }
            >
              {MIRROR_TARGETS.map((t) => (
                <option key={t.value} value={t.value}>
                  {t.label}
                </option>
              ))}
            </select>
          </label>
        )}

        <button
          type="button"
          disabled={busy || trackIds.length === 0}
          className="rounded border border-border px-3 py-1 hover:bg-surface-hover disabled:opacity-50"
          onClick={() => void runPreview()}
        >
          Preview cues
        </button>
        <button
          type="button"
          disabled={busy || actionable.length === 0}
          className="rounded bg-accent px-3 py-1 text-white hover:bg-accent-hover disabled:opacity-50"
          onClick={() => void stage()}
        >
          Stage {actionable.length} track(s)
        </button>
      </div>

      {preview != null && (
        <div data-testid="cue-recipe-preview">
          {preview.length === 0 ? (
            <p className="text-xs text-muted">
              No cue changes — nothing to do on the selected tracks.
            </p>
          ) : (
            <ul className="max-h-48 space-y-0.5 overflow-auto text-xs">
              {preview.map((t) => (
                <li key={t.track_id} className="flex flex-wrap items-center gap-2">
                  <span className="truncate">{t.track_title}</span>
                  {t.skipped ? (
                    <span className="text-amber-500">{t.skipped}</span>
                  ) : (
                    <>
                      {t.edits.length > 0 && (
                        <span className="rounded bg-sky-500/15 px-1 text-sky-400">
                          {t.edits.length} edit(s)
                        </span>
                      )}
                      {(t.additions?.length ?? 0) > 0 && (
                        <span className="rounded bg-emerald-500/15 px-1 text-emerald-400">
                          +{t.additions!.length} cue(s)
                        </span>
                      )}
                      {t.deletions.length > 0 && (
                        <span className="rounded bg-red-500/15 px-1 text-red-400">
                          −{t.deletions.length} cue(s)
                        </span>
                      )}
                    </>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
