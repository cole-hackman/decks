import { useCallback, useEffect, useMemo, useState } from "react";
import {
  createSmartlist,
  deleteSmartlist,
  evaluateSmartlist,
  generateSmartlists,
  listSmartlists,
  previewSmartlist,
  smartlistCompatibility,
  smartlistCounts,
  updateSmartlist,
} from "../ipc";
import {
  ALL_FIELDS,
  FIELD_KINDS,
  FIELD_LABELS,
  OPERATOR_LABELS,
  coerceRule,
  describeRule,
  emptyClause,
  operatorsFor,
  takesOperand,
} from "../lib/smartlist-fields";
import { useToast } from "./Toast";
import type {
  Smartlist,
  SmartlistClause,
  SmartlistCombinator,
  SmartlistCompatibility,
  SmartlistField,
  SmartlistGeneratorSpec,
  SmartlistOperator,
  SmartlistValue,
  Track,
} from "../types";

interface Props {
  libraryPath: string;
  onOpenInspector?: (track: Track) => void;
}

function isNative(c: SmartlistCompatibility | undefined): boolean {
  return c === "native" || (typeof c === "object" && "native" in c);
}

function compatReason(c: SmartlistCompatibility | undefined): string | null {
  if (c && typeof c === "object" && "materialised" in c) {
    return c.materialised.reason;
  }
  return null;
}

/** One rule row inside a clause. */
function RuleRow({
  rule,
  onChange,
  onRemove,
  canRemove,
}: {
  rule: { field: SmartlistField; op: SmartlistOperator; value: SmartlistValue };
  onChange: (next: {
    field: SmartlistField;
    op: SmartlistOperator;
    value: SmartlistValue;
  }) => void;
  onRemove: () => void;
  canRemove: boolean;
}) {
  const ops = operatorsFor(rule.field);
  const kind = FIELD_KINDS[rule.field];

  return (
    <div className="flex flex-wrap items-center gap-2">
      <select
        aria-label="Field"
        className="rounded border border-border bg-surface px-2 py-1 text-sm"
        value={rule.field}
        onChange={(e) =>
          onChange(coerceRule(e.target.value as SmartlistField, rule.op))
        }
      >
        {ALL_FIELDS.map((f) => (
          <option key={f} value={f}>
            {FIELD_LABELS[f]}
          </option>
        ))}
      </select>

      <select
        aria-label="Operator"
        className="rounded border border-border bg-surface px-2 py-1 text-sm"
        value={rule.op}
        onChange={(e) => {
          const next = coerceRule(rule.field, e.target.value as SmartlistOperator);
          onChange(next);
        }}
      >
        {ops.map((o) => (
          <option key={o} value={o}>
            {OPERATOR_LABELS[o]}
          </option>
        ))}
      </select>

      {takesOperand(rule.op) && rule.value.type === "range" && (
        <>
          <input
            aria-label="From"
            type="number"
            className="w-24 rounded border border-border bg-surface px-2 py-1 text-sm"
            value={rule.value.value[0]}
            onChange={(e) =>
              onChange({
                ...rule,
                value: {
                  type: "range",
                  value: [
                    Number(e.target.value),
                    (rule.value as { value: [number, number] }).value[1],
                  ],
                },
              })
            }
          />
          <span className="text-xs text-muted">and</span>
          <input
            aria-label="To"
            type="number"
            className="w-24 rounded border border-border bg-surface px-2 py-1 text-sm"
            value={rule.value.value[1]}
            onChange={(e) =>
              onChange({
                ...rule,
                value: {
                  type: "range",
                  value: [
                    (rule.value as { value: [number, number] }).value[0],
                    Number(e.target.value),
                  ],
                },
              })
            }
          />
        </>
      )}

      {takesOperand(rule.op) && rule.value.type === "number" && (
        <input
          aria-label="Value"
          type="number"
          className="w-28 rounded border border-border bg-surface px-2 py-1 text-sm"
          value={rule.value.value}
          onChange={(e) =>
            onChange({
              ...rule,
              value: { type: "number", value: Number(e.target.value) },
            })
          }
        />
      )}

      {takesOperand(rule.op) && rule.value.type === "text" && (
        <input
          aria-label="Value"
          type={kind === "date" ? "date" : "text"}
          placeholder={
            kind === "key" ? "8A, Am, 8m…" : kind === "date" ? "YYYY-MM-DD" : "value"
          }
          className="w-48 rounded border border-border bg-surface px-2 py-1 text-sm"
          value={rule.value.value}
          onChange={(e) =>
            onChange({ ...rule, value: { type: "text", value: e.target.value } })
          }
        />
      )}

      {takesOperand(rule.op) && rule.value.type === "text_range" && (
        <>
          <input
            aria-label="From"
            type="date"
            className="w-40 rounded border border-border bg-surface px-2 py-1 text-sm"
            value={rule.value.value[0]}
            onChange={(e) =>
              onChange({
                ...rule,
                value: {
                  type: "text_range",
                  value: [
                    e.target.value,
                    (rule.value as { value: [string, string] }).value[1],
                  ],
                },
              })
            }
          />
          <span className="text-xs text-muted">and</span>
          <input
            aria-label="To"
            type="date"
            className="w-40 rounded border border-border bg-surface px-2 py-1 text-sm"
            value={rule.value.value[1]}
            onChange={(e) =>
              onChange({
                ...rule,
                value: {
                  type: "text_range",
                  value: [
                    (rule.value as { value: [string, string] }).value[0],
                    e.target.value,
                  ],
                },
              })
            }
          />
        </>
      )}

      {takesOperand(rule.op) && rule.value.type === "tags" && (
        <input
          aria-label="Tag IDs"
          type="text"
          placeholder="tag ids, comma separated"
          className="w-64 rounded border border-border bg-surface px-2 py-1 text-sm"
          value={rule.value.value.join(",")}
          onChange={(e) =>
            onChange({
              ...rule,
              value: {
                type: "tags",
                value: e.target.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean),
              },
            })
          }
        />
      )}

      {canRemove && (
        <button
          type="button"
          className="text-xs text-muted hover:text-fg"
          onClick={onRemove}
          aria-label="Remove rule"
        >
          Remove
        </button>
      )}
    </div>
  );
}

