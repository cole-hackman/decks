import { useEffect, useMemo, useRef, useState } from "react";
import type { StagedChange, ChangeStatus } from "../agent/types";
import { useStagedChanges } from "../hooks/useStagedChanges";
import { useLibrary } from "../hooks/useLibrary";
import { useToast } from "./Toast";
import { ErrorPanel } from "./ErrorPanel";
import { UndoHistoryPanel } from "./UndoHistoryPanel";

interface Props {
  libraryPath: string;
  /** When provided, the panel renders as a right-side drawer with a close button.
   *  When omitted, it renders inline as a primary workspace view. */
  onClose?: () => void;
}

const STATUS_LABELS: Record<StagedChange["status"], string> = {
  Proposed: "Proposed",
  Accepted: "Accepted",
  Rejected: "Rejected",
  Exported: "Exported",
};

export function DiffReviewPanel({ libraryPath, onClose }: Props) {
  const {
    data: changes = [],
    isLoading,
    error,
    refetch,
    acceptChange,
    rejectChange,
    acceptAllSafe,
    rejectAll,
    exportAcceptedChanges,
    exportResult,
    isMutating,
  } = useStagedChanges(libraryPath);
  const { toast } = useToast();
  const lastExportRef = useRef<string | null>(null);

  useEffect(() => {
    if (!exportResult) return;
    // Avoid re-firing the toast on identical re-renders.
    const key = `${exportResult.output_path}:${exportResult.exported_count}`;
    if (lastExportRef.current === key) return;
    lastExportRef.current = key;
    toast({
      variant: "success",
      message: `Exported ${exportResult.exported_count} ${exportResult.exported_count === 1 ? "change" : "changes"}`,
      detail: exportResult.output_path,
      duration: 6000,
    });
  }, [exportResult, toast]);
  const { data: tracks = [] } = useLibrary(libraryPath);
  const trackById = useMemo(() => {
    const m = new Map<string, { title?: string | null; artist?: string | null }>();
    for (const t of tracks) m.set(t.id, t);
    return m;
  }, [tracks]);
  const counts = countByStatus(changes);
  const proposedCount = counts.Proposed ?? 0;
  const acceptedCount = counts.Accepted ?? 0;

  const [filterStatus, setFilterStatus] = useState<ChangeStatus | "All">("Proposed");

  const filteredChanges = useMemo(() => {
    return changes.filter(c => filterStatus === "All" || c.status === filterStatus);
  }, [changes, filterStatus]);

  const { groups, noTarget } = useMemo(() => {
    const groups: Record<string, StagedChange[]> = {};
    const noTarget: StagedChange[] = [];
    
    for (const change of filteredChanges) {
      if (!change.target_id) {
        noTarget.push(change);
      } else {
        if (!groups[change.target_id]) groups[change.target_id] = [];
        groups[change.target_id].push(change);
      }
    }
    return { groups, noTarget };
  }, [filteredChanges]);

  return (
    <aside
      className={
        onClose
          ? "flex h-full w-[28rem] shrink-0 flex-col border-l border-edge bg-base"
          : "flex h-full flex-1 flex-col bg-base"
      }
    >
      <div className="flex shrink-0 items-center justify-between border-b border-edge px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold text-ink">Review changes</h2>
          <div className="mt-1 flex gap-2 text-[11px] text-ink-muted">
            {Object.entries(STATUS_LABELS).map(([status, label]) => (
              <button
                key={status}
                onClick={() => setFilterStatus(filterStatus === status ? "All" : status as ChangeStatus)}
                className={`transition-colors hover:text-ink-secondary ${
                  filterStatus === status ? "text-accent-hover font-medium" : ""
                }`}
              >
                {label}: {counts[status as ChangeStatus] ?? 0}
              </button>
            ))}
          </div>
        </div>
        {onClose && (
          <button
            onClick={onClose}
            aria-label="Close change review"
            className="rounded p-1 text-ink-muted transition-colors hover:text-ink-secondary"
          >
            <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
              <path d="M3.22 3.22a.75.75 0 011.06 0L8 6.94l3.72-3.72a.75.75 0 111.06 1.06L9.06 8l3.72 3.72a.75.75 0 11-1.06 1.06L8 9.06l-3.72 3.72a.75.75 0 01-1.06-1.06L6.94 8 3.22 4.28a.75.75 0 010-1.06z" />
            </svg>
          </button>
        )}
      </div>

      <div className="flex gap-2 border-b border-edge px-4 py-2">
        <button
          onClick={() => acceptAllSafe()}
          disabled={isMutating || proposedCount === 0}
          className="rounded-md border border-edge-strong px-2 py-1 text-xs text-ink-secondary transition-colors hover:border-emerald-600 hover:text-emerald-300 disabled:cursor-not-allowed disabled:opacity-40"
        >
          Accept safe
        </button>
        <button
          onClick={() => rejectAll()}
          disabled={isMutating || proposedCount === 0}
          className="rounded-md border border-edge-strong px-2 py-1 text-xs text-ink-secondary transition-colors hover:border-red-700 hover:text-red-300 disabled:cursor-not-allowed disabled:opacity-40"
        >
          Reject proposed
        </button>
        <button
          onClick={() => exportAcceptedChanges()}
          disabled={isMutating || acceptedCount === 0}
          className="ml-auto rounded-md bg-accent-strong px-2 py-1 text-xs text-white transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-40"
        >
          Export XML
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-3">
        {exportResult && (
          <div className="mb-3 rounded-md border border-emerald-900 bg-emerald-950/40 px-3 py-2 text-xs text-emerald-300">
            Exported {exportResult.exported_count} changes to{" "}
            {exportResult.output_path}
          </div>
        )}

        {isLoading && (
          <div className="flex h-32 items-center justify-center">
            <div className="h-5 w-5 animate-spin rounded-full border border-edge-strong border-t-accent-hover" />
          </div>
        )}

        {error && (
          <ErrorPanel title="Could not load changes" error={error} compact />
        )}

        {!isLoading && !error && filteredChanges.length === 0 && (
          <div className="flex h-64 flex-col items-center justify-center gap-3 rounded-md border border-dashed border-edge px-6 text-center">
            <p className="text-sm font-medium text-ink-secondary">No {filterStatus !== "All" ? filterStatus.toLowerCase() : "proposed"} changes</p>
            <p className="text-xs text-ink-muted leading-relaxed">
              Ask the agent to audit your library, or manually edit track metadata.<br/>
              Changes will appear here for you to review and export.
            </p>
          </div>
        )}

        <div className="space-y-5">
          {Object.entries(groups).map(([targetId, groupChanges]) => {
            const proposedInGroup = groupChanges.filter(
              (c) => c.status === "Proposed",
            );
            return (
              <DiffGroup
                key={targetId}
                header={(() => {
                  const t = trackById.get(targetId);
                  if (!t || (!t.title && !t.artist)) return `Track ${targetId.slice(0, 8)}`;
                  return [t.title ?? "Untitled", t.artist].filter(Boolean).join(" — ");
                })()}
                count={groupChanges.length}
                proposedCount={proposedInGroup.length}
                disabled={isMutating}
                onAcceptAll={() => {
                  for (const change of proposedInGroup) acceptChange(change.id);
                }}
              >
                {groupChanges.map((change) => (
                  <ChangeCard
                    key={change.id}
                    change={change}
                    onAccept={() => acceptChange(change.id)}
                    onReject={() => rejectChange(change.id)}
                    disabled={isMutating}
                  />
                ))}
              </DiffGroup>
            );
          })}

          {noTarget.length > 0 && (
            <DiffGroup
              header="Global"
              count={noTarget.length}
              proposedCount={noTarget.filter((c) => c.status === "Proposed").length}
              disabled={isMutating}
              onAcceptAll={() => {
                for (const change of noTarget) {
                  if (change.status === "Proposed") acceptChange(change.id);
                }
              }}
            >
              {noTarget.map((change) => (
                <ChangeCard
                  key={change.id}
                  change={change}
                  onAccept={() => acceptChange(change.id)}
                  onReject={() => rejectChange(change.id)}
                  disabled={isMutating}
                />
              ))}
            </DiffGroup>
          )}
        </div>
      </div>

      <UndoHistoryPanel libraryPath={libraryPath} onStaged={() => void refetch()} />
    </aside>
  );
}

