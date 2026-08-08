import { useCallback, useEffect, useMemo, useState } from "react";
import {
  deleteMixableTemplate,
  findMixableTracks,
  getKeyMixingMode,
  listMixableTemplates,
  mixableDefaultOptions,
  saveMixableTemplate,
  setKeyMixingMode,
} from "../ipc";
import { useToast } from "./Toast";
import type {
  KeyMixingMode,
  MixableOptions,
  MixableResult,
  MixableTemplate,
  NumericRule,
  Track,
} from "../types";

interface Props {
  libraryPath: string;
  /** The track to mix out of. `null` while nothing is selected. */
  track: Track | null;
  /** Re-seeds the panel from a result row — the spec's `Use as next track`. */
  onUseAsNextTrack: (track: Track) => void;
  onClose: () => void;
}

function numericLabel(rule: NumericRule): string {
  switch (rule.kind) {
    case "off":
      return "Any";
    case "near_source":
      return "Within 1";
    case "range":
      return `${rule.min}–${rule.max}`;
  }
}

/**
 * Mixable Tracks — "pick a track, get a ranked list of tracks that mix with it".
 *
 * Per `docs/lexicon/04-analysis.md §Mixable Tracks`. Two tiers, as in the spec:
 * the panel opens in **basic mode** (BPM and key), and Advanced reveals the
 * rest of the rule set.
 *
 * The rules **filter**; the score only orders what survives. That is why the
 * header reports "12 of 4,213" — a rule that quietly demoted rather than
 * excluded would make "must have cue points" a suggestion.
 *
 * Three of the spec's options are absent, and not for want of a column.
 * Popularity, Danceability and Happiness come from Spotify's `audio-features`
 * endpoint in Lexicon; it was deprecated in November 2024 and 403s for
 * applications registered since, and Popularity is a catalog metric that cannot
 * be computed locally at all (ADR-0012). They are missing rather than
 * present-and-inert.
 */
