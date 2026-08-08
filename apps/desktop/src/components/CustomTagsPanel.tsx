import { useEffect, useMemo, useState } from "react";
import {
  listTagCategories,
  createTagCategory,
  listTags,
  createTag,
  deleteTag,
  previewMyTagImport,
  importMyTags,
  setTagCategoryColor,
  setTagHotkey,
  reorderTags,
} from "../ipc";
import type { TagCategory, Tag, MyTagImportPreview } from "../types";
import { indexOfId, moveWithin } from "../lib/reorder";
import { PlusIcon, ChevronDownIcon, ChevronRightIcon } from "lucide-react";

/**
 * The colours a category may carry, per `docs/lexicon/02-library.md
 * §Custom Tags`: "Categories carry a colour".
 *
 * Deliberately the same eight as Rekordbox's track palette. A category colour
 * is a visual grouping, not a value that syncs anywhere — but offering a
 * different set would invite the reasonable assumption that the two relate.
 */
const CATEGORY_COLOURS: { name: string; hex: string }[] = [
  { name: "Pink", hex: "#ff6b9d" },
  { name: "Red", hex: "#e5484d" },
  { name: "Orange", hex: "#f76b15" },
  { name: "Yellow", hex: "#f5d90a" },
  { name: "Green", hex: "#46a758" },
  { name: "Aqua", hex: "#12a5b8" },
  { name: "Blue", hex: "#3e63dd" },
  { name: "Purple", hex: "#8e4ec6" },
];


interface Props {
  /** Optional — when provided, the panel renders a "Show tracks" button that
   *  hands the selected tag IDs back to the parent (which typically updates
   *  the library filter and switches view). */
  onShowTracks?: (tagIds: string[], tagGroups: string[][]) => void;
  /** Needed to read Rekordbox's own MyTags. Absent when no library is open. */
  libraryPath?: string | null;
}