export function SmartlistsView({ libraryPath, onOpenInspector }: Props) {
  const { toast } = useToast();
  const [lists, setLists] = useState<Smartlist[]>([]);
  const [counts, setCounts] = useState<Record<string, number>>({});
  const [compat, setCompat] = useState<Record<string, SmartlistCompatibility>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Editor state. `editingId` null means "creating a new one".
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [combinator, setCombinator] = useState<SmartlistCombinator>("all");
  const [clauses, setClauses] = useState<SmartlistClause[]>([emptyClause()]);
  const [previewCount, setPreviewCount] = useState<number | null>(null);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tracks, setTracks] = useState<Track[]>([]);

  const refresh = useCallback(async () => {
    if (!libraryPath) return;
    setLoading(true);
    setError(null);
    try {
      const rows = await listSmartlists(libraryPath);
      setLists(rows);
      const [c, k] = await Promise.all([
        smartlistCounts(libraryPath).catch(() => ({}) as Record<string, number>),
        smartlistCompatibility(libraryPath).catch(
          () => ({}) as Record<string, SmartlistCompatibility>,
        ),
      ]);
      setCounts(c);
      setCompat(k);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [libraryPath]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Live match count for the rule set being edited.
  useEffect(() => {
    if (!editorOpen || !libraryPath) return;
    let cancelled = false;
    previewSmartlist(libraryPath, combinator, clauses)
      .then((t) => {
        if (!cancelled) setPreviewCount(t.length);
      })
      .catch(() => {
        if (!cancelled) setPreviewCount(null);
      });
    return () => {
      cancelled = true;
    };
  }, [editorOpen, libraryPath, combinator, clauses]);

  const openNew = () => {
    setEditingId(null);
    setName("");
    setCombinator("all");
    setClauses([emptyClause()]);
    setPreviewCount(null);
    setEditorOpen(true);
  };

  const openEdit = (list: Smartlist) => {
    setEditingId(list.id);
    setName(list.name);
    setCombinator(list.combinator);
    setClauses(list.clauses.length > 0 ? list.clauses : [emptyClause()]);
    setPreviewCount(null);
    setEditorOpen(true);
  };

  const handleSave = async () => {
    try {
      if (editingId) {
        await updateSmartlist(libraryPath, editingId, name, combinator, clauses);
        toast({ variant: "success", message: `Updated "${name}".` });
      } else {
        await createSmartlist(libraryPath, name, combinator, clauses);
        toast({ variant: "success", message: `Created "${name}".` });
      }
      setEditorOpen(false);
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  };

  const handleDelete = async (list: Smartlist) => {
    try {
      await deleteSmartlist(libraryPath, list.id);
      if (selectedId === list.id) {
        setSelectedId(null);
        setTracks([]);
      }
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  };

  const handleShow = async (list: Smartlist) => {
    try {
      const rows = await evaluateSmartlist(libraryPath, list.id);
      setSelectedId(list.id);
      setTracks(rows);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  };

  const handleGenerate = async (spec: SmartlistGeneratorSpec) => {
    try {
      const created = await generateSmartlists(libraryPath, spec);
      toast({
        variant: "success",
        message:
          created.length === 0
            ? "Nothing new to generate — everything already exists."
            : `Generated ${created.length} smartlist(s).`,
      });
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  };

  // OR grouping only exists in "all" mode — in "any" mode every rule is already
  // unioned, so a clause layer would be meaningless. This mirrors Lexicon.
  const orGroupingEnabled = combinator === "all";

  const summary = useMemo(
    () =>
      clauses
        .map((c) => c.rules.map((r) => describeRule(r.field, r.op, r.value)).join(" OR "))
        .filter(Boolean),
    [clauses],
  );

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold">Smartlists</h2>
          <p className="text-xs text-muted">
            Rules-driven playlists that update themselves as your library changes.
          </p>
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            className="rounded bg-accent px-3 py-1.5 text-sm text-white hover:bg-accent-hover"
            onClick={openNew}
          >
            New smartlist
          </button>
        </div>
      </header>

      {error && (
        <div className="border-b border-border bg-red-500/10 px-4 py-2 text-sm text-red-400">
          {error}
        </div>
      )}

      <div className="flex flex-1 overflow-hidden">
        <div className="w-1/2 overflow-auto border-r border-border">
          <div className="border-b border-border px-4 py-2">
            <p className="mb-2 text-xs font-medium uppercase tracking-wide text-muted">
              Generator
            </p>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                className="rounded border border-border px-2 py-1 text-xs hover:bg-surface-hover"
                onClick={() => handleGenerate({ kind: "by_field", field: "genre" })}
              >
                By genre
              </button>
              <button
                type="button"
                className="rounded border border-border px-2 py-1 text-xs hover:bg-surface-hover"
                onClick={() => handleGenerate({ kind: "by_decade" })}
              >
                By decade
              </button>
              <button
                type="button"
                className="rounded border border-border px-2 py-1 text-xs hover:bg-surface-hover"
                onClick={() => handleGenerate({ kind: "by_bpm_range", width: 10 })}
              >
                By BPM range
              </button>
              <button
                type="button"
                className="rounded border border-border px-2 py-1 text-xs hover:bg-surface-hover"
                onClick={() => handleGenerate({ kind: "by_play_count", threshold: 10 })}
              >
                By play count
              </button>
            </div>
            <p className="mt-2 text-[11px] text-muted">
              Generated smartlists land in the <code>Lexicon</code> folder. Re-running is
              safe — it only creates what is missing.
            </p>
          </div>

          {loading && <p className="px-4 py-3 text-sm text-muted">Loading…</p>}
          {!loading && lists.length === 0 && (
            <div className="px-4 py-8 text-center">
              <p className="text-sm text-muted">No smartlists yet.</p>
              <button
                type="button"
                className="mt-2 text-sm text-accent hover:underline"
                onClick={openNew}
              >
                Create your first one
              </button>
            </div>
          )}

          <ul>
            {lists.map((l) => {
              const reason = compatReason(compat[l.id]);
              return (
                <li
                  key={l.id}
                  className={`border-b border-border px-4 py-3 ${
                    selectedId === l.id ? "bg-surface-hover" : ""
                  }`}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium">{l.name}</p>
                      <p className="text-xs text-muted">
                        {counts[l.id] ?? 0} tracks ·{" "}
                        {l.combinator === "all" ? "All rules" : "Any rule"}
                        {l.parent_folder_id === "Lexicon" && " · generated"}
                      </p>
                    </div>
                    <div className="flex shrink-0 gap-2 text-xs">
                      <button
                        type="button"
                        className="text-accent hover:underline"
                        onClick={() => handleShow(l)}
                      >
                        Show
                      </button>
                      <button
                        type="button"
                        className="text-muted hover:text-fg"
                        onClick={() => openEdit(l)}
                      >
                        Edit
                      </button>
                      <button
                        type="button"
                        className="text-muted hover:text-red-400"
                        onClick={() => handleDelete(l)}
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                  <p
                    className={`mt-1 text-[11px] ${
                      isNative(compat[l.id]) ? "text-emerald-500" : "text-amber-500"
                    }`}
                    title={reason ?? undefined}
                  >
                    {isNative(compat[l.id])
                      ? "Rekordbox: native MyTag smartlist"
                      : `Rekordbox: flattened to a playlist${reason ? ` — ${reason}` : ""}`}
                  </p>
                </li>
              );
            })}
          </ul>
        </div>

        <div className="w-1/2 overflow-auto">
          {editorOpen ? (
            <div className="p-4">
              <h3 className="mb-3 text-sm font-semibold">
                {editingId ? "Edit smartlist" : "New smartlist"}
              </h3>

              <input
                aria-label="Name"
                type="text"
                placeholder="Name"
                className="mb-3 w-full rounded border border-border bg-surface px-2 py-1.5 text-sm"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />

              <div className="mb-3 flex items-center gap-3 text-sm">
                <label className="flex items-center gap-1">
                  <input
                    type="radio"
                    name="combinator"
                    checked={combinator === "all"}
                    onChange={() => setCombinator("all")}
                  />
                  All rules
                </label>
                <label className="flex items-center gap-1">
                  <input
                    type="radio"
                    name="combinator"
                    checked={combinator === "any"}
                    onChange={() => setCombinator("any")}
                  />
                  Any rule
                </label>
              </div>

              {clauses.map((clause, ci) => (
                <div
                  key={ci}
                  className="mb-3 rounded border border-border p-2"
                  data-testid="clause"
                >
                  {clause.rules.map((rule, ri) => (
                    <div key={ri} className="mb-2 last:mb-0">
                      <RuleRow
                        rule={rule}
                        canRemove={clause.rules.length > 1 || clauses.length > 1}
                        onChange={(next) => {
                          const copy = structuredClone(clauses);
                          copy[ci].rules[ri] = next;
                          setClauses(copy);
                        }}
                        onRemove={() => {
                          const copy = structuredClone(clauses);
                          copy[ci].rules.splice(ri, 1);
                          if (copy[ci].rules.length === 0) copy.splice(ci, 1);
                          setClauses(copy.length > 0 ? copy : [emptyClause()]);
                        }}
                      />
                      {ri < clause.rules.length - 1 && (
                        <p className="mt-1 text-[11px] font-medium text-muted">OR</p>
                      )}
                    </div>
                  ))}
                  {orGroupingEnabled && (
                    <button
                      type="button"
                      className="mt-1 text-xs text-accent hover:underline"
                      onClick={() => {
                        const copy = structuredClone(clauses);
                        copy[ci].rules.push(emptyClause().rules[0]);
                        setClauses(copy);
                      }}
                    >
                      + OR condition
                    </button>
                  )}
                </div>
              ))}

              <div className="mb-3 flex items-center gap-3">
                <button
                  type="button"
                  className="text-xs text-accent hover:underline"
                  onClick={() => setClauses([...clauses, emptyClause()])}
                >
                  + Add rule
                </button>
                {!orGroupingEnabled && (
                  <span className="text-[11px] text-muted">
                    OR grouping is only available in “All rules” mode.
                  </span>
                )}
              </div>

              <p className="mb-3 text-xs text-muted" data-testid="preview-count">
                {previewCount === null
                  ? "Matching…"
                  : `${previewCount} track(s) match`}
              </p>

              {summary.length > 0 && (
                <ul className="mb-3 space-y-0.5 text-[11px] text-muted">
                  {summary.map((s, i) => (
                    <li key={i}>
                      {i > 0 && combinator === "all" ? "AND " : ""}
                      {s}
                    </li>
                  ))}
                </ul>
              )}

              <div className="flex gap-2">
                <button
                  type="button"
                  className="rounded bg-accent px-3 py-1.5 text-sm text-white hover:bg-accent-hover disabled:opacity-50"
                  disabled={name.trim() === ""}
                  onClick={handleSave}
                >
                  Save
                </button>
                <button
                  type="button"
                  className="rounded border border-border px-3 py-1.5 text-sm hover:bg-surface-hover"
                  onClick={() => setEditorOpen(false)}
                >
                  Cancel
                </button>
              </div>
            </div>
          ) : (
            <div className="p-4">
              <h3 className="mb-2 text-sm font-semibold">
                {selectedId
                  ? `${tracks.length} matching track(s)`
                  : "Select a smartlist"}
              </h3>
              <ul>
                {tracks.map((t) => (
                  <li
                    key={t.id}
                    className="cursor-pointer border-b border-border py-1.5 text-sm hover:bg-surface-hover"
                    onClick={() => onOpenInspector?.(t)}
                  >
                    <span className="font-medium">{t.title}</span>
                    <span className="text-muted"> — {t.artist ?? "Unknown"}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
