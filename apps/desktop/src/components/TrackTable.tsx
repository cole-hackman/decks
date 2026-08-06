import { useCallback, useMemo, useRef, useState } from "react";
import { SearchIcon, Wand2 } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  getFilteredRowModel,
  type ColumnDef,
  type SortingState,
  type ColumnFiltersState,
  flexRender,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useLibrary } from "../hooks/useLibrary";
import { useKeyboardShortcuts } from "../hooks/useKeyboardShortcuts";
import { EmptyState } from "./EmptyState";
import { ErrorPanel } from "./ErrorPanel";
import { applyFilters, type FilterContext, type Filters } from "../lib/filters";
import { colorForKey, toCamelot } from "../lib/camelot";
import { EnergyBar } from "./EnergyBar";
import type { Track } from "../types";

const ROW_H = 28;

const MAX_INLINE_TAGS = 3;

function buildColumns(
  tagsByTrack: Map<string, Set<string>>,
  tagLabelById: Record<string, string>,
  showTagsColumn: boolean,
  /** Camelot codes that mix out of the reference track, under the global Key
   *  Mixing Mode. Empty when there is no reference or its key is unreadable —
   *  in which case no row is marked, rather than every row being marked. */
  compatibleKeys: Set<string>,
): ColumnDef<Track>[] {
  const cols: ColumnDef<Track>[] = [
    {
      accessorKey: "title",
      header: "Title",
      size: 280,
      cell: (info) => <span className="truncate">{info.getValue<string>()}</span>,
    },
    {
      accessorKey: "artist",
      header: "Artist",
      size: 180,
      cell: (info) => (
        <span className="truncate text-ink-secondary">{info.getValue<string | null>() ?? "—"}</span>
      ),
    },
    {
      accessorKey: "bpm",
      header: "BPM",
      size: 64,
      meta: { align: "right" },
      cell: (info) => {
        const v = info.getValue<number | null>();
        return v != null && v > 0 ? v.toFixed(1) : "—";
      },
    },
    {
      accessorKey: "musical_key",
      header: "Key",
      size: 56,
      meta: { align: "center" },
      cell: (info) => {
        const value = info.getValue<string | null>();
        if (value == null || value === "") return "—";
        const color = colorForKey(value);
        const camelot = toCamelot(value);
        // Only ever a *positive* mark. An unmarked row means "not compatible
        // or we cannot tell", and those are not worth distinguishing at a
        // glance; marking every non-match would drown the ones that are.
        const mixes =
          compatibleKeys.size > 0 &&
          camelot != null &&
          compatibleKeys.has(camelot);
        return (
          <span
            style={color ? { color } : undefined}
            className="font-mono"
            title={mixes ? "Mixes with the selected track" : undefined}
          >
            {value}
            {mixes && (
              <span className="ml-0.5 text-accent" aria-label="mixes with the selected track">
                •
              </span>
            )}
          </span>
        );
      },
    },
    {
      accessorKey: "energy",
      header: "Energy",
      size: 80,
      enableColumnFilter: false,
      meta: { align: "center" },
      cell: (info) => {
        const v = info.getValue<number | null>();
        if (v == null) return "—";
        return <EnergyBar value={v} />;
      },
    },
    {
      accessorKey: "duration_secs",
      header: "Time",
      size: 60,
      meta: { align: "right" },
      cell: (info) => {
        const s = info.getValue<number | null>();
        if (s == null) return "—";
        const m = Math.floor(s / 60);
        const ss = String(s % 60).padStart(2, "0");
        return `${m}:${ss}`;
      },
    },
    {
      accessorKey: "genre",
      header: "Genre",
      size: 130,
      cell: (info) => (
        <span className="truncate text-ink-secondary">{info.getValue<string | null>() ?? "—"}</span>
      ),
    },
  ];

  if (showTagsColumn) {
    cols.push({
      id: "tags",
      header: "Tags",
      size: 200,
      enableSorting: false,
      enableColumnFilter: false,
      cell: (info) => {
        const trackId = info.row.original.id;
        const ids = tagsByTrack.get(trackId);
        if (!ids || ids.size === 0) {
          return <span className="text-ink-faint">—</span>;
        }
        const idArr = Array.from(ids);
        const visible = idArr.slice(0, MAX_INLINE_TAGS);
        const overflow = idArr.length - visible.length;
        return (
          <div className="flex items-center gap-1 overflow-hidden">
            {visible.map((id) => {
              // Tag labels look like "Mood ▸ Energetic" — keep just the leaf for
              // the chip to stay readable in a 200px column.
              const fullLabel = tagLabelById[id] ?? id;
              const leaf = fullLabel.includes("▸")
                ? fullLabel.split("▸").pop()?.trim() ?? fullLabel
                : fullLabel;
              return (
                <span
                  key={id}
                  title={fullLabel}
                  data-testid="track-tag-chip"
                  className="inline-flex max-w-[80px] truncate rounded bg-elevated px-1.5 py-0 text-[10px] text-ink-secondary"
                >
                  {leaf}
                </span>
              );
            })}
            {overflow > 0 && (
              <span className="text-[10px] text-ink-faint">+{overflow} more</span>
            )}
          </div>
        );
      },
    });
  }

  return cols;
}