function DiffGroup({
  header,
  count,
  proposedCount,
  onAcceptAll,
  disabled,
  children,
}: {
  header: string;
  count: number;
  proposedCount: number;
  onAcceptAll: () => void;
  disabled: boolean;
  children: React.ReactNode;
}) {
  const [collapsed, setCollapsed] = useState(false);
  return (
    <div className="overflow-hidden rounded-md border border-edge bg-elevated">
      <div className="sticky top-0 z-10 flex items-center gap-2 border-b border-edge bg-elevated px-3 py-1.5">
        <button
          type="button"
          onClick={() => setCollapsed((v) => !v)}
          aria-label={collapsed ? "Expand group" : "Collapse group"}
          className="flex h-4 w-4 shrink-0 items-center justify-center rounded text-ink-muted transition-colors hover:bg-elevated hover:text-ink-secondary"
        >
          <svg
            viewBox="0 0 16 16"
            fill="currentColor"
            className={`h-3 w-3 transition-transform duration-150 ${collapsed ? "" : "rotate-90"}`}
          >
            <path d="M5.22 4.22a.75.75 0 011.06 0l3.25 3.25a.75.75 0 010 1.06l-3.25 3.25a.75.75 0 11-1.06-1.06L7.94 8 5.22 5.28a.75.75 0 010-1.06z" />
          </svg>
        </button>
        <h3 className="truncate font-mono text-[11px] text-ink-secondary">
          {header}
        </h3>
        <span className="rounded bg-elevated px-1.5 py-0.5 font-mono text-[10px] tabular-nums text-ink-secondary">
          {count}
        </span>
        {proposedCount > 0 && (
          <button
            type="button"
            onClick={onAcceptAll}
            disabled={disabled}
            className="ml-auto rounded border border-emerald-700/60 px-2 py-0.5 text-[10px] uppercase tracking-wider text-emerald-300 transition-colors hover:border-emerald-500 hover:bg-emerald-900/30 disabled:cursor-not-allowed disabled:opacity-40"
          >
            Accept {proposedCount}
          </button>
        )}
      </div>
      {!collapsed && <div className="space-y-2 p-2">{children}</div>}
    </div>
  );
}

