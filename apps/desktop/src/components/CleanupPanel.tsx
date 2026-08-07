import { useCallback, useEffect, useMemo, useState } from "react";
import {
  listGenres,
  listArtists,
  renameGenre,
  renameArtist,
  deleteGenre,
  deleteArtist,
  listCleanupLocks,
  toggleCleanupLock,
  listPinnedLetters,
  togglePinnedLetter,
} from "../ipc";
import { useDialog } from "../hooks/useDialog";
import { useToast } from "./Toast";

interface Props {
  mode: "genre" | "artist";
  libraryPath: string;
  onGoToSync?: () => void;
  /** Alt/Option-click a chip to filter the track browser to that value. */
  onFilterTo?: (mode: "genre" | "artist", value: string) => void;
}

type SortMode = "name" | "count";

interface Item {
  name: string;
  count: number;
}

/** The letter a value files under. Anything non-alphabetic groups as "#". */
function initial(name: string): string {
  const first = name.trim().charAt(0).toUpperCase();
  return /[A-Z]/.test(first) ? first : "#";
}

export function CleanupPanel({ mode, libraryPath, onGoToSync, onFilterTo }: Props) {
  const dialog = useDialog();
  const { toast } = useToast();

  const [items, setItems] = useState<Item[]>([]);
  const [selectedItems, setSelectedItems] = useState<Set<string>>(new Set());
  const [locked, setLocked] = useState<Set<string>>(new Set());
  const [pinned, setPinned] = useState<Set<string>>(new Set());
  const [sort, setSort] = useState<SortMode>("count");

  const loadData = useCallback(async () => {
    try {
      if (mode === "genre") {
        const res = await listGenres(libraryPath);
        setItems(res.map((g) => ({ name: g.genre, count: g.count })));
      } else {
        const res = await listArtists(libraryPath);
        setItems(res.map((a) => ({ name: a.artist, count: a.count })));
      }
    } catch (e) {
      toast({ variant: "error", message: `Failed to load ${mode}s`, detail: String(e) });
    }
  }, [mode, libraryPath, toast]);

  const loadState = useCallback(async () => {
    try {
      const [locks, letters] = await Promise.all([
        listCleanupLocks(mode),
        listPinnedLetters(mode),
      ]);
      setLocked(new Set(Array.isArray(locks) ? locks : []));
      setPinned(new Set(Array.isArray(letters) ? letters : []));
    } catch {
      // Locks are a convenience; failing to load them must not take the panel
      // down with them.
      setLocked(new Set());
      setPinned(new Set());
    }
  }, [mode]);

  useEffect(() => {
    void loadData();
    void loadState();
    setSelectedItems(new Set());
  }, [loadData, loadState]);

  const sorted = useMemo(() => {
    const rows = [...items];
    if (sort === "name") {
      rows.sort((a, b) => a.name.localeCompare(b.name));
    } else {
      // Count descending, then name — otherwise ties shuffle between loads.
      rows.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
    }
    return rows;
  }, [items, sort]);

  /** Letters actually present, so the bar never offers a dead jump. */
  const letters = useMemo(() => {
    const present = new Set(items.map((i) => initial(i.name)));
    return [...present].sort();
  }, [items]);

  const handleToggle = (name: string, multi: boolean) => {
    // A locked value cannot be selected at all: that is what the lock is for.
    if (locked.has(name)) {
      toast({
        variant: "info",
        message: `“${name}” is locked. Right-click to unlock it.`,
      });
      return;
    }
    const next = new Set(multi ? selectedItems : []);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    setSelectedItems(next);
  };

  const handleLock = async (name: string) => {
    try {
      const now = await toggleCleanupLock(mode, name);
      setLocked((prev) => {
        const next = new Set(prev);
        if (now) next.add(name);
        else next.delete(name);
        return next;
      });
      // Locking something already selected would leave it selected and
      // unselectable, which reads as the lock not working.
      if (now) {
        setSelectedItems((prev) => {
          const next = new Set(prev);
          next.delete(name);
          return next;
        });
      }
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  };

  const handlePin = async (letter: string) => {
    try {
      const now = await togglePinnedLetter(mode, letter);
      setPinned((prev) => {
        const next = new Set(prev);
        if (now) next.add(letter);
        else next.delete(letter);
        return next;
      });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  };

  // Cmd/Ctrl+A selects everything unlocked; Esc clears. Per the spec, and the
  // "unlocked" half is the point — otherwise a lock would not protect against
  // the one gesture most likely to sweep it up.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const target = e.target;
      if (
        target instanceof HTMLElement &&
        (target.tagName === "INPUT" || target.tagName === "TEXTAREA")
      ) {
        return;
      }
      if (e.key === "Escape") {
        setSelectedItems(new Set());
        return;
      }
      if (e.key.toLowerCase() === "a" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setSelectedItems(
          new Set(items.filter((i) => !locked.has(i.name)).map((i) => i.name)),
        );
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [items, locked]);

  const afterStage = (totalTracks: number) => {
    if (totalTracks === 0) {
      toast({ variant: "info", message: "No tracks matched; nothing staged." });
      return;
    }
    toast({
      variant: "success",
      message: `Staged ${totalTracks} change(s).`,
      detail: "Review and apply in the Sync panel.",
      action: onGoToSync
        ? { label: "Review & Sync", onClick: onGoToSync }
        : undefined,
    });
  };

  const handleRename = async () => {
    if (selectedItems.size === 0) return;
    const newName = await dialog.prompt({
      title: `Rename ${selectedItems.size} ${mode}${selectedItems.size === 1 ? "" : "s"}`,
      body: `Selected: ${[...selectedItems].slice(0, 6).join(", ")}${
        selectedItems.size > 6 ? "…" : ""
      }`,
      placeholder: `New ${mode} name`,
      confirmLabel: "Stage rename",
    });
    if (!newName) return;

    let totalTracks = 0;
    for (const name of selectedItems) {
      const res =
        mode === "genre"
          ? await renameGenre(libraryPath, name, newName)
          : await renameArtist(libraryPath, name, newName);
      totalTracks += res.affected_tracks;
    }
    afterStage(totalTracks);
    await loadData();
    setSelectedItems(new Set());
  };

  const handleDelete = async () => {
    if (selectedItems.size === 0) return;
    const ok = await dialog.confirm({
      title: `Stage deletion of ${selectedItems.size} ${mode}${selectedItems.size === 1 ? "" : "s"}?`,
      body: `This clears the ${mode} field on every matching track. Nothing is written to master.db until you apply in the Sync panel.`,
      confirmLabel: "Stage deletion",
      destructive: true,
    });
    if (!ok) return;

    let totalTracks = 0;
    for (const name of selectedItems) {
      const res =
        mode === "genre"
          ? await deleteGenre(libraryPath, name)
          : await deleteArtist(libraryPath, name);
      totalTracks += res.affected_tracks;
    }
    afterStage(totalTracks);
    await loadData();
    setSelectedItems(new Set());
  };

  return (
    <div className="flex h-full flex-col bg-surface p-4 text-sm">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold capitalize text-ink">{mode} Cleanup</h2>
        <div className="flex items-center gap-2">
          <label className="text-xs text-ink-muted">
            <span className="mr-1">Sort</span>
            <select
              aria-label="Sort by"
              className="rounded border border-edge bg-elevated px-2 py-1 text-xs text-ink"
              value={sort}
              onChange={(e) => setSort(e.target.value as SortMode)}
            >
              <option value="count">Track count</option>
              <option value="name">Name</option>
            </select>
          </label>
          {onGoToSync && (
            <button
              onClick={onGoToSync}
              className="rounded bg-elevated px-3 py-1 text-ink hover:bg-edge"
            >
              Review &amp; Sync →
            </button>
          )}
          <button
            disabled={selectedItems.size === 0}
            onClick={handleRename}
            className="rounded bg-elevated px-3 py-1 font-medium text-ink hover:bg-edge disabled:opacity-50"
          >
            Rename
          </button>
          <button
            disabled={selectedItems.size === 0}
            onClick={handleDelete}
            className="rounded bg-red-500/10 px-3 py-1 font-medium text-red-500 hover:bg-red-500/20 disabled:opacity-50"
          >
            Delete
          </button>
        </div>
      </div>

      {letters.length > 0 && (
        <div
          className="mb-2 flex flex-wrap items-center gap-1 text-[11px]"
          aria-label="Letter navigation"
        >
          {letters.map((letter) => (
            <button
              key={letter}
              aria-label={`Jump to ${letter}`}
              title="Click to jump · right-click to pin"
              className={`rounded px-1.5 py-0.5 ${
                pinned.has(letter)
                  ? "bg-accent/20 font-semibold text-accent-hover"
                  : "text-ink-muted hover:bg-elevated"
              }`}
              onClick={() =>
                document
                  .getElementById(`cleanup-letter-${letter}`)
                  ?.scrollIntoView({ block: "nearest" })
              }
              onContextMenu={(e) => {
                e.preventDefault();
                void handlePin(letter);
              }}
            >
              {letter}
            </button>
          ))}
          <span className="ml-2 text-ink-faint">
            Right-click a letter to pin it · right-click a chip to lock it
          </span>
        </div>
      )}

      <div className="flex-1 overflow-y-auto rounded-lg border border-edge bg-base p-4">
        {sorted.length === 0 ? (
          <div className="flex h-full items-center justify-center text-ink-muted">
            No {mode}s found.
          </div>
        ) : (
          <div className="flex flex-wrap gap-2">
            {sorted.map((item, i) => {
              const isSelected = selectedItems.has(item.name);
              const isLocked = locked.has(item.name);
              // Anchor the first chip of each letter, so the bar can jump to it.
              const letter = initial(item.name);
              const firstOfLetter =
                sort === "name" &&
                (i === 0 || initial(sorted[i - 1].name) !== letter);
              return (
                <button
                  key={item.name}
                  id={firstOfLetter ? `cleanup-letter-${letter}` : undefined}
                  onClick={(e) => {
                    // Alt/Option-click filters the browser instead of selecting.
                    if (e.altKey && onFilterTo) {
                      onFilterTo(mode, item.name);
                      return;
                    }
                    handleToggle(item.name, e.shiftKey || e.metaKey);
                  }}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    void handleLock(item.name);
                  }}
                  // No `aria-label`: it would replace the accessible name and
                  // throw away the count, which is the number the user is
                  // actually reading. Locked state is appended as text instead.
                  title={isLocked ? "Locked — right-click to unlock" : undefined}
                  className={`flex items-center gap-2 rounded-full px-3 py-1.5 text-xs transition-colors ${
                    isSelected
                      ? "bg-accent text-base"
                      : isLocked
                        ? "bg-elevated text-ink-muted ring-1 ring-edge-strong"
                        : "bg-elevated text-ink hover:bg-edge"
                  }`}
                >
                  {isLocked && <span aria-hidden>🔒</span>}
                  <span className="max-w-[200px] truncate font-medium">
                    {item.name}
                  </span>
                  <span
                    className={`tabular-nums ${isSelected ? "text-base/80" : "text-ink-muted"}`}
                  >
                    {item.count}
                  </span>
                  {isLocked && <span className="sr-only">locked</span>}
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