function SortChevron({ direction }: { direction: "asc" | "desc" | false }) {
  if (!direction) return null;
  return (
    <svg
      viewBox="0 0 10 6"
      fill="currentColor"
      aria-hidden
      className={`h-1.5 w-2.5 text-ink-secondary transition-transform duration-150 ${direction === "asc" ? "rotate-180" : ""}`}
    >
      <path d="M0 0h10L5 6z" />
    </svg>
  );
}

interface Props {
  libraryPath: string;
  filters: Filters;
  filterCtx: FilterContext;
  selectedTrackIds: Set<string>;
  onSelectionChange: (ids: Set<string>) => void;
  onSelect: (track: Track) => void;
  onTrackContextMenu?: (track: Track, anchor: { x: number; y: number }) => void;
  /** Camelot codes that mix out of a reference track, for the compatible-key
   *  indicator. Per `docs/lexicon/04-analysis.md §Mixable Tracks`, the set
   *  follows the global Key Mixing Mode. */
  compatibleWith?: string[];
  tracksOverride?: Track[];
  /** Lookup table from tag-id → display label, used by the inline Tags column. */
  tagLabelById?: Record<string, string>;
}

export function TrackTable({
  libraryPath,
  filters,
  filterCtx,
  selectedTrackIds,
  onSelectionChange,
  onSelect,
  onTrackContextMenu,
  compatibleWith,
  tracksOverride,
  tagLabelById,
}: Props) {
  const { data: fetchedTracks = [], isLoading, error } = useLibrary(libraryPath);
  const tracks = tracksOverride ?? fetchedTracks;
  
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [showColumnFilters, setShowColumnFilters] = useState(false);
  const [lastSelectedIdx, setLastSelectedIdx] = useState<number | null>(null);
  const [isAddingCues, setIsAddingCues] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const queryClient = useQueryClient();

  const filtered = useMemo(() => {
    if (tracksOverride) return tracksOverride;
    return applyFilters(fetchedTracks, filters, filterCtx);
  }, [fetchedTracks, filters, filterCtx, tracksOverride]);

  const showTagsColumn = filterCtx.tagsByTrack.size > 0;
  const compatibleKeys = useMemo(
    () => new Set(compatibleWith ?? []),
    [compatibleWith],
  );

  const columns = useMemo(
    () =>
      buildColumns(
        filterCtx.tagsByTrack,
        tagLabelById ?? {},
        showTagsColumn,
        compatibleKeys,
      ),
    [filterCtx.tagsByTrack, tagLabelById, showTagsColumn, compatibleKeys],
  );

  const table = useReactTable({
    data: filtered,
    columns,
    state: { sorting, columnFilters },
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    columnResizeMode: "onChange",
  });

  const rows = table.getRowModel().rows;

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => ROW_H,
    overscan: 20,
  });

  const handleRowClick = useCallback(
    (event: React.MouseEvent, index: number, track: Track) => {
      const isCmd = event.metaKey || event.ctrlKey;
      const isShift = event.shiftKey;

      let next = new Set<string>(selectedTrackIds);

      if (isShift && lastSelectedIdx !== null) {
        const start = Math.min(lastSelectedIdx, index);
        const end = Math.max(lastSelectedIdx, index);
        for (let i = start; i <= end; i++) {
          next.add(rows[i].original.id);
        }
      } else if (isCmd) {
        if (next.has(track.id)) next.delete(track.id);
        else next.add(track.id);
        setLastSelectedIdx(index);
      } else {
        next = new Set([track.id]);
        setLastSelectedIdx(index);
        onSelect(track);
      }

      onSelectionChange(next);
    },
    [selectedTrackIds, lastSelectedIdx, rows, onSelect, onSelectionChange],
  );

  const moveSelection = useCallback(
    (delta: number) => {
      if (rows.length === 0) return;
      const currentIdx = lastSelectedIdx ?? -1;
      const nextIdx =
        currentIdx === -1
          ? delta > 0
            ? 0
            : rows.length - 1
          : Math.max(0, Math.min(rows.length - 1, currentIdx + delta));
      const target = rows[nextIdx];
      if (target) {
        onSelectionChange(new Set([target.original.id]));
        setLastSelectedIdx(nextIdx);
        onSelect(target.original);
        virtualizer.scrollToIndex(nextIdx, { align: "auto" });
      }
    },
    [rows, lastSelectedIdx, onSelect, onSelectionChange, virtualizer],
  );

  useKeyboardShortcuts(
    useMemo(
      () => [
        { key: "j", handler: () => moveSelection(1) },
        { key: "k", handler: () => moveSelection(-1) },
        { key: "arrowdown", handler: () => moveSelection(1) },
        { key: "arrowup", handler: () => moveSelection(-1) },
        {
          key: "a",
          meta: true,
          handler: (e) => {
            e.preventDefault();
            onSelectionChange(new Set(rows.map((r) => r.original.id)));
          },
        },
      ],
      [moveSelection, rows, onSelectionChange],
    ),
  );

  const virtualItems = virtualizer.getVirtualItems();
  const hasMultipleSelection = selectedTrackIds.size > 1;

  if (isLoading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-edge-strong border-t-accent-hover" />
      </div>
    );
  }

  if (error) {
    return <ErrorPanel title="Failed to load library" error={error} />;
  }

  const handleBulkAddCues = async () => {
    if (selectedTrackIds.size === 0 || isAddingCues) return;
    setIsAddingCues(true);
    try {
      const { libraryStageIntroCues } = await import("../ipc");
      await libraryStageIntroCues(libraryPath, Array.from(selectedTrackIds));
      await queryClient.invalidateQueries({ queryKey: ["staged-changes", libraryPath] });
    } catch (e) {
      console.error("Failed to add intro cues:", e);
    } finally {
      setIsAddingCues(false);
      onSelectionChange(new Set());
    }
  };

  return (
    <div className="relative flex flex-1 flex-col overflow-hidden">
      {/* Column-filter toggle row */}
      <div className="flex shrink-0 items-center justify-end gap-1 border-b border-edge bg-base px-2 py-1">
        <button
          type="button"
          onClick={() => setShowColumnFilters((v) => !v)}
          aria-pressed={showColumnFilters}
          aria-label={showColumnFilters ? "Hide column filters" : "Show column filters"}
          title={showColumnFilters ? "Hide column filters" : "Show column filters"}
          className={[
            "flex items-center gap-1 rounded px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-wider transition-colors",
            showColumnFilters
              ? "text-accent-hover"
              : "text-ink-faint hover:text-ink-secondary",
          ].join(" ")}
        >
          <SearchIcon className="h-2.5 w-2.5" />
          <span>Column filters</span>
        </button>
      </div>
      {/* Column headers */}
      <div
        className="flex border-b border-edge bg-surface text-[10px] font-semibold uppercase tracking-[0.08em] text-ink-muted select-none"
        style={{ width: table.getTotalSize() }}
      >
        {table.getFlatHeaders().map((header) => {
          const meta = header.column.columnDef.meta as
            | { align?: string }
            | undefined;
          const align = meta?.align;
          const sorted = header.column.getIsSorted();
          const canFilter = header.column.getCanFilter();

          return (
            <div
              key={header.id}
              style={{ width: header.getSize(), minWidth: header.getSize() }}
              className={[
                "group relative flex flex-col transition-colors duration-150",
                sorted ? "text-ink-secondary" : "",
              ].join(" ")}
            >
              <div
                className={[
                  "flex flex-1 cursor-pointer items-center gap-1 px-2.5 py-1.5 hover:text-ink-secondary",
                  align === "right"
                    ? "justify-end text-right"
                    : align === "center"
                      ? "justify-center text-center"
                      : "",
                ].join(" ")}
                onClick={header.column.getToggleSortingHandler()}
              >
                {flexRender(header.column.columnDef.header, header.getContext())}
                <SortChevron direction={sorted} />
              </div>

              {canFilter && (showColumnFilters || !!header.column.getFilterValue()) && (
                <div className="relative px-2 pb-1.5">
                  <div className="relative">
                    <SearchIcon className="absolute left-1.5 top-1/2 h-2.5 w-2.5 -translate-y-1/2 text-ink-faint" />
                    <input
                      type="text"
                      value={(header.column.getFilterValue() as string) ?? ""}
                      onChange={(e) => header.column.setFilterValue(e.target.value)}
                      onClick={(e) => e.stopPropagation()}
                      placeholder="Filter..."
                      className="w-full rounded bg-base/40 py-0.5 pl-4.5 pr-1.5 font-sans text-[9px] font-normal lowercase tracking-normal text-ink outline-none ring-accent/30 focus:bg-base/80 focus:ring-1"
                    />
                  </div>
                </div>
              )}

              {/* Resize handle */}
              <div
                onMouseDown={header.getResizeHandler()}
                onTouchStart={header.getResizeHandler()}
                className={[
                  "absolute right-0 top-0 h-full w-1 cursor-col-resize touch-none select-none",
                  header.column.getIsResizing()
                    ? "bg-accent opacity-100"
                    : "bg-edge opacity-0 group-hover:opacity-100",
                ].join(" ")}
              />
            </div>
          );
        })}
      </div>

      {/* Virtualized rows */}
      <div ref={containerRef} className="flex-1 overflow-y-auto overflow-x-auto">
        <div
          style={{
            height: virtualizer.getTotalSize(),
            width: table.getTotalSize(),
            position: "relative",
          }}
        >
          {virtualItems.map((vItem) => {
            const row = rows[vItem.index];
            const isSelected = selectedTrackIds.has(row.original.id);
            return (
              <div
                key={row.id}
                data-index={vItem.index}
                style={{
                  position: "absolute",
                  top: 0,
                  transform: `translateY(${vItem.start}px)`,
                  height: ROW_H,
                  width: "100%",
                  display: "flex",
                  alignItems: "center",
                }}
                className={[
                  "cursor-pointer border-b border-edge/30 text-[12px] leading-tight text-ink select-none",
                  isSelected
                    ? "bg-accent/12 shadow-[inset_2px_0_0_0_rgb(var(--accent))] hover:bg-accent/15"
                    : "hover:bg-elevated/60",
                ].join(" ")}
                onClick={(e) => handleRowClick(e, vItem.index, row.original)}
                onContextMenu={(e) => {
                  if (!onTrackContextMenu) return;
                  e.preventDefault();
                  onTrackContextMenu(row.original, { x: e.clientX, y: e.clientY });
                }}
              >
                {row.getVisibleCells().map((cell) => {
                  const meta = cell.column.columnDef.meta as
                    | { align?: string }
                    | undefined;
                  const align = meta?.align;
                  return (
                    <div
                      key={cell.id}
                      style={{
                        width: cell.column.getSize(),
                        minWidth: cell.column.getSize(),
                        overflow: "hidden",
                      }}
                      className={[
                        "px-2.5",
                        align === "right"
                          ? "text-right font-mono tabular-nums text-[11px]"
                          : align === "center"
                            ? "text-center font-mono tabular-nums text-[11px]"
                            : "",
                        isSelected ? "text-ink" : "",
                      ].join(" ")}
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>

        {rows.length === 0 && (
          <EmptyState
            icon={
              <svg viewBox="0 0 16 16" fill="currentColor" className="h-5 w-5">
                <path d="M11.742 10.344a6.5 6.5 0 10-1.397 1.398h-.001c.03.04.062.078.098.115l3.85 3.85a1 1 0 001.415-1.414l-3.85-3.85a1.007 1.007 0 00-.115-.1zM12 6.5a5.5 5.5 0 11-11 0 5.5 5.5 0 0111 0z" />
              </svg>
            }
            title={
              filters.query || (filtered.length === 0 && tracks.length > 0)
                ? "No tracks match your filters"
                : "Library is empty"
            }
            description={
              tracks.length > 0
                ? "Try clearing some filters or adjusting your search."
                : "Your Rekordbox library exists, but no tracks were found. Add tracks in Rekordbox and re-open this app."
            }
          />
        )}
      </div>

      {hasMultipleSelection && (
        <div className="absolute bottom-4 left-1/2 flex -translate-x-1/2 items-center gap-3 rounded-full border border-accent/40 bg-base/90 px-4 py-2 shadow-2xl shadow-black/80 backdrop-blur-md animate-[slideInUp_200ms_ease-out]">
          <span className="font-mono text-[11px] font-semibold tabular-nums text-accent-hover">
            {selectedTrackIds.size} tracks selected
          </span>
          <div className="h-3 w-px bg-edge-strong" />
          <button
            onClick={handleBulkAddCues}
            disabled={isAddingCues}
            className="flex items-center gap-1.5 rounded bg-accent/20 px-2 py-1 text-[10px] font-bold uppercase tracking-wider text-accent-hover transition-colors hover:bg-accent/40 disabled:opacity-50"
            title="Automatically detect the first beat and add a memory cue and 4-bar loop"
          >
            {isAddingCues ? (
              <span className="h-3 w-3 animate-spin rounded-full border border-accent-hover border-t-transparent" />
            ) : (
              <Wand2 className="h-3 w-3" />
            )}
            Add Intro Cues
          </button>
          <div className="h-3 w-px bg-edge-strong" />
          <button
            onClick={() => onSelectionChange(new Set())}
            className="text-[10px] font-bold uppercase tracking-wider text-ink-secondary hover:text-ink"
          >
            Deselect
          </button>
        </div>
      )}

    </div>
  );
}