export function CustomTagsPanel({ onShowTracks, libraryPath }: Props = {}) {
  const [categories, setCategories] = useState<TagCategory[]>([]);
  const [tags, setTags] = useState<Record<string, Tag[]>>({});
  const [expandedCats, setExpandedCats] = useState<Set<string>>(new Set());
  const [selectedTagIds, setSelectedTagIds] = useState<Set<string>>(new Set());
  /**
   * The MyTag import, previewed before it runs.
   *
   * The spec has Rekordbox MyTags import *automatically*. Here it is
   * preview-then-apply like every other bulk operation: this merges a second
   * taxonomy into the user's own tag tree, and doing that unannounced is how a
   * tag list becomes unusable.
   */
  const [importPreview, setImportPreview] = useState<MyTagImportPreview | null>(
    null,
  );
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [importDone, setImportDone] = useState<string | null>(null);

  /**
   * Selected ids grouped by the category they came from.
   *
   * The spec's semantics for this page are **OR within a category, AND across
   * categories** — picking two genres and one energy means "either genre, and
   * that energy". A flat list cannot say that.
   */
  const tagGroups = useMemo(
    () =>
      categories
        .map((cat) =>
          (tags[cat.id] ?? [])
            .filter((t) => selectedTagIds.has(t.id))
            .map((t) => t.id),
        )
        .filter((g) => g.length > 0),
    [categories, tags, selectedTagIds],
  );

  const loadData = async () => {
    try {
      const cats = await listTagCategories();
      setCategories(cats);

      const allTags = await listTags();
      const tagMap: Record<string, Tag[]> = {};
      cats.forEach((c) => (tagMap[c.id] = []));
      allTags.forEach((t) => {
        if (!tagMap[t.category_id]) tagMap[t.category_id] = [];
        tagMap[t.category_id].push(t);
      });
      setTags(tagMap);
      // Prune selection of any tag IDs that no longer exist.
      setSelectedTagIds((prev) => {
        const live = new Set(allTags.map((t) => t.id));
        const pruned = new Set([...prev].filter((id) => live.has(id)));
        return pruned.size === prev.size ? prev : pruned;
      });
    } catch (e) {
      console.error("Failed to load tags", e);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleAddCategory = async () => {
    const name = prompt("Enter category name:");
    if (name) {
      await createTagCategory(name);
      await loadData();
    }
  };

  const handleAddTag = async (categoryId: string) => {
    const name = prompt("Enter tag name:");
    if (name) {
      await createTag(categoryId, name);
      setExpandedCats(new Set(expandedCats).add(categoryId));
      await loadData();
    }
  };

  const runPreview = async () => {
    if (!libraryPath) return;
    setImportError(null);
    setImportDone(null);
    try {
      setImportPreview(await previewMyTagImport(libraryPath));
    } catch (e) {
      setImportError(String(e));
    }
  };

  const runImport = async () => {
    if (!libraryPath) return;
    setImporting(true);
    setImportError(null);
    try {
      const result = await importMyTags(libraryPath);
      setImportPreview(null);
      setImportDone(
        result.categories_created + result.tags_created + result.links_created >
          0
          ? `Imported ${result.categories_created} category(ies), ${result.tags_created} tag(s) and ${result.links_created} track link(s).`
          : "Everything was already imported — nothing to do.",
      );
      await loadData();
    } catch (e) {
      setImportError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const toggleCat = (id: string) => {
    const next = new Set(expandedCats);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setExpandedCats(next);
  };

  /** Which category's colour menu is open, if any. */
  const [colourMenuFor, setColourMenuFor] = useState<string | null>(null);
  /** The tag id currently being dragged, so a drop knows what moved. */
  const [draggingTagId, setDraggingTagId] = useState<string | null>(null);

  const applyColour = async (categoryId: string, hex: string | null) => {
    setColourMenuFor(null);
    await setTagCategoryColor(categoryId, hex);
    await loadData();
  };

  const applyHotkey = async (tagId: string, value: string) => {
    // The hotkey is global, so the backend takes it from whichever tag held it.
    await setTagHotkey(tagId, value === "" ? null : Number(value));
    await loadData();
  };

  /**
   * Commit a drag as a whole new order for the category.
   *
   * Optimistic: the chips reorder before the write lands, because a list that
   * snaps back for a moment after every drop reads as a failed drag. `loadData`
   * afterwards is what makes a genuine failure visible.
   */
  const dropTag = async (categoryId: string, overTagId: string) => {
    const dragged = draggingTagId;
    setDraggingTagId(null);
    if (!dragged || dragged === overTagId) return;

    const current = tags[categoryId] ?? [];
    const next = moveWithin(
      current,
      indexOfId(current, dragged),
      indexOfId(current, overTagId),
    );
    if (next === current) return;

    setTags((prev) => ({ ...prev, [categoryId]: next }));
    await reorderTags(
      categoryId,
      next.map((t) => t.id),
    );
    await loadData();
  };

  /**
   * Keyboard reorder, because a drag-only list is unreachable without a mouse.
   *
   * `Alt` + arrow moves the focused chip one place. Plain arrows are left to
   * the browser so tabbing through chips still works.
   */
  const nudgeTag = async (
    categoryId: string,
    tagId: string,
    delta: number,
  ) => {
    const current = tags[categoryId] ?? [];
    const from = indexOfId(current, tagId);
    const next = moveWithin(current, from, from + delta);
    if (next === current) return;
    setTags((prev) => ({ ...prev, [categoryId]: next }));
    await reorderTags(
      categoryId,
      next.map((t) => t.id),
    );
    await loadData();
  };

  const toggleTagSelection = (tagId: string) => {
    setSelectedTagIds((prev) => {
      const next = new Set(prev);
      if (next.has(tagId)) next.delete(tagId);
      else next.add(tagId);
      return next;
    });
  };

  return (
    <div className="flex h-full flex-col bg-surface p-4 text-sm">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-ink">Custom Tags</h2>
        <button
          onClick={handleAddCategory}
          className="flex items-center gap-1 rounded bg-accent px-2 py-1 text-xs font-medium text-base hover:bg-accent-hover"
        >
          <PlusIcon className="h-3 w-3" />
          Category
        </button>
      </div>

      {libraryPath && (
        <section
          className="mb-3 rounded-md border border-edge bg-base p-2 text-xs"
          aria-label="Rekordbox MyTags"
        >
          <div className="flex items-center justify-between">
            <span className="text-ink-secondary">
              Rekordbox MyTags
            </span>
            <button
              type="button"
              onClick={runPreview}
              className="rounded border border-edge px-2 py-0.5 text-ink-secondary hover:border-edge-strong hover:text-ink"
            >
              Check for MyTags
            </button>
          </div>

          {importPreview && (
            <div className="mt-2" data-testid="mytag-preview">
              <p className="text-ink">
                {importPreview.new_categories.length} new category(ies),{" "}
                {importPreview.new_tags.length} new tag(s),{" "}
                {importPreview.new_links} new track link(s).
                {importPreview.existing_tags > 0 && (
                  <span className="text-ink-muted">
                    {" "}
                    {importPreview.existing_tags} tag(s) already here will be
                    reused.
                  </span>
                )}
              </p>
              {importPreview.unmatched_links > 0 && (
                <p className="mt-1 text-[11px] text-amber-500">
                  {importPreview.unmatched_links} link(s) point at tracks that
                  are not in this library, and will be skipped.
                </p>
              )}
              <button
                type="button"
                disabled={importing}
                onClick={runImport}
                className="mt-2 rounded bg-accent px-2 py-0.5 text-xs font-medium text-base hover:bg-accent-hover disabled:opacity-50"
              >
                {importing ? "Importing…" : "Import"}
              </button>
            </div>
          )}

          {importDone && (
            <p className="mt-2 text-ink-secondary" data-testid="mytag-done">
              {importDone}
            </p>
          )}
          {importError && (
            <p className="mt-2 text-red-400" data-testid="mytag-error">
              {importError}
            </p>
          )}
        </section>
      )}

      <div className="flex-1 overflow-y-auto">
        {categories.length === 0 ? (
          <div className="flex h-32 items-center justify-center text-ink-muted">
            No tag categories found.
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {categories.map((cat) => {
              const isExpanded = expandedCats.has(cat.id);
              const catTags = tags[cat.id] || [];
              return (
                <div
                  key={cat.id}
                  className="rounded-md border border-edge bg-base p-2"
                >
                  <div className="flex items-center justify-between">
                    <button
                      className="flex items-center gap-2 font-medium text-ink"
                      onClick={() => toggleCat(cat.id)}
                    >
                      {isExpanded ? (
                        <ChevronDownIcon className="h-4 w-4" />
                      ) : (
                        <ChevronRightIcon className="h-4 w-4" />
                      )}
                      <span
                        aria-hidden
                        className="h-3 w-1 rounded-full"
                        style={{ backgroundColor: cat.color ?? "transparent" }}
                      />
                      {cat.name}
                    </button>
                    <div className="flex items-center gap-2">
                      <div className="relative">
                        <button
                          type="button"
                          onClick={() =>
                            setColourMenuFor((c) => (c === cat.id ? null : cat.id))
                          }
                          aria-label={`Colour for ${cat.name}`}
                          title="Category colour"
                          className="h-4 w-4 rounded-full border border-edge-strong"
                          style={
                            cat.color ? { backgroundColor: cat.color } : undefined
                          }
                        />
                        {colourMenuFor === cat.id && (
                          <div
                            data-testid={`colour-menu-${cat.id}`}
                            className="absolute right-0 z-10 mt-1 flex w-40 flex-wrap gap-1 rounded border border-edge bg-elevated p-2 shadow"
                          >
                            {CATEGORY_COLOURS.map((c) => (
                              <button
                                key={c.hex}
                                type="button"
                                aria-label={c.name}
                                title={c.name}
                                onClick={() => applyColour(cat.id, c.hex)}
                                className="h-5 w-5 rounded-full border border-edge-strong"
                                style={{ backgroundColor: c.hex }}
                              />
                            ))}
                            {/* No colour is a real end state, not a failure to
                                choose — most categories will never have one. */}
                            <button
                              type="button"
                              onClick={() => applyColour(cat.id, null)}
                              className="mt-1 w-full rounded px-1 py-0.5 text-[11px] text-ink-muted hover:text-ink"
                            >
                              No colour
                            </button>
                          </div>
                        )}
                      </div>
                      <button
                        onClick={() => handleAddTag(cat.id)}
                        className="text-ink-muted hover:text-accent"
                        title="Add tag"
                      >
                        <PlusIcon className="h-3 w-3" />
                      </button>
                    </div>
                  </div>

                  {isExpanded && (
                    <div className="mt-2 pl-6">
                      {catTags.length === 0 ? (
                        <div className="text-xs text-ink-faint">No tags.</div>
                      ) : (
                        <div
                          className="flex flex-wrap gap-2"
                          data-testid={`tags-${cat.id}`}
                        >
                          {catTags.map((tag) => {
                            const selected = selectedTagIds.has(tag.id);
                            return (
                              <div
                                key={tag.id}
                                draggable
                                onDragStart={() => setDraggingTagId(tag.id)}
                                onDragEnd={() => setDraggingTagId(null)}
                                onDragOver={(e) => e.preventDefault()}
                                onDrop={() => dropTag(cat.id, tag.id)}
                                className={[
                                  "flex items-center gap-1 rounded border px-2 py-1 text-xs",
                                  selected
                                    ? "border-accent bg-accent/10 text-accent-hover"
                                    : "border-edge bg-elevated text-ink hover:border-edge-strong",
                                  draggingTagId === tag.id ? "opacity-50" : "",
                                ].join(" ")}
                              >
                                <button
                                  type="button"
                                  onClick={() => toggleTagSelection(tag.id)}
                                  // Drag is a mouse gesture, so the same move
                                  // has to exist on the keyboard or the chip
                                  // order is unreachable without one.
                                  onKeyDown={(e) => {
                                    if (!e.altKey) return;
                                    if (e.key === "ArrowRight") {
                                      e.preventDefault();
                                      void nudgeTag(cat.id, tag.id, 1);
                                    } else if (e.key === "ArrowLeft") {
                                      e.preventDefault();
                                      void nudgeTag(cat.id, tag.id, -1);
                                    }
                                  }}
                                  className="flex items-center gap-1"
                                >
                                  <span>{tag.name}</span>
                                  {tag.usage_count > 0 && (
                                    <span className="text-[10px] text-ink-muted">
                                      ({tag.usage_count})
                                    </span>
                                  )}
                                </button>
                                <select
                                  aria-label={`Hotkey for ${tag.name}`}
                                  title="Number-row hotkey"
                                  value={tag.hotkey ?? ""}
                                  onChange={(e) => applyHotkey(tag.id, e.target.value)}
                                  className="ml-1 rounded border border-edge bg-base px-0.5 text-[10px] text-ink-muted"
                                >
                                  <option value="">–</option>
                                  {[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => (
                                    <option key={n} value={n}>
                                      {n}
                                    </option>
                                  ))}
                                </select>
                                <button
                                  type="button"
                                  onClick={async () => {
                                    if (confirm(`Delete tag ${tag.name}?`)) {
                                      await deleteTag(tag.id);
                                      await loadData();
                                    }
                                  }}
                                  className="ml-1 cursor-pointer text-ink-faint hover:text-red-500"
                                  aria-label={`Delete ${tag.name}`}
                                >
                                  &times;
                                </button>
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {onShowTracks && selectedTagIds.size > 0 && (
        <div className="mt-4 flex shrink-0 items-center justify-between border-t border-edge pt-3">
          <span className="text-xs text-ink-muted">
            {selectedTagIds.size} tag
            {selectedTagIds.size === 1 ? "" : "s"} selected
            {tagGroups.length > 1 && (
              <span className="ml-1 text-[11px] opacity-70" data-testid="tag-selection-rule">
                — any within a category, all across
              </span>
            )}
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setSelectedTagIds(new Set())}
              className="rounded border border-edge px-2 py-1 text-xs text-ink-secondary hover:border-edge-strong hover:text-ink"
            >
              Clear
            </button>
            <button
              type="button"
              onClick={() => onShowTracks([...selectedTagIds], tagGroups)}
              className="rounded bg-accent px-2 py-1 text-xs font-medium text-base hover:bg-accent-hover"
            >
              Show {selectedTagIds.size} tag
              {selectedTagIds.size === 1 ? "" : "s"} in library
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
