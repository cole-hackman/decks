import { useCallback, useEffect, useState } from "react";
import { multiEditApply, multiEditForm } from "../ipc";
import { useToast } from "./Toast";
import type { MultiEdit, MultiEditFieldValue } from "../types";

interface Props {
  libraryPath: string;
  trackIds: string[];
  onClose: () => void;
  /** Called after edits are staged, so the change count can refresh. */
  onStaged?: () => void;
}

/** What a field with disagreeing values shows. */
const MULTIPLE = "<multiple values>";

interface FieldState {
  field: string;
  /** The value as loaded — `null` when the selection disagrees. */
  original: string | null;
  /** What the input holds. */
  draft: string;
  /** False until the user types in it. Untouched fields are never written. */
  touched: boolean;
}

function toState(field: string, value: MultiEditFieldValue): FieldState {
  const same = value.kind === "same";
  return {
    field,
    original: same ? (value.value ?? "") : null,
    draft: same ? (value.value ?? "") : "",
    touched: false,
  };
}

/**
 * Manual multi-track editor.
 *
 * The whole feature turns on one rule: **a field the user did not touch is not
 * written.** Open this on forty tracks, change the genre, press Save — and the
 * other nine fields must come out exactly as they went in, even though the form
 * had to show something in each of them.
 *
 * So the state carries a `touched` flag per field, and only touched fields are
 * sent. A field where the selection disagrees shows `<multiple values>` as a
 * placeholder rather than as text, so there is nothing to accidentally save.
 *
 * Per `docs/lexicon/02-library.md §Manual Editing`. Album art is out of scope —
 * `decks` has no album art anywhere yet.
 */
export function MultiTrackEditor({
  libraryPath,
  trackIds,
  onClose,
  onStaged,
}: Props) {
  const { toast } = useToast();
  const [fields, setFields] = useState<FieldState[]>([]);
  const [count, setCount] = useState(0);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    multiEditForm(libraryPath, trackIds)
      .then((form) => {
        if (cancelled) return;
        const list = Array.isArray(form?.fields) ? form.fields : [];
        setFields(list.map(([field, value]) => toState(field, value)));
        setCount(form?.track_count ?? 0);
        setLoading(false);
      })
      .catch((e) => {
        if (cancelled) return;
        toast({ variant: "error", message: String(e) });
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [libraryPath, trackIds, toast]);

  const edits: MultiEdit[] = fields
    .filter((f) => f.touched && f.draft !== (f.original ?? ""))
    .map((f) => ({ field: f.field, value: f.draft === "" ? null : f.draft }));

  const save = useCallback(async () => {
    if (edits.length === 0) {
      onClose();
      return;
    }
    setBusy(true);
    try {
      const ids = await multiEditApply(libraryPath, trackIds, edits);
      toast({
        variant: "success",
        message: `Staged ${ids.length} change(s) for review.`,
      });
      onStaged?.();
      onClose();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, trackIds, edits, onClose, onStaged, toast]);

  return (
    <div
      role="dialog"
      aria-label="Edit tracks"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
        // Enter saves and closes, but not from inside a textarea where the
        // user is plausibly typing a newline.
        if (e.key === "Enter" && !(e.target instanceof HTMLTextAreaElement)) {
          e.preventDefault();
          void save();
        }
      }}
    >
      <div className="max-h-[80vh] w-[32rem] overflow-auto rounded-lg border border-edge bg-base p-4 shadow-xl">
        <header className="mb-3">
          <h2 className="text-sm font-semibold text-ink">
            Edit {count} track{count === 1 ? "" : "s"}
          </h2>
          <p className="text-[11px] text-ink-muted">
            Only fields you change are written. Changes are staged for review —
            nothing goes to the library directly.
          </p>
        </header>

        {loading ? (
          <p className="text-xs text-ink-muted" data-testid="multi-edit-loading">
            Loading…
          </p>
        ) : (
          <div className="space-y-2">
            {fields.map((f, i) => (
              <label key={f.field} className="block text-xs">
                <span className="mb-1 block text-ink-secondary">{f.field}</span>
                <input
                  aria-label={f.field}
                  className="w-full rounded-md border border-edge-strong bg-surface px-2 py-1 text-xs"
                  // A placeholder, not a value: there is nothing here to save
                  // by accident.
                  placeholder={f.original === null ? MULTIPLE : ""}
                  value={f.draft}
                  onChange={(e) => {
                    const next = [...fields];
                    next[i] = { ...f, draft: e.target.value, touched: true };
                    setFields(next);
                  }}
                />
              </label>
            ))}
          </div>
        )}

        <footer className="mt-4 flex items-center gap-2 text-xs">
          <span className="text-ink-muted" data-testid="multi-edit-count">
            {edits.length} field(s) changed
          </span>
          <button
            type="button"
            className="ml-auto rounded-md border border-edge-strong px-3 py-1 hover:bg-elevated"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={busy || loading}
            className="rounded-md bg-accent-strong px-3 py-1 text-white hover:bg-accent disabled:opacity-50"
            onClick={() => void save()}
          >
            Save
          </button>
        </footer>
      </div>
    </div>
  );
}
