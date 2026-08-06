import type { Playlist, Smartlist, Track } from "../types";

/**
 * The Find Popup's search — one box over playlists, smartlists and tracks.
 *
 * Per `docs/lexicon/00-overview.md §Find Popup`. Distinct from the Action
 * Center (`Cmd+K`), which searches *commands*: this searches **content**, and
 * each result carries actions that make sense for what it is.
 *
 * Pure and synchronous. The library is already in memory for the browser, so a
 * round-trip per keystroke would be slower and worse — and it means the ranking
 * rules are testable without a Tauri host.
 */

export type FindResultKind = "track" | "playlist" | "smartlist";

export interface FindResult {
  kind: FindResultKind;
  id: string;
  /** What the row reads as. */
  label: string;
  /** The dimmer second line — artist, or how many rules a smartlist has. */
  sublabel?: string;
  /** Higher sorts first. Internal, but exposed so the tests can assert on it. */
  score: number;
}

export interface FindCorpus {
  tracks: Track[];
  playlists: Playlist[];
  smartlists: Smartlist[];
}

/** Results shown per section, so one huge library cannot bury the playlists. */
export const PER_KIND_LIMIT = 8;

/**
 * How well `haystack` matches `needle`, or `0` for no match.
 *
 * Three tiers rather than a fuzzy distance: an exact match, then a match at the
 * start of the string, then at the start of any word, then anywhere. Fuzzy
 * subsequence matching is great for command palettes with a hundred short
 * fixed strings and bad for a library with four thousand track titles, where it
 * matches almost everything and ranks by noise.
 */
export function scoreMatch(haystack: string, needle: string): number {
  if (needle === "") return 1;
  const h = haystack.toLowerCase();
  const n = needle.toLowerCase();
  if (h === n) return 100;
  if (h.startsWith(n)) return 75;
  // A word start — "rain" should find "Acid Rain" ahead of "Braindance".
  const atWordStart = new RegExp(`\\b${escapeRegExp(n)}`).test(h);
  if (atWordStart) return 50;
  if (h.includes(n)) return 25;
  return 0;
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** The best score across several fields, so artist matches as well as title. */
function bestScore(needle: string, ...fields: (string | null | undefined)[]) {
  let best = 0;
  for (const field of fields) {
    if (!field) continue;
    best = Math.max(best, scoreMatch(field, needle));
  }
  return best;
}

/**
 * Search everything.
 *
 * Returns results grouped by kind, each already sorted and capped. Grouping is
 * returned rather than one flat list because the popup renders sections, and
 * flattening then re-splitting in the component is how the cap silently stops
 * applying per section.
 *
 * An empty query returns nothing rather than everything: a Find popup that
 * opens showing 4,000 tracks has answered a question nobody asked.
 */
export function findAll(
  query: string,
  corpus: FindCorpus,
  limit: number = PER_KIND_LIMIT,
): Record<FindResultKind, FindResult[]> {
  const q = query.trim();
  const empty = { track: [], playlist: [], smartlist: [] };
  if (q === "") return empty;

  const tracks: FindResult[] = [];
  for (const t of corpus.tracks) {
    const score = bestScore(q, t.title, t.artist, t.album);
    if (score > 0) {
      tracks.push({
        kind: "track",
        id: t.id,
        label: t.title,
        sublabel: t.artist ?? undefined,
        score,
      });
    }
  }

  const playlists: FindResult[] = [];
  for (const p of corpus.playlists) {
    // Folders are not a destination for tracks and cannot be played; offering
    // one would give the user a row whose actions all fail.
    if (p.kind === "Folder") continue;
    const score = scoreMatch(p.name, q);
    if (score > 0) {
      playlists.push({ kind: "playlist", id: p.id, label: p.name, score });
    }
  }

  const smartlists: FindResult[] = [];
  for (const s of corpus.smartlists) {
    const score = scoreMatch(s.name, q);
    if (score > 0) {
      const ruleCount = s.clauses.reduce((n, c) => n + c.rules.length, 0);
      smartlists.push({
        kind: "smartlist",
        id: s.id,
        label: s.name,
        sublabel: `${ruleCount} rule${ruleCount === 1 ? "" : "s"}`,
        score,
      });
    }
  }

  return {
    track: rank(tracks, limit),
    playlist: rank(playlists, limit),
    smartlist: rank(smartlists, limit),
  };
}

/**
 * Sort by score, then alphabetically.
 *
 * The alphabetical tiebreak is not cosmetic: without it, equal-scoring results
 * come back in library order, so the same query returns a different top result
 * after any re-sort — and `Enter` plays something different than it did a
 * moment ago.
 */
function rank(results: FindResult[], limit: number): FindResult[] {
  return results
    .sort((a, b) => b.score - a.score || a.label.localeCompare(b.label))
    .slice(0, limit);
}

/** The results as one list, in section order — what arrow keys walk. */
export function flatten(
  grouped: Record<FindResultKind, FindResult[]>,
): FindResult[] {
  return [...grouped.playlist, ...grouped.smartlist, ...grouped.track];
}
