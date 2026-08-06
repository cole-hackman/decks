import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyPlaylistMerge,
  applyPlaylistPrefix,
  applyPlaylistSort,
  applyRewriteOrder,
  listPlaylists,
  previewCrossReference,
  previewPlaylistMerge,
  previewPlaylistPrefix,
  previewPlaylistSort,
  previewRewriteOrder,
  playlistOccurrence,
  getPlaylist,
} from "../ipc";
import { useToast } from "./Toast";
import type {
  CrossReferenceMode,
  CrossReferencePreview,
  MergePreview,
  OccurrenceReport,
  Playlist,
  PlaylistRenamePlan,
  PlaylistSortMode,
  RewriteOrderPlan,
  SortPreview,
  Track,
} from "../types";

interface Props {
  libraryPath: string;
}

type Tool =
  | "merge"
  | "sort"
  | "cross-reference"
  | "prefix"
  | "rewrite-order"
  | "occurrence";

const TOOLS: { id: Tool; label: string; blurb: string }[] = [
  {
    id: "merge",
    label: "Merge",
    blurb:
      "Combine playlists into one new playlist, duplicates dropped. The sources are left alone.",
  },
  {
    id: "sort",
    label: "Sort",
    blurb:
      "Order the playlists themselves inside a folder — not the tracks inside them.",
  },
  {
    id: "cross-reference",
    label: "Cross Reference",
    blurb:
      "Tracks common to every selected playlist, or library tracks in none of them.",
  },
  {
    id: "prefix",
    label: "Prefix",
    blurb:
      "Prepend text, or an incrementing number. Numbering follows the order you tick them.",
  },
  {
    id: "rewrite-order",
    label: "Rewrite Order",
    blurb:
      "Persist a sort as the playlist's stored order, so it reaches the CDJ that way.",
  },
  {
    id: "occurrence",
    label: "Occurrence",
    blurb:
      "Which tracks appear in exactly N playlists? N = 0 finds the orphans. A report — nothing is staged.",
  },
];

/** Fields the Rewrite Order sort can use. Energy is the point of the exercise:
 *  no CDJ can sort by it, which is the whole reason this tool exists. */
const SORT_FIELDS = [
  { value: "energy", label: "Energy" },
  { value: "bpm", label: "BPM" },
  { value: "musical_key", label: "Key" },
  { value: "rating", label: "Rating" },
  { value: "title", label: "Title" },
  { value: "artist", label: "Artist" },
] as const;

type SortField = (typeof SORT_FIELDS)[number]["value"];

/**
 * Compare two tracks on `field`, honouring `desc`.
 *
 * The direction is handled **inside** rather than by negating the result,
 * because tracks with no value sort last in *either* direction. Negating the
 * whole comparison flips that too, and an un-analysed track leads the set
 * purely because null happened to compare low.
 */
function compareTracks(
  a: Track,
  b: Track,
  field: SortField,
  desc: boolean,
): number {
  const pick = (t: Track): string | number | null => {
    const v = t[field as keyof Track];
    return typeof v === "string" || typeof v === "number" ? v : null;
  };
  const av = pick(a);
  const bv = pick(b);
  if (av == null && bv == null) return 0;
  if (av == null) return 1;
  if (bv == null) return -1;

  const cmp =
    typeof av === "number" && typeof bv === "number"
      ? av - bv
      : String(av).localeCompare(String(bv), undefined, {
          sensitivity: "base",
        });
  return desc ? -cmp : cmp;
}

/**
 * Playlist Tools — Merge, Sort, Cross Reference, Prefix, Rewrite Order.
 *
 * Per `docs/lexicon/02-library.md §Playlist Tools`. All five preview before
 * they do anything, and everything they do is a staged change that goes through
 * review and Sync.
 *
 * **Rewrite Order is the one that earns its keep.** It has no visible effect
 * here, exactly as it has none inside Lexicon — its purpose is that a CDJ can
 * only sort by a handful of columns and knows nothing about Energy. Sort by
 * Energy, rewrite the order, and the playlist arrives on the gear that way.
 *
 * **Divergence:** Lexicon rewrites "the current visible sort" of the browser.
 * `decks` sorts here instead, on a field you pick. The browser's column sort is
 * transient UI state that is not plumbed to this view, and a button that
 * silently depends on which column you last clicked somewhere else is worse
 * than one that states its input.
 */