export function MixableTracksPanel({
  libraryPath,
  track,
  onUseAsNextTrack,
  onClose,
}: Props) {
  const { toast } = useToast();
  // `null` until the backend has served basic mode. Nothing searches before
  // then — the alternative is a duplicate default in TypeScript that drifts
  // silently the first time `MixableOptions::basic()` changes.
  const [options, setOptions] = useState<MixableOptions | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [result, setResult] = useState<MixableResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [templates, setTemplates] = useState<MixableTemplate[]>([]);
  const [templateName, setTemplateName] = useState("");
  const [mode, setMode] = useState<KeyMixingMode>("harmonically_compatible");

  useEffect(() => {
    mixableDefaultOptions()
      .then(setOptions)
      .catch((e: unknown) =>
        setError(e instanceof Error ? e.message : String(e)),
      );
    getKeyMixingMode()
      .then(setMode)
      .catch(() => {});
    listMixableTemplates()
      .then((got) => setTemplates(Array.isArray(got) ? got : []))
      .catch(() => {});
  }, []);

  // Re-run whenever the seed track or the rules change. Driving a set live
  // means the list has to keep up with the track that just went on.
  useEffect(() => {
    if (!track || !libraryPath || !options) {
      setResult(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    findMixableTracks(libraryPath, track.id, options)
      .then((got) => {
        if (!cancelled) setResult(got);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [libraryPath, track, options]);

  /** Patch the rule set. A no-op before the defaults have loaded. */
  const update = useCallback((patch: Partial<MixableOptions>) => {
    setOptions((o) => (o ? { ...o, ...patch } : o));
  }, []);

  const changeMode = useCallback(
    async (next: KeyMixingMode) => {
      setMode(next);
      try {
        await setKeyMixingMode(next);
        // The backend overrides whatever the options carry with the stored
        // mode, so this only exists to retrigger the search.
        update({ key_mixing_mode: next });
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [toast, update],
  );

  const saveTemplate = useCallback(async () => {
    const name = templateName.trim();
    if (name === "" || !options) return;
    try {
      await saveMixableTemplate(name, options);
      setTemplates(await listMixableTemplates());
      setTemplateName("");
      toast({ variant: "success", message: `Saved “${name}”.` });
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  }, [options, templateName, toast]);

  const removeTemplate = useCallback(
    async (id: string) => {
      try {
        await deleteMixableTemplate(id);
        setTemplates(await listMixableTemplates());
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [toast],
  );

  const genreText = useMemo(
    () => (options ? options.genres.join(", ") : ""),
    [options],
  );

  if (!track) {
    return (
      <div className="flex h-full flex-col p-4 text-xs text-muted">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-xs font-semibold uppercase tracking-wide">
            Mixable Tracks
          </h2>
          <button type="button" onClick={onClose} aria-label="Close mixable tracks">
            ✕
          </button>
        </div>
        <p>Select a track to see what mixes out of it.</p>
      </div>
    );
  }

  if (!options) {
    return (
      <div className="flex h-full flex-col p-4 text-xs text-muted">
        <p>{error ?? "Loading rules…"}</p>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden" aria-label="Mixable tracks">
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-muted">
          Mixable Tracks
        </h2>
        <button type="button" onClick={onClose} aria-label="Close mixable tracks">
          ✕
        </button>
      </div>

      <div className="border-b border-border px-4 py-2">
        <p className="truncate text-sm font-medium">{track.title}</p>
        <p className="truncate text-[11px] text-muted">
          {track.artist ?? "Unknown artist"}
          {track.bpm != null && ` · ${track.bpm.toFixed(1)} BPM`}
          {track.musical_key != null && ` · ${track.musical_key}`}
        </p>
        {result != null && result.compatible_keys.length > 0 && (
          <p className="mt-1 text-[11px] text-muted" data-testid="compatible-keys">
            Mixes into {result.compatible_keys.join(", ")}
          </p>
        )}
      </div>

      <div className="space-y-2 border-b border-border px-4 py-3 text-xs">
        <label className="flex items-center justify-between gap-2">
          <span className="text-muted">BPM range (±%)</span>
          <input
            type="number"
            min={0}
            step={0.5}
            aria-label="BPM tolerance percent"
            className="w-20 rounded border border-border bg-surface px-2 py-1 text-xs"
            value={options.bpm_tolerance_pct ?? ""}
            placeholder="any"
            onChange={(e) =>
              update({
                bpm_tolerance_pct:
                  e.target.value === "" ? null : Number(e.target.value),
              })
            }
          />
        </label>

        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={options.match_key}
            aria-label="Match key"
            onChange={(e) =>
              update({ match_key: e.target.checked })
            }
          />
          Match key
        </label>

        <label className="flex items-center justify-between gap-2">
          <span className="text-muted">Key mixing mode</span>
          <select
            aria-label="Key mixing mode"
            className="rounded border border-border bg-surface px-2 py-1 text-xs"
            value={mode}
            onChange={(e) => void changeMode(e.target.value as KeyMixingMode)}
          >
            <option value="harmonically_compatible">Harmonically compatible</option>
            <option value="fuzzy">Fuzzy key mixing</option>
          </select>
        </label>

        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={options.include_half_double}
            aria-label="Include half and double BPM"
            onChange={(e) =>
              update({ include_half_double: e.target.checked })
            }
          />
          Include half / double BPM
        </label>

        <button
          type="button"
          className="text-[11px] text-muted underline"
          onClick={() => setAdvanced((a) => !a)}
        >
          {advanced ? "Hide advanced rules" : "Advanced rules"}
        </button>

        {advanced && (
          <div className="space-y-2 border-t border-border pt-2" data-testid="advanced-rules">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={options.must_have_cues}
                aria-label="Must have cue points"
                onChange={(e) =>
                  update({ must_have_cues: e.target.checked })
                }
              />
              Must have cue points
            </label>

            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={options.match_color}
                aria-label="Match colour"
                onChange={(e) => update({ match_color: e.target.checked })}
              />
              Match colour
            </label>

            <label className="flex items-center justify-between gap-2">
              <span className="text-muted">Added since</span>
              <input
                type="date"
                aria-label="Added since"
                className="rounded border border-border bg-surface px-2 py-1 text-xs"
                value={options.added_since ?? ""}
                onChange={(e) =>
                  update({ added_since: e.target.value || null })
                }
              />
            </label>

            <label className="block">
              <span className="mb-1 block text-muted">Genres (comma separated)</span>
              <input
                aria-label="Genres"
                className="w-full rounded border border-border bg-surface px-2 py-1 text-xs"
                value={genreText}
                placeholder="any"
                onChange={(e) =>
                  update({
                    genres: e.target.value
                      .split(",")
                      .map((g) => g.trim())
                      .filter((g) => g !== ""),
                  })
                }
              />
            </label>

            <label className="flex items-center justify-between gap-2">
              <span className="text-muted">Energy</span>
              <select
                aria-label="Energy rule"
                className="rounded border border-border bg-surface px-2 py-1 text-xs"
                value={options.energy.kind}
                onChange={(e) =>
                  update({
                    energy:
                      e.target.value === "near_source"
                        ? { kind: "near_source" }
                        : { kind: "off" },
                  })
                }
              >
                <option value="off">Any</option>
                <option value="near_source">Within 1 of this track</option>
              </select>
            </label>

            <label className="flex items-center justify-between gap-2">
              <span className="text-muted">Rating</span>
              <select
                aria-label="Rating rule"
                className="rounded border border-border bg-surface px-2 py-1 text-xs"
                value={options.rating.kind}
                onChange={(e) =>
                  update({
                    rating:
                      e.target.value === "near_source"
                        ? { kind: "near_source" }
                        : { kind: "off" },
                  })
                }
              >
                <option value="off">Any</option>
                <option value="near_source">Within 1 of this track</option>
              </select>
            </label>

            <label className="flex items-center justify-between gap-2">
              <span className="text-muted">Year</span>
              <select
                aria-label="Year rule"
                className="rounded border border-border bg-surface px-2 py-1 text-xs"
                value={options.year.kind}
                onChange={(e) =>
                  update({
                    year:
                      e.target.value === "same_as_source"
                        ? { kind: "same_as_source" }
                        : { kind: "off" },
                  })
                }
              >
                <option value="off">Any</option>
                <option value="same_as_source">Same as this track</option>
              </select>
            </label>

            <p className="text-[11px] text-muted">
              Popularity, Danceability and Happiness are not offered: Lexicon
              takes them from Spotify, whose audio-features endpoint has been
              withdrawn, and Popularity cannot be measured locally at all.
            </p>

            <div className="border-t border-border pt-2">
              <span className="mb-1 block text-muted">Templates</span>
              {templates.length > 0 && (
                <ul className="mb-1 space-y-0.5" data-testid="mixable-templates">
                  {templates.map((t) => (
                    <li key={t.id} className="flex items-center justify-between gap-2">
                      <button
                        type="button"
                        className="truncate text-left underline"
                        onClick={() => setOptions(t.options)}
                      >
                        {t.name}
                      </button>
                      <button
                        type="button"
                        aria-label={`Delete template ${t.name}`}
                        className="text-muted"
                        onClick={() => void removeTemplate(t.id)}
                      >
                        ✕
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <div className="flex gap-1">
                <input
                  aria-label="Template name"
                  className="flex-1 rounded border border-border bg-surface px-2 py-1 text-xs"
                  placeholder="Peak time"
                  value={templateName}
                  onChange={(e) => setTemplateName(e.target.value)}
                />
                <button
                  type="button"
                  disabled={templateName.trim() === ""}
                  className="rounded border border-border px-2 py-1 disabled:opacity-50"
                  onClick={() => void saveTemplate()}
                >
                  Save
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

      <div className="flex-1 overflow-auto px-4 py-2 text-xs">
        {loading && <p className="text-muted">Searching…</p>}
        {error != null && <p className="text-red-500">{error}</p>}
        {!loading && error == null && result != null && (
          <>
            <p className="mb-2 text-[11px] text-muted" data-testid="mixable-count">
              {result.matches.length} of {result.considered} track(s) mix out of
              this one.
            </p>
            {result.matches.length === 0 ? (
              <p className="text-muted">
                Nothing matched. Widen the BPM range, or turn off Match key.
              </p>
            ) : (
              <ul className="space-y-1" data-testid="mixable-results">
                {result.matches.map((m) => (
                  <li
                    key={m.track.id}
                    className="rounded border border-border px-2 py-1"
                  >
                    <div className="flex items-baseline justify-between gap-2">
                      <span className="truncate font-medium">{m.track.title}</span>
                      <span className="shrink-0 tabular-nums text-muted">
                        {m.score.toFixed(0)}
                      </span>
                    </div>
                    <p className="truncate text-[11px] text-muted">
                      {m.track.artist ?? "Unknown artist"}
                      {m.track.bpm != null && ` · ${m.track.bpm.toFixed(1)}`}
                      {m.track.musical_key != null && ` · ${m.track.musical_key}`}
                      {m.bpm_relation !== "direct" && ` · ${m.bpm_relation}-time`}
                    </p>
                    <p className="truncate text-[11px] text-muted">
                      {m.reasons.join(" · ")}
                    </p>
                    <button
                      type="button"
                      className="mt-1 text-[11px] underline"
                      onClick={() => onUseAsNextTrack(m.track)}
                    >
                      Use as next track
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </div>

      <div className="border-t border-border px-4 py-2 text-[11px] text-muted">
        Energy: {numericLabel(options.energy)} · Rating:{" "}
        {numericLabel(options.rating)}
      </div>
    </div>
  );
}
