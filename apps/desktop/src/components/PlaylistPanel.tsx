import { useCallback, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ChevronLeftIcon } from "lucide-react";
import { usePlaylistDetail, usePlaylists } from "../hooks/usePlaylists";
import { findDuplicates } from "../lib/playlist-dedupe";
import { TrackTimeline } from "./TrackTimeline";
import { useToast } from "./Toast";
import { applyPlaylistMove } from "../ipc";
import { canDropInto, siblingsOf } from "../lib/playlist-tree";
import type { Playlist, Track } from "../types";

function formatDuration(secs: number | null): string {
  if (secs == null || secs <= 0) return "—";
  const m = Math.floor(secs / 60);
  const s = String(secs % 60).padStart(2, "0");
  return `${m}:${s}`;
}

function missingFieldsLabel(t: Track): string {
  const missing: string[] = [];
  if (!t.artist || t.artist.trim() === "") missing.push("artist");
  if (t.bpm == null || t.bpm <= 0) missing.push("BPM");
  if (!t.musical_key) missing.push("key");
  if (!t.genre) missing.push("genre");
  return missing.length === 0 ? "" : `Missing: ${missing.join(", ")}`;
}

interface Props {
  libraryPath: string;
  selectedTrackId?: string | null;
  onSelectTrack?: (track: Track) => void;
  onTrackContextMenu?: (
    track: Track,
    anchor: { x: number; y: number },
    options?: { playlistId?: string },
  ) => void;
  /** Jump to a playlist from outside — a favourite hotkey, for instance.
   *  Each distinct value selects once; the panel owns the selection after. */
  focusPlaylistId?: string | null;
}

interface TreeNode {
  playlist: Playlist;
  children: TreeNode[];
  depth: number;
  /** Total number of non-folder descendants (recursive). */
  leafCount: number;
}

function isFolder(p: Playlist): boolean {
  return p.kind === "Folder";
}


/** Build a hierarchical tree from a flat list. Playlists whose parent_id
 *  is missing from the list are promoted to root. */
function buildTree(playlists: Playlist[]): TreeNode[] {
  const byId = new Map(playlists.map((p) => [p.id, p]));
  const byParent = new Map<string, Playlist[]>();
  const ROOT = "__root__";
  for (const p of playlists) {
    const key = p.parent_id && byId.has(p.parent_id) ? p.parent_id : ROOT;
    if (!byParent.has(key)) byParent.set(key, []);
    byParent.get(key)!.push(p);
  }
  for (const bucket of byParent.values()) {
    bucket.sort(
      (a, b) =>
        (a.seq ?? Number.MAX_SAFE_INTEGER) -
        (b.seq ?? Number.MAX_SAFE_INTEGER) ||
        a.name.localeCompare(b.name),
    );
  }
  const build = (parentKey: string, depth: number): TreeNode[] => {
    const children = byParent.get(parentKey) ?? [];
    return children.map((p) => {
      const childNodes = build(p.id, depth + 1);
      const leafCount = isFolder(p)
        ? childNodes.reduce((sum, c) => sum + c.leafCount, 0)
        : 1;
      return { playlist: p, children: childNodes, depth, leafCount };
    });
  };
  return build(ROOT, 0);
}

/** Flatten a tree into the visible row order, honoring per-folder
 *  expanded state. Folders themselves appear as rows. */
function flattenForRender(
  tree: TreeNode[],
  expanded: Set<string>,
): TreeNode[] {
  const out: TreeNode[] = [];
  const walk = (nodes: TreeNode[]) => {
    for (const node of nodes) {
      out.push(node);
      if (isFolder(node.playlist) && expanded.has(node.playlist.id)) {
        walk(node.children);
      }
    }
  };
  walk(tree);
  return out;
}

/** When the user types a filter, collapse to a flat list of matching
 *  non-folder playlists. */
function matchesFilter(p: Playlist, query: string): boolean {
  return p.name.toLowerCase().includes(query);
}