export function PlaylistToolsView({ libraryPath }: Props) {
  const { toast } = useToast();
  const [tool, setTool] = useState<Tool>("merge");
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  /** Ticked ids **in tick order** — Merge concatenates in it and Prefix numbers
   *  in it, so a Set would lose the thing that matters. */
  const [picked, setPicked] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);

  const [mergeName, setMergeName] = useState("");
  const [mergePreview, setMergePreview] = useState<MergePreview | null>(null);

  const [sortParent, setSortParent] = useState<string>("");
  const [sortMode, setSortMode] = useState<PlaylistSortMode>("name_asc");
  const [sortPreview, setSortPreview] = useState<SortPreview | null>(null);

  const [xrefMode, setXrefMode] = useState<CrossReferenceMode>("in_all");
  const [xref, setXref] = useState<CrossReferencePreview | null>(null);

  const [prefixText, setPrefixText] = useState("");
  const [numbered, setNumbered] = useState(false);
  const [numberStart, setNumberStart] = useState(1);
  const [numberPad, setNumberPad] = useState(2);
  const [replaceExisting, setReplaceExisting] = useState(true);
  const [renames, setRenames] = useState<PlaylistRenamePlan[] | null>(null);

  const [rewriteField, setRewriteField] = useState<SortField>("energy");
  const [rewriteDesc, setRewriteDesc] = useState(false);
  const [rewritePlan, setRewritePlan] = useState<RewriteOrderPlan | null>(null);
  const [rewriteNames, setRewriteNames] = useState<string[]>([]);

  const [occurrenceN, setOccurrenceN] = useState(0);
  const [occurrence, setOccurrence] = useState<OccurrenceReport | null>(null);

  useEffect(() => {
    listPlaylists(libraryPath)
      .then((got) => setPlaylists(Array.isArray(got) ? got : []))
      .catch((e: unknown) => toast({ variant: "error", message: String(e) }));
  }, [libraryPath, toast]);

  const leaves = useMemo(
    () => playlists.filter((p) => p.kind !== "Folder"),
    [playlists],
  );
  const folders = useMemo(
    () => playlists.filter((p) => p.kind === "Folder"),
    [playlists],
  );

  /** Clearing every preview whenever the selection changes: a preview that
   *  outlives the input it was computed from is a wrong answer that looks
   *  right. */
  const resetPreviews = useCallback(() => {
    setMergePreview(null);
    setXref(null);
    setRenames(null);
    setRewritePlan(null);
    setOccurrence(null);
  }, []);

  const toggle = useCallback(
    (id: string) => {
      setPicked((prev) =>
        prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id],
      );
      resetPreviews();
    },
    [resetPreviews],
  );

  const run = useCallback(
    async (fn: () => Promise<void>) => {
      setBusy(true);
      try {
        await fn();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [toast],
  );

  const doRewritePreview = useCallback(
    async (playlistId: string) => {
      const detail = await getPlaylist(libraryPath, playlistId);
      if (detail == null) throw new Error(`Playlist not found: ${playlistId}`);
      const sorted = [...detail.tracks].sort((a, b) =>
        compareTracks(a, b, rewriteField, rewriteDesc),
      );
      setRewriteNames(sorted.map((t) => t.title));
      setRewritePlan(
        await previewRewriteOrder(
          libraryPath,
          playlistId,
          sorted.map((t) => t.id),
        ),
      );
    },
    [libraryPath, rewriteField, rewriteDesc],
  );

  const active = TOOLS.find((t) => t.id === tool)!;
  const pickedNames = useMemo(
    () =>
      picked.map((id) => playlists.find((p) => p.id === id)?.name ?? id),
    [picked, playlists],
  );

  return (
    <div className="flex h-full flex-col overflow-hidden" aria-label="Playlist tools">
      <div className="border-b border-border px-4 py-3">
        <h2 className="text-sm font-semibold">Playlist Tools</h2>
        <div className="mt-2 flex flex-wrap gap-1">
          {TOOLS.map((t) => (
            <button
              key={t.id}
              type="button"
              className={`rounded border px-2 py-1 text-xs ${
                tool === t.id
                  ? "border-accent bg-accent/10 text-accent"
                  : "border-border hover:bg-surface-hover"
              }`}
              onClick={() => {
                setTool(t.id);
                resetPreviews();
              }}
            >
              {t.label}
            </button>
          ))}
        </div>
        <p className="mt-2 text-[11px] text-muted" data-testid="tool-blurb">
          {active.blurb}
        </p>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {tool !== "sort" && tool !== "occurrence" && (
          <div className="w-64 shrink-0 overflow-auto border-r border-border p-2 text-xs">
            <div className="mb-1 flex items-center justify-between">
              <span className="text-muted">
                {tool === "rewrite-order" ? "Playlist" : "Playlists"}
              </span>
              {picked.length > 0 && (
                <button
                  type="button"
                  className="text-[11px] underline"
                  onClick={() => {
                    setPicked([]);
                    resetPreviews();
                  }}
                >
                  Clear
                </button>
              )}
            </div>
            <ul data-testid="playlist-picker">
              {leaves.map((p) => {
                const index = picked.indexOf(p.id);
                return (
                  <li key={p.id}>
                    <label className="flex items-center gap-2 rounded px-1 py-0.5 hover:bg-surface-hover">
                      <input
                        type={tool === "rewrite-order" ? "radio" : "checkbox"}
                        name={tool === "rewrite-order" ? "rewrite-target" : undefined}
                        aria-label={p.name}
                        checked={index !== -1}
                        onChange={() => {
                          if (tool === "rewrite-order") {
                            setPicked([p.id]);
                            resetPreviews();
                          } else {
                            toggle(p.id);
                          }
                        }}
                      />
                      <span className="truncate">{p.name}</span>
                      {index !== -1 && tool === "prefix" && (
                        <span className="ml-auto shrink-0 tabular-nums text-muted">
                          {index + 1}
                        </span>
                      )}
                    </label>
                  </li>
                );
              })}
            </ul>
          </div>
        )}

        <div className="flex-1 overflow-auto p-4 text-xs">
          {/* ── Merge ───────────────────────────────────────────────── */}
          {tool === "merge" && (
            <div className="space-y-2">
              <label className="block">
                <span className="mb-1 block text-muted">New playlist name</span>
                <input
                  aria-label="Merged playlist name"
                  className="w-full max-w-sm rounded border border-border bg-surface px-2 py-1 text-xs"
                  value={mergeName}
                  onChange={(e) => setMergeName(e.target.value)}
                />
              </label>
              <div className="flex gap-2">
                <button
                  type="button"
                  disabled={busy || picked.length < 2}
                  className="rounded border border-border px-3 py-1 disabled:opacity-50"
                  onClick={() =>
                    void run(async () =>
                      setMergePreview(
                        await previewPlaylistMerge(libraryPath, picked),
                      ),
                    )
                  }
                >
                  Preview merge
                </button>
                <button
                  type="button"
                  disabled={
                    busy || mergePreview == null || mergeName.trim() === ""
                  }
                  className="rounded bg-accent px-3 py-1 text-white disabled:opacity-50"
                  onClick={() =>
                    void run(async () => {
                      const ids = await applyPlaylistMerge(
                        libraryPath,
                        mergeName,
                        null,
                        mergePreview!.track_ids,
                      );
                      toast({
                        variant: "success",
                        message: `Staged “${mergeName}” with ${mergePreview!.track_ids.length} track(s).`,
                      });
                      setMergePreview(null);
                      setPicked([]);
                      void ids;
                    })
                  }
                >
                  Stage playlist
                </button>
              </div>
              {picked.length < 2 && (
                <p className="text-muted">Tick at least two playlists.</p>
              )}
              {mergePreview != null && (
                <p data-testid="merge-preview">
                  {mergePreview.track_ids.length} track(s) from{" "}
                  {mergePreview.source_rows} row(s) —{" "}
                  {mergePreview.source_rows - mergePreview.track_ids.length}{" "}
                  duplicate(s) dropped. Sources are left alone.
                </p>
              )}
            </div>
          )}

          {/* ── Sort ────────────────────────────────────────────────── */}
          {tool === "sort" && (
            <div className="space-y-2">
              <div className="flex flex-wrap items-end gap-2">
                <label>
                  <span className="mb-1 block text-muted">Folder</span>
                  <select
                    aria-label="Folder to sort"
                    className="rounded border border-border bg-surface px-2 py-1 text-xs"
                    value={sortParent}
                    onChange={(e) => {
                      setSortParent(e.target.value);
                      setSortPreview(null);
                    }}
                  >
                    <option value="">Root level</option>
                    {folders.map((f) => (
                      <option key={f.id} value={f.id}>
                        {f.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span className="mb-1 block text-muted">Order</span>
                  <select
                    aria-label="Sort order"
                    className="rounded border border-border bg-surface px-2 py-1 text-xs"
                    value={sortMode}
                    onChange={(e) => {
                      setSortMode(e.target.value as PlaylistSortMode);
                      setSortPreview(null);
                    }}
                  >
                    <option value="name_asc">Name A–Z</option>
                    <option value="name_desc">Name Z–A</option>
                    <option value="track_count_desc">Most tracks first</option>
                  </select>
                </label>
                <button
                  type="button"
                  disabled={busy}
                  className="rounded border border-border px-3 py-1 disabled:opacity-50"
                  onClick={() =>
                    void run(async () =>
                      setSortPreview(
                        await previewPlaylistSort(
                          libraryPath,
                          sortParent === "" ? null : sortParent,
                          sortMode,
                        ),
                      ),
                    )
                  }
                >
                  Preview sort
                </button>
                <button
                  type="button"
                  disabled={busy || sortPreview == null || sortPreview.unchanged}
                  className="rounded bg-accent px-3 py-1 text-white disabled:opacity-50"
                  onClick={() =>
                    void run(async () => {
                      await applyPlaylistSort(
                        libraryPath,
                        sortParent === "" ? null : sortParent,
                        sortPreview!.order.map(([id]) => id),
                      );
                      toast({
                        variant: "success",
                        message: "Staged the new playlist order for review.",
                      });
                      setSortPreview(null);
                    })
                  }
                >
                  Stage order
                </button>
              </div>
              {sortPreview != null &&
                (sortPreview.unchanged ? (
                  <p data-testid="sort-preview">
                    Already in that order — nothing to stage.
                  </p>
                ) : (
                  <ol
                    className="list-decimal space-y-0.5 pl-5"
                    data-testid="sort-preview"
                  >
                    {sortPreview.order.map(([id, name]) => (
                      <li key={id}>{name}</li>
                    ))}
                  </ol>
                ))}
            </div>
          )}

          {/* ── Cross Reference ─────────────────────────────────────── */}
          {tool === "cross-reference" && (
            <div className="space-y-2">
              <label className="flex items-center gap-2">
                <span className="text-muted">Show</span>
                <select
                  aria-label="Cross reference mode"
                  className="rounded border border-border bg-surface px-2 py-1 text-xs"
                  value={xrefMode}
                  onChange={(e) => {
                    setXrefMode(e.target.value as CrossReferenceMode);
                    setXref(null);
                  }}
                >
                  <option value="in_all">Tracks in every selected playlist</option>
                  <option value="in_none">Library tracks in none of them</option>
                </select>
              </label>
              {xrefMode === "in_none" && (
                <p className="text-amber-500" data-testid="xref-warning">
                  This can return most of the library. It is a report — nothing
                  is staged either way.
                </p>
              )}
              <button
                type="button"
                disabled={busy || picked.length === 0}
                className="rounded border border-border px-3 py-1 disabled:opacity-50"
                onClick={() =>
                  void run(async () =>
                    setXref(
                      await previewCrossReference(libraryPath, picked, xrefMode),
                    ),
                  )
                }
              >
                Run cross reference
              </button>
              {picked.length === 0 && (
                <p className="text-muted">Tick at least one playlist.</p>
              )}
              {xref != null && (
                <p data-testid="xref-result">
                  {xref.track_ids.length} of {xref.considered} track(s) match
                  across {pickedNames.length} playlist(s).
                </p>
              )}
            </div>
          )}

          {/* ── Prefix ──────────────────────────────────────────────── */}
          {tool === "prefix" && (
            <div className="space-y-2">
              <label className="block">
                <span className="mb-1 block text-muted">
                  Text (carries its own separator)
                </span>
                <input
                  aria-label="Prefix text"
                  className="w-full max-w-sm rounded border border-border bg-surface px-2 py-1 text-xs"
                  placeholder=" - "
                  value={prefixText}
                  onChange={(e) => {
                    setPrefixText(e.target.value);
                    setRenames(null);
                  }}
                />
              </label>
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  aria-label="Number them"
                  checked={numbered}
                  onChange={(e) => {
                    setNumbered(e.target.checked);
                    setRenames(null);
                  }}
                />
                Number them, in the order ticked
              </label>
              {numbered && (
                <div className="flex flex-wrap items-end gap-2" data-testid="numbering">
                  <label>
                    <span className="mb-1 block text-muted">Start at</span>
                    <input
                      type="number"
                      min={0}
                      aria-label="Start number"
                      className="w-20 rounded border border-border bg-surface px-2 py-1 text-xs"
                      value={numberStart}
                      onChange={(e) => {
                        setNumberStart(Number(e.target.value));
                        setRenames(null);
                      }}
                    />
                  </label>
                  <label>
                    <span className="mb-1 block text-muted">Pad to</span>
                    <input
                      type="number"
                      min={0}
                      aria-label="Zero pad width"
                      className="w-20 rounded border border-border bg-surface px-2 py-1 text-xs"
                      value={numberPad}
                      onChange={(e) => {
                        setNumberPad(Number(e.target.value));
                        setRenames(null);
                      }}
                    />
                  </label>
                  <label className="flex items-center gap-1">
                    <input
                      type="checkbox"
                      aria-label="Replace an existing number"
                      checked={replaceExisting}
                      onChange={(e) => {
                        setReplaceExisting(e.target.checked);
                        setRenames(null);
                      }}
                    />
                    Replace an existing number
                  </label>
                </div>
              )}
              <div className="flex gap-2">
                <button
                  type="button"
                  disabled={busy || picked.length === 0}
                  className="rounded border border-border px-3 py-1 disabled:opacity-50"
                  onClick={() =>
                    void run(async () =>
                      setRenames(
                        await previewPlaylistPrefix(libraryPath, picked, {
                          text: prefixText,
                          numbering: numbered
                            ? {
                                start: numberStart,
                                pad: numberPad,
                                replace_existing: replaceExisting,
                              }
                            : null,
                        }),
                      ),
                    )
                  }
                >
                  Preview names
                </button>
                <button
                  type="button"
                  disabled={busy || renames == null || renames.length === 0}
                  className="rounded bg-accent px-3 py-1 text-white disabled:opacity-50"
                  onClick={() =>
                    void run(async () => {
                      const ids = await applyPlaylistPrefix(libraryPath, renames!);
                      toast({
                        variant: "success",
                        message: `Staged ${ids.length} rename(s) for review.`,
                      });
                      setRenames(null);
                    })
                  }
                >
                  Stage {renames?.length ?? 0} rename(s)
                </button>
              </div>
              {renames != null &&
                (renames.length === 0 ? (
                  <p data-testid="prefix-preview">
                    Every name is already what it would become — nothing to
                    stage.
                  </p>
                ) : (
                  <ul className="space-y-0.5" data-testid="prefix-preview">
                    {renames.map((r) => (
                      <li key={r.id}>
                        <span className="text-muted line-through">{r.from}</span>{" "}
                        → {r.to}
                      </li>
                    ))}
                  </ul>
                ))}
            </div>
          )}

          {/* ── Occurrence ──────────────────────────────────────────── */}
          {tool === "occurrence" && (
            <div className="space-y-2">
              <div className="flex flex-wrap items-end gap-2">
                <label>
                  <span className="mb-1 block text-muted">
                    In exactly how many playlists
                  </span>
                  <input
                    type="number"
                    min={0}
                    aria-label="Playlist count"
                    className="w-24 rounded border border-border bg-surface px-2 py-1 text-xs"
                    value={occurrenceN}
                    onChange={(e) => {
                      setOccurrenceN(Math.max(0, Number(e.target.value)));
                      setOccurrence(null);
                    }}
                  />
                </label>
                <button
                  type="button"
                  disabled={busy}
                  className="rounded border border-border px-3 py-1 disabled:opacity-50"
                  onClick={() =>
                    void run(async () =>
                      setOccurrence(
                        await playlistOccurrence(libraryPath, occurrenceN),
                      ),
                    )
                  }
                >
                  Find tracks
                </button>
              </div>
              {occurrence != null && (
                <div data-testid="occurrence-result">
                  <p className="mb-1">
                    {occurrence.tracks.length} track(s) are in exactly{" "}
                    {occurrenceN} playlist(s).
                  </p>
                  {/* The distribution is why there is no guessing: it answers
                      "what is a useful N?" before the number box is touched. */}
                  <table className="mb-2 text-[11px]" data-testid="occurrence-distribution">
                    <thead>
                      <tr className="text-muted">
                        <th className="pr-3 text-left font-normal">Playlists</th>
                        <th className="text-left font-normal">Tracks</th>
                      </tr>
                    </thead>
                    <tbody>
                      {occurrence.distribution.map(([count, tracks]) => (
                        <tr key={count}>
                          <td className="pr-3 tabular-nums">
                            <button
                              type="button"
                              className="underline"
                              onClick={() => {
                                setOccurrenceN(count);
                                void run(async () =>
                                  setOccurrence(
                                    await playlistOccurrence(libraryPath, count),
                                  ),
                                );
                              }}
                            >
                              {count}
                            </button>
                          </td>
                          <td className="tabular-nums">{tracks}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  <ul className="space-y-0.5">
                    {occurrence.tracks.slice(0, 200).map((t) => (
                      <li key={t.id} className="truncate">
                        {t.title}
                        <span className="text-muted">
                          {" "}
                          — {t.artist ?? "Unknown artist"}
                        </span>
                      </li>
                    ))}
                  </ul>
                  {occurrence.tracks.length > 200 && (
                    <p className="mt-1 text-muted" data-testid="occurrence-truncated">
                      Showing the first 200 of {occurrence.tracks.length}.
                    </p>
                  )}
                </div>
              )}
            </div>
          )}

          {/* ── Rewrite Order ───────────────────────────────────────── */}
          {tool === "rewrite-order" && (
            <div className="space-y-2">
              <p className="text-muted">
                A CDJ can only sort by a handful of columns and knows nothing
                about Energy. Sort here, rewrite the order, and the playlist
                reaches the gear that way.
              </p>
              <div className="flex flex-wrap items-end gap-2">
                <label>
                  <span className="mb-1 block text-muted">Sort by</span>
                  <select
                    aria-label="Sort field"
                    className="rounded border border-border bg-surface px-2 py-1 text-xs"
                    value={rewriteField}
                    onChange={(e) => {
                      setRewriteField(e.target.value as SortField);
                      setRewritePlan(null);
                    }}
                  >
                    {SORT_FIELDS.map((f) => (
                      <option key={f.value} value={f.value}>
                        {f.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex items-center gap-1">
                  <input
                    type="checkbox"
                    aria-label="Descending"
                    checked={rewriteDesc}
                    onChange={(e) => {
                      setRewriteDesc(e.target.checked);
                      setRewritePlan(null);
                    }}
                  />
                  Descending
                </label>
                <button
                  type="button"
                  disabled={busy || picked.length !== 1}
                  className="rounded border border-border px-3 py-1 disabled:opacity-50"
                  onClick={() => void run(() => doRewritePreview(picked[0]))}
                >
                  Preview order
                </button>
                <button
                  type="button"
                  disabled={
                    busy || rewritePlan == null || rewritePlan.unchanged
                  }
                  className="rounded bg-accent px-3 py-1 text-white disabled:opacity-50"
                  onClick={() =>
                    void run(async () => {
                      const id = await applyRewriteOrder(
                        libraryPath,
                        rewritePlan!,
                      );
                      toast({
                        variant: id == null ? "info" : "success",
                        message:
                          id == null
                            ? "Already in that order — nothing staged."
                            : "Staged the new track order for review.",
                      });
                      setRewritePlan(null);
                    })
                  }
                >
                  Stage order
                </button>
              </div>
              {picked.length !== 1 && (
                <p className="text-muted">Pick one playlist.</p>
              )}
              {rewritePlan != null && (
                <div data-testid="rewrite-order-preview">
                  {rewritePlan.unchanged ? (
                    <p>Already in that order — nothing to stage.</p>
                  ) : (
                    <ol className="list-decimal space-y-0.5 pl-5">
                      {rewriteNames.slice(0, 50).map((name, i) => (
                        <li key={`${name}-${i}`}>{name}</li>
                      ))}
                    </ol>
                  )}
                  {rewritePlan.appended.length > 0 && (
                    <p className="mt-1 text-amber-500" data-testid="rewrite-appended">
                      {rewritePlan.appended.length} track(s) were not in the
                      sorted view and were appended rather than dropped.
                    </p>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
