import { useEffect, useMemo, useState } from "react";
import {
  listTagCategories,
  createTagCategory,
  listTags,
  createTag,
  deleteTag,
} from "../ipc";
import type { TagCategory, Tag } from "../types";
import { PlusIcon, ChevronDownIcon, ChevronRightIcon } from "lucide-react";

interface Props {
  /** Optional — when provided, the panel renders a "Show tracks" button that
   *  hands the selected tag IDs back to the parent (which typically updates
   *  the library filter and switches view). */
  onShowTracks?: (tagIds: string[], tagGroups: string[][]) => void;
}

export function CustomTagsPanel({ onShowTracks }: Props = {}) {
  const [categories, setCategories] = useState<TagCategory[]>([]);
  const [tags, setTags] = useState<Record<string, Tag[]>>({});
  const [expandedCats, setExpandedCats] = useState<Set<string>>(new Set());
  const [selectedTagIds, setSelectedTagIds] = useState<Set<string>>(new Set());

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

  const toggleCat = (id: string) => {
    const next = new Set(expandedCats);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setExpandedCats(next);
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
                      {cat.name}
                    </button>
                    <div className="flex gap-2">
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
                        // NOTE: drag-to-move tag chips between categories (and
                        // within-category reorder) is deferred — the backend
                        // `move_tag` IPC exists, but reorder still needs a new
                        // `reorder_tags` command before we wire @dnd-kit.
                        <div className="flex flex-wrap gap-2">
                          {catTags.map((tag) => {
                            const selected = selectedTagIds.has(tag.id);
                            return (
                              <button
                                key={tag.id}
                                type="button"
                                onClick={() => toggleTagSelection(tag.id)}
                                className={[
                                  "flex items-center gap-1 rounded border px-2 py-1 text-xs",
                                  selected
                                    ? "border-accent bg-accent/10 text-accent-hover"
                                    : "border-edge bg-elevated text-ink hover:border-edge-strong",
                                ].join(" ")}
                              >
                                <span>{tag.name}</span>
                                {tag.usage_count > 0 && (
                                  <span className="text-[10px] text-ink-muted">
                                    ({tag.usage_count})
                                  </span>
                                )}
                                <span
                                  role="button"
                                  tabIndex={-1}
                                  onClick={async (e) => {
                                    e.stopPropagation();
                                    if (confirm(`Delete tag ${tag.name}?`)) {
                                      await deleteTag(tag.id);
                                      await loadData();
                                    }
                                  }}
                                  className="ml-1 cursor-pointer text-ink-faint hover:text-red-500"
                                  aria-label={`Delete ${tag.name}`}
                                >
                                  &times;
                                </span>
                              </button>
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