export function PlaylistPanel({
  libraryPath,
  selectedTrackId,
  onSelectTrack,
  onTrackContextMenu,
  focusPlaylistId = null,
}: Props) {
  const [filter, setFilter] = useState("");
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [didInitExpanded, setDidInitExpanded] = useState(false);
  const [sidebarHidden, setSidebarHidden] = useState(false);
  const { data: playlists = [], isLoading, error } = usePlaylists(libraryPath);

  const tree = useMemo(() => buildTree(playlists), [playlists]);

  /** The row being dragged, and the folder currently under the pointer. */
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);

  /**
   * Stage a move into `targetId`.
   *
   * Staged, not written — the tree redraws from `master.db`, so the row does
   * not appear to move until Sync applies it. That is the honest behaviour for
   * a change that has not happened yet, and it matches every other edit here.
   */
  const dropInto = useCallback(
    async (targetId: string) => {
      const dragged = draggingId;
      setDraggingId(null);
      setDropTargetId(null);
      if (!dragged || !libraryPath) return;
      if (!canDropInto(dragged, targetId, playlists)) return;

      const moving = playlists.find((p) => p.id === dragged);
      try {
        await applyPlaylistMove(
          libraryPath,
          dragged,
          targetId,
          moving?.parent_id ?? null,
          // Last within the destination, which is where a drop onto a folder
          // reads as putting something.
          siblingsOf(targetId, playlists).length + 1,
        );
        toast({
          variant: "success",
          message: "Move staged — apply it in Sync to move it in Rekordbox.",
        });
        await queryClient.invalidateQueries({
          queryKey: ["staged-changes", libraryPath],
        });
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [draggingId, libraryPath, playlists, queryClient, toast],
  );

  // On first load with real data, expand top-level folders so the user
  // can see their playlists without clicking.
  useEffect(() => {
    if (didInitExpanded || playlists.length === 0) return;
    const next = new Set<string>();
    for (const node of tree) {
      if (isFolder(node.playlist)) next.add(node.playlist.id);
    }
    setExpanded(next);
    setDidInitExpanded(true);
  }, [tree, playlists.length, didInitExpanded]);

  const query = filter.trim().toLowerCase();
  const filtering = query.length > 0;

  // While filtering: show a flat list of matching non-folder playlists.
  // Otherwise: respect tree + expansion.
  const visibleRows = useMemo<TreeNode[]>(() => {
    if (!filtering) return flattenForRender(tree, expanded);
    return playlists
      .filter((p) => !isFolder(p) && matchesFilter(p, query))
      .map<TreeNode>((p) => ({
        playlist: p,
        children: [],
        depth: 0,
        leafCount: 1,
      }));
  }, [filtering, tree, expanded, playlists, query]);

  // Selectable rows are non-folder leaves.
  const selectableIds = useMemo(
    () => new Set(visibleRows.filter((r) => !isFolder(r.playlist)).map((r) => r.playlist.id)),
    [visibleRows],
  );

  useEffect(() => {
    if (selectedId !== null && selectableIds.has(selectedId)) return;
    const firstLeaf = visibleRows.find((r) => !isFolder(r.playlist));
    setSelectedId(firstLeaf ? firstLeaf.playlist.id : null);
  }, [selectableIds, selectedId, visibleRows]);

  // Jump to a playlist asked for from outside. It also has to be *reachable*:
  // a favourite inside a collapsed folder would otherwise select invisibly and
  // then be reset by the fallback effect above.
  useEffect(() => {
    if (focusPlaylistId == null) return;
    const target = playlists.find((p) => p.id === focusPlaylistId);
    if (!target) return;
    setFilter("");
    setExpanded((prev) => {
      const next = new Set(prev);
      let parent = target.parent_id;
      while (parent != null) {
        next.add(parent);
        parent = playlists.find((p) => p.id === parent)?.parent_id ?? null;
      }
      return next;
    });
    setSelectedId(focusPlaylistId);
  }, [focusPlaylistId, playlists]);

  const toggleFolder = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const { data: detail, isLoading: detailLoading } = usePlaylistDetail(
    libraryPath,
    selectedId,
  );

  if (isLoading) {
    return (
      <div className="flex h-64 items-center justify-center border-b border-edge">
        <div className="h-5 w-5 animate-spin rounded-full border border-edge-strong border-t-accent-hover" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="border-b border-edge p-4 text-sm text-red-400">
        Failed to load playlists: {error.message}
      </div>
    );
  }

  return (
    <div
      data-testid="playlist-panel"
      className="flex min-h-0 flex-1 border-b border-edge bg-base"
    >
      {!sidebarHidden && (
        <div className="flex w-64 shrink-0 flex-col border-r border-edge">
          <div className="border-b border-edge p-2">
            <input
              type="search"
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder="Filter playlists…"
              className="w-full rounded-md border border-edge-strong bg-surface px-2 py-1.5 text-sm text-ink placeholder:text-ink-faint focus:border-accent focus:outline-none"
            />
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto py-1">
            {visibleRows.length === 0 ? (
              <p className="p-4 text-sm text-ink-muted">No playlists found.</p>
            ) : (
              visibleRows.map((node) => {
                const p = node.playlist;
                const folder = isFolder(p);
                const isOpen = folder && expanded.has(p.id);
                return (
                  <PlaylistRow
                    key={p.id}
                    node={node}
                    isFolder={folder}
                    isOpen={isOpen}
                    isSelected={!folder && selectedId === p.id}
                    isDropTarget={dropTargetId === p.id}
                    onDragStart={() => setDraggingId(p.id)}
                    onDragEnd={() => {
                      setDraggingId(null);
                      setDropTargetId(null);
                    }}
                    onDragOver={() => {
                      // Only highlight somewhere the drop would actually be
                      // accepted — a target that lights up and then refuses is
                      // worse than one that never lit up.
                      if (
                        draggingId &&
                        canDropInto(draggingId, p.id, playlists)
                      ) {
                        setDropTargetId(p.id);
                      }
                    }}
                    onDrop={() => void dropInto(p.id)}
                    onClick={() => {
                      if (folder) toggleFolder(p.id);
                      else setSelectedId(p.id);
                    }}
                  />
                );
              })
            )}
          </div>
        </div>
      )}

      <div className="min-w-0 flex-1 overflow-y-auto">
        <div className="sticky top-0 z-20 flex h-10 items-center border-b border-edge bg-base/80 px-2 backdrop-blur-md">
          <button
            onClick={() => setSidebarHidden(!sidebarHidden)}
            className="group flex h-7 w-7 items-center justify-center rounded-md text-ink-muted transition-colors hover:bg-elevated hover:text-ink"
            title={sidebarHidden ? "Show playlist browser" : "Hide playlist browser"}
          >
            <ChevronLeftIcon
              className={[
                "h-4 w-4 transition-transform duration-200",
                sidebarHidden ? "rotate-180" : "",
              ].join(" ")}
            />
          </button>
          {!sidebarHidden && <div className="h-4 w-px bg-edge-strong ml-1 mr-3" />}
          {detail?.playlist && (
            <h2 className="truncate text-[13px] font-semibold text-ink">
              {detail.playlist.name}
            </h2>
          )}
        </div>
        {detailLoading ? (
          <div className="flex h-full items-center justify-center">
            <div className="h-5 w-5 animate-spin rounded-full border border-edge-strong border-t-accent-hover" />
          </div>
        ) : detail === null || detail === undefined ? (
          <div className="flex h-full items-center justify-center text-sm text-ink-muted">
            Select a playlist.
          </div>
        ) : detail.tracks.length === 0 ? (
          <div className="p-4">
            <p className="mt-2 text-sm text-ink-muted">No tracks in this playlist.</p>
          </div>
        ) : (
          <>
            <TrackTimeline
              tracks={detail.tracks}
              label={detail.playlist.name}
              onSelectTrack={(id: string) => {
                const found = detail.tracks.find((t) => t.id === id);
                if (found) onSelectTrack?.(found);
              }}
            />
            <PlaylistTrackList
              tracks={detail.tracks}
              playlistId={detail.playlist.id}
              selectedTrackId={selectedTrackId ?? null}
              onSelectTrack={onSelectTrack}
              onTrackContextMenu={onTrackContextMenu}
            />
          </>
        )}
      </div>
    </div>
  );
}

function PlaylistRow({
  node,
  isFolder,
  isOpen,
  isSelected,
  isDropTarget,
  onClick,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDrop,
}: {
  node: TreeNode;
  isFolder: boolean;
  isOpen: boolean;
  isSelected: boolean;
  isDropTarget: boolean;
  onClick: () => void;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDragOver: () => void;
  onDrop: () => void;
}) {
  const { playlist: p, depth, leafCount } = node;
  const indent = depth * 12;
  const isSmart = p.kind === "SmartPlaylist";

  return (
    <button
      type="button"
      onClick={onClick}
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onDragOver={(e) => {
        e.preventDefault();
        onDragOver();
      }}
      onDrop={(e) => {
        e.preventDefault();
        onDrop();
      }}
      data-drop-target={isDropTarget || undefined}
      aria-expanded={isFolder ? isOpen : undefined}
      className={[
        "flex w-full items-center gap-1.5 px-2 py-1 text-left text-[13px] transition-colors duration-150",
        isSelected
          ? "bg-accent/15 text-ink"
          : isFolder
            ? "text-ink hover:bg-elevated/60"
            : "text-ink-secondary hover:bg-elevated/60 hover:text-ink",
        isDropTarget ? "ring-1 ring-inset ring-accent" : "",
      ].join(" ")}
      style={{ paddingLeft: indent + 6 }}
    >
      {/* Chevron column (always reserved so leaves and folder items align) */}
      <span className="flex h-3 w-3 shrink-0 items-center justify-center text-ink-muted">
        {isFolder ? (
          <svg
            viewBox="0 0 16 16"
            fill="currentColor"
            className={`h-2.5 w-2.5 transition-transform duration-150 ${isOpen ? "rotate-90" : ""}`}
          >
            <path d="M5.22 4.22a.75.75 0 011.06 0l3.25 3.25a.75.75 0 010 1.06l-3.25 3.25a.75.75 0 11-1.06-1.06L7.94 8 5.22 5.28a.75.75 0 010-1.06z" />
          </svg>
        ) : null}
      </span>

      {/* Icon */}
      <span className="flex h-4 w-4 shrink-0 items-center justify-center text-ink-muted">
        {isFolder ? (
          <svg
            viewBox="0 0 16 16"
            fill="currentColor"
            className="h-3.5 w-3.5"
          >
            <path d="M.54 3.87L.5 3a2 2 0 012-2h3.672a2 2 0 011.414.586l.828.828A2 2 0 009.828 3h3.982a2 2 0 011.992 2.181l-.637 7A2 2 0 0113.174 14H2.826a2 2 0 01-1.991-1.819l-.637-7a1.99 1.99 0 01.342-1.31zM2.19 3a1 1 0 00-.998 1.094L1.81 11.09a1 1 0 00.998.91h10.384a1 1 0 00.998-.91L14.81 4.094A1 1 0 0013.81 3H9.828a1 1 0 01-.707-.293L8.293 1.879A1 1 0 007.586 1.5H4.5z" />
          </svg>
        ) : isSmart ? (
          <svg
            viewBox="0 0 16 16"
            fill="currentColor"
            className="h-3.5 w-3.5"
          >
            <path d="M8 1.5a.5.5 0 01.5.5v1.5h1.5a.5.5 0 010 1H8.5V6a.5.5 0 01-1 0V4.5H6a.5.5 0 010-1h1.5V2a.5.5 0 01.5-.5zM3 7a1 1 0 011-1h8a1 1 0 011 1v6a2 2 0 01-2 2H5a2 2 0 01-2-2V7zm1 0v6a1 1 0 001 1h6a1 1 0 001-1V7H4z" />
          </svg>
        ) : (
          <svg
            viewBox="0 0 16 16"
            fill="currentColor"
            className="h-3.5 w-3.5"
          >
            <path d="M1 2.5A.5.5 0 011.5 2h13a.5.5 0 010 1h-13a.5.5 0 01-.5-.5zm0 3A.5.5 0 011.5 5h13a.5.5 0 010 1h-13a.5.5 0 01-.5-.5zm0 3A.5.5 0 011.5 8h13a.5.5 0 010 1h-13a.5.5 0 01-.5-.5zm0 3a.5.5 0 01.5-.5h13a.5.5 0 010 1h-13a.5.5 0 01-.5-.5z" />
          </svg>
        )}
      </span>

      <span className="min-w-0 flex-1 truncate">{p.name}</span>

      {/* Right-side count */}
      {isFolder && leafCount > 0 && (
        <span className="ml-auto shrink-0 font-mono text-[10px] tabular-nums text-ink-faint">
          {leafCount}
        </span>
      )}
    </button>
  );
}

function PlaylistTrackList({
  tracks,
  playlistId,
  selectedTrackId,
  onSelectTrack,
  onTrackContextMenu,
}: {
  tracks: Track[];
  playlistId: string;
  selectedTrackId: string | null;
  onSelectTrack?: (track: Track) => void;
  onTrackContextMenu?: (
    track: Track,
    anchor: { x: number; y: number },
    options?: { playlistId?: string },
  ) => void;
}) {
  const dupes = findDuplicates(tracks);
  return (
    <div data-testid="playlist-track-list">
      <div className="border-b border-edge bg-base px-4 py-2.5">
        <p className="mt-0.5 flex items-center gap-2 text-[11px] text-ink-muted">
          <span>
            <span className="font-mono tabular-nums">
              {tracks.length.toLocaleString()}
            </span>{" "}
            tracks
          </span>
          {dupes.duplicateRowCount > 0 && (
            <span
              className="rounded-full border border-accent/40 px-1.5 py-px font-mono text-[9px] uppercase tracking-wider text-accent-hover"
              title={`${dupes.duplicateRowCount} duplicate ${dupes.duplicateRowCount === 1 ? "row" : "rows"} (real playlist entries in Rekordbox)`}
            >
              {dupes.duplicateRowCount}{" "}
              {dupes.duplicateRowCount === 1 ? "duplicate" : "duplicates"}
            </span>
          )}
        </p>
      </div>
      {/* Column header */}
      <div className="sticky top-0 z-[5] grid grid-cols-[1.75rem_0.75rem_minmax(110px,2.4fr)_minmax(80px,1.6fr)_minmax(70px,1fr)_3rem_2.5rem_3.25rem_2.75rem] gap-2 border-b border-edge bg-surface px-4 py-1.5 text-[9px] font-semibold uppercase tracking-[0.1em] text-ink-muted">
        <span className="text-right">#</span>
        <span aria-hidden></span>
        <span>Title</span>
        <span>Artist</span>
        <span>Genre</span>
        <span className="text-right">BPM</span>
        <span className="text-center">Key</span>
        <span className="text-right">Time</span>
        <span className="text-right">Year</span>
      </div>
      {tracks.map((track, index) => {
        const rank = dupes.duplicateRanks.get(index) ?? 1;
        const isDup = rank >= 2;
        const missingLabel = missingFieldsLabel(track);
        return (
          <button
            key={`${track.id}-${index}`}
            type="button"
            onClick={() => onSelectTrack?.(track)}
            onContextMenu={(e) => {
              if (!onTrackContextMenu) return;
              e.preventDefault();
              onTrackContextMenu(track, { x: e.clientX, y: e.clientY }, { playlistId });
            }}
            className={`grid w-full cursor-pointer grid-cols-[1.75rem_0.75rem_minmax(110px,2.4fr)_minmax(80px,1.6fr)_minmax(70px,1fr)_3rem_2.5rem_3.25rem_2.75rem] gap-2 border-b border-edge/30 px-4 py-1.5 text-left text-[12px] leading-tight transition-colors ${
              track.id === selectedTrackId
                ? "bg-accent/12 shadow-[inset_2px_0_0_0_rgb(var(--accent))] hover:bg-accent/15"
                : "hover:bg-elevated/60"
            }`}
          >
            <span className="text-right font-mono tabular-nums text-[11px] text-ink-faint">
              {index + 1}
            </span>
            <span
              aria-hidden
              title={missingLabel}
              className={`flex items-center justify-center ${missingLabel ? "" : "opacity-0"}`}
            >
              {missingLabel && (
                <span className="block h-1.5 w-1.5 rounded-full bg-status-warn" />
              )}
            </span>
            <span className="flex min-w-0 items-center gap-1.5">
              <span className="truncate text-ink">{track.title}</span>
              {isDup && (
                <span
                  className="shrink-0 rounded border border-accent/40 px-1 font-mono text-[9px] uppercase tracking-wider text-accent-hover"
                  title="Second or later occurrence of this track in the playlist"
                >
                  DUP
                </span>
              )}
            </span>
            <span className="truncate text-ink-secondary">
              {track.artist ?? "—"}
            </span>
            <span className="truncate text-ink-secondary">
              {track.genre ?? "—"}
            </span>
            <span className="text-right font-mono tabular-nums text-[11px] text-ink-secondary">
              {track.bpm != null && track.bpm > 0 ? track.bpm.toFixed(1) : "—"}
            </span>
            <span className="text-center font-mono tabular-nums text-[11px] text-ink-secondary">
              {track.musical_key ?? "—"}
            </span>
            <span className="text-right font-mono tabular-nums text-[11px] text-ink-secondary">
              {formatDuration(track.duration_secs)}
            </span>
            <span className="text-right font-mono tabular-nums text-[11px] text-ink-secondary">
              {track.release_year != null && track.release_year > 0
                ? track.release_year
                : "—"}
            </span>
          </button>
        );
      })}
    </div>
  );
}