function ChangeCard({
  change,
  onAccept,
  onReject,
  disabled,
}: {
  change: StagedChange;
  onAccept: () => void;
  onReject: () => void;
  disabled: boolean;
}) {
  const canReview = change.status === "Proposed";
  return (
    <article className="rounded-md border border-edge bg-surface p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="truncate text-sm font-medium text-ink">
            {change.field ?? formatKind(change.kind)}
          </div>
          <div className="mt-0.5 text-xs text-ink-muted">
            {formatKind(change.kind)}
            {change.target_id ? ` · ${change.target_id}` : ""}
          </div>
        </div>
        <span className={statusClass(change.status)}>
          {STATUS_LABELS[change.status]}
        </span>
      </div>

      <div className="mt-3 grid grid-cols-2 gap-2 text-xs">
        <ValueBox label="Old" value={change.old_value} />
        <ValueBox label="New" value={change.new_value} highlight />
      </div>

      {(change.reason || change.confidence !== null) && (
        <div className="mt-3 text-xs leading-relaxed text-ink-secondary">
          {change.reason}
          {change.confidence !== null && (
            <span className="text-ink-faint">
              {change.reason ? " · " : ""}
              {Math.round(change.confidence * 100)}% confidence
            </span>
          )}
        </div>
      )}

      {canReview && (
        <div className="mt-3 flex justify-end gap-2">
          <button
            onClick={onReject}
            disabled={disabled}
            className="rounded-md px-2 py-1 text-xs text-ink-secondary transition-colors hover:bg-elevated hover:text-red-300 disabled:opacity-40"
          >
            Reject
          </button>
          <button
            onClick={onAccept}
            disabled={disabled}
            className="rounded-md bg-emerald-700 px-2 py-1 text-xs text-white transition-colors hover:bg-emerald-600 disabled:opacity-40"
          >
            Accept
          </button>
        </div>
      )}
    </article>
  );
}

function ValueBox({
  label,
  value,
  highlight = false,
}: {
  label: string;
  value: unknown;
  highlight?: boolean;
}) {
  return (
    <div className="min-w-0 rounded border border-edge bg-base px-2 py-1.5">
      <div className="mb-1 text-[10px] uppercase text-ink-faint">{label}</div>
      <div
        className={`truncate font-mono ${highlight ? "text-emerald-300" : "text-ink-secondary"}`}
        title={formatValue(value)}
      >
        {formatValue(value)}
      </div>
    </div>
  );
}

function countByStatus(changes: StagedChange[]) {
  return changes.reduce<Partial<Record<StagedChange["status"], number>>>(
    (acc, change) => {
      acc[change.status] = (acc[change.status] ?? 0) + 1;
      return acc;
    },
    {},
  );
}

function formatValue(value: unknown): string {
  if (value === null || value === undefined) return "Empty";
  if (typeof value === "string") return value || "Empty";
  return JSON.stringify(value);
}

function formatKind(kind: string): string {
  return kind.replace(/([A-Z])/g, " $1").trim();
}

function statusClass(status: StagedChange["status"]): string {
  const base = "shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium";
  if (status === "Accepted") return `${base} bg-emerald-950 text-emerald-300`;
  if (status === "Rejected") return `${base} bg-red-950 text-red-300`;
  if (status === "Exported") return `${base} bg-accent-dim/40 text-accent-hover`;
  return `${base} bg-elevated text-ink-secondary`;
}
