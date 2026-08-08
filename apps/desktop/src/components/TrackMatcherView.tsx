import { useMemo, useState } from "react";
import {
  createPlaylistFromTracks,
  matchTracks,
  parseCsvForMatcher,
  parseCsvHeadersForMatcher,
  parseTracklistForMatcher,
  storeLinksForTracks,
  type MatchInput,
  type MatchResult,
} from "../ipc";
import type { Separator, Store } from "../types";
import { useDialog } from "../hooks/useDialog";
import { readTextFile } from "../lib/read-file";
import { useToast } from "./Toast";

interface Props {
  libraryPath: string;
  onGoToSync?: () => void;
}

type Source = "paste" | "txt" | "csv";

/**
 * What the separator `<select>` shows vs. what goes over IPC. Kept as its
 * own union rather than reusing `Separator` directly so "custom" can be a
 * selectable state before the user has typed anything into the delimiter
 * box — `Separator`'s `Custom` variant always carries a (possibly empty)
 * string, which isn't a value this dropdown can represent on its own.
 */
type SeparatorKind = "hyphen" | "en_dash" | "em_dash" | "by" | "none" | "custom";

/** Every storefront the backend knows how to build a search link for. There
 *  is no per-store picker in this cut — Lexicon parity here is "give the user
 *  somewhere to look", not curation, so asking them to opt stores in first
 *  would be friction for no benefit. */
const ALL_STORES: Store[] = [
  "beatport",
  "bandcamp",
  "discogs",
  "spotify",
  "tidal",
  "soundcloud",
  "youtube",
];

export function TrackMatcherView({ libraryPath, onGoToSync }: Props) {
  const dialog = useDialog();
  const { toast } = useToast();

  const [source, setSource] = useState<Source>("paste");
  const [pasted, setPasted] = useState("");
  const [txtText, setTxtText] = useState("");
  const [separatorKind, setSeparatorKind] = useState<SeparatorKind>("hyphen");
  const [customSeparator, setCustomSeparator] = useState("");
  const [csvText, setCsvText] = useState<string>("");
  const [csvHeaders, setCsvHeaders] = useState<string[]>([]);
  const [csvRowCount, setCsvRowCount] = useState<number>(0);
  const [titleCol, setTitleCol] = useState<number>(0);
  const [artistCol, setArtistCol] = useState<number>(-1);
  const [results, setResults] = useState<MatchResult[]>([]);
  const [matching, setMatching] = useState(false);
  const [storeLinks, setStoreLinks] = useState<
    { title: string; artist: string | null; links: [string, string][] }[]
  >([]);
  const [findingLinks, setFindingLinks] = useState(false);

  const matched = useMemo(
    () => results.filter((r) => r.track !== null),
    [results],
  );
  const matchedIds = useMemo(
    () => matched.map((r) => r.track!.id),
    [matched],
  );
  const unmatched = useMemo(
    () => results.filter((r) => r.status === "Unmatched"),
    [results],
  );

  const parsePasted = (): MatchInput[] => {
    return pasted
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => {
        // "Artist - Title" → split; otherwise treat whole line as title
        const parts = line.split(/\s+-\s+/);
        if (parts.length === 2) {
          return { title: parts[1].trim(), artist: parts[0].trim() };
        }
        return { title: line };
      });
  };

  /** What the separator select actually sends. "custom" is a UI-only state
   *  until this point — it becomes the `Custom` variant here, carrying
   *  whatever the user typed (even empty, which the backend can reject on
   *  its own rather than this component guessing what counts as valid). */
  const separatorArg = (): Separator =>
    separatorKind === "custom" ? { custom: customSeparator } : separatorKind;

  const onTxtUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0];
    if (!f) return;
    setTxtText(await readTextFile(f));
  };

  const onCsvUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0];
    if (!f) return;
    const text = await readTextFile(f);
    // Extract headers via the backend RFC-4180 CSV parser so quoted commas in
    // header names (e.g. `"Last, First",artist`) are handled correctly. Backend
    // re-parses authoritatively at match-time.
    let headers: string[];
    try {
      headers = await parseCsvHeadersForMatcher(text);
    } catch (err) {
      toast({
        variant: "error",
        message: "Failed to parse CSV headers",
        detail: String(err),
      });
      return;
    }
    if (headers.length === 0) {
      // Empty/headerless CSV — fall back to treating the file as one input per line.
      setPasted(text);
      setSource("paste");
      return;
    }
    const nonEmpty = text.split(/\r?\n/).filter((l) => l.trim().length > 0);
    setCsvText(text);
    setCsvHeaders(headers);
    setCsvRowCount(Math.max(0, nonEmpty.length - 1));
    setTitleCol(0);
    setArtistCol(headers.length > 1 ? 1 : -1);
    setSource("csv");
  };

  const doMatch = async () => {
    setMatching(true);
    try {
      let inputs: MatchInput[];
      if (source === "csv") {
        if (!csvText || csvHeaders.length === 0 || titleCol < 0) {
          toast({ variant: "info", message: "Upload a CSV first." });
          return;
        }
        inputs = await parseCsvForMatcher(
          csvText,
          csvHeaders[titleCol]!,
          artistCol >= 0 ? csvHeaders[artistCol] : undefined,
        );
      } else if (source === "txt") {
        if (!txtText.trim()) {
          toast({ variant: "info", message: "Paste or upload a tracklist first." });
          return;
        }
        inputs = await parseTracklistForMatcher(txtText, separatorArg());
      } else {
        inputs = parsePasted();
      }
      if (inputs.length === 0) {
        toast({ variant: "info", message: "No input rows to match." });
        return;
      }
      const res = await matchTracks(libraryPath, inputs);
      setResults(res);
      // Stale store links point at the previous unmatched list by index —
      // clearing them here is cheaper than trying to reconcile old links
      // against a new match run.
      setStoreLinks([]);
    } catch (e) {
      toast({ variant: "error", message: "Match failed", detail: String(e) });
    } finally {
      setMatching(false);
    }
  };

  const doCreatePlaylist = async () => {
    if (matchedIds.length === 0) return;
    const name = await dialog.prompt({
      title: "Playlist name",
      placeholder: "e.g. Spotify – Chill Vibes",
      defaultValue:
        source === "csv"
          ? "Imported (CSV)"
          : source === "txt"
            ? "Imported (tracklist)"
            : "Imported (paste)",
    });
    if (!name) return;
    await createPlaylistFromTracks(libraryPath, name, matchedIds);
    toast({
      variant: "success",
      message: `Staged playlist '${name}' with ${matchedIds.length} track(s).`,
      detail: "Review and apply in the Sync panel.",
      action: onGoToSync ? { label: "Review & Sync", onClick: onGoToSync } : undefined,
    });
  };

  const doExportUnmatched = () => {
    const lines = unmatched.map((r) =>
      r.input_artist ? `${r.input_artist} - ${r.input_title}` : r.input_title,
    );
    if (lines.length === 0) return;
    const blob = new Blob([lines.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "unmatched.txt";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  /**
   * Look up storefront search links for whatever is still unmatched. Fired
   * on demand rather than after every match — most matches leave only a
   * handful of stragglers, and firing seven store lookups per row on every
   * Match click would be a lot of network noise for links most users won't
   * click.
   */
  const doFindStoreLinks = async () => {
    if (unmatched.length === 0) return;
    setFindingLinks(true);
    try {
      const tracks = unmatched.map((r) => ({
        title: r.input_title,
        artist: r.input_artist,
      }));
      const links = await storeLinksForTracks(tracks, ALL_STORES);
      setStoreLinks(links);
    } catch (e) {
      toast({
        variant: "error",
        message: "Store search failed",
        detail: String(e),
      });
    } finally {
      setFindingLinks(false);
    }
  };

  return (
    <div className="flex h-full flex-col bg-surface p-4 text-sm">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-ink">Track Matcher</h2>
        {onGoToSync && (
          <button
            onClick={onGoToSync}
            className="rounded bg-elevated px-3 py-1 text-ink hover:bg-edge"
          >
            Review & Sync →
          </button>
        )}
      </div>

      <div className="mb-3 flex items-center gap-2">
        <label className="text-xs uppercase tracking-wide text-ink-muted">Source</label>
        <select
          value={source}
          onChange={(e) => setSource(e.target.value as Source)}
          className="rounded border border-edge bg-base px-2 py-1 text-ink"
        >
          <option value="paste">Paste / text</option>
          <option value="txt">.txt / .m3u8 tracklist</option>
          <option value="csv">.csv upload</option>
        </select>
        {source === "txt" && (
          <input type="file" accept=".txt,.m3u8" onChange={onTxtUpload} />
        )}
        {source === "csv" && (
          <input type="file" accept=".csv" onChange={onCsvUpload} />
        )}
        <div className="flex-1" />
        <button
          onClick={doMatch}
          disabled={matching}
          className="rounded bg-accent px-3 py-1 font-medium text-base hover:opacity-90 disabled:opacity-50"
        >
          {matching ? "Matching…" : "Match"}
        </button>
      </div>

      {source === "paste" && (
        <textarea
          value={pasted}
          onChange={(e) => setPasted(e.target.value)}
          placeholder={"One per line:\nArtist - Title\nor just Title"}
          className="mb-3 h-32 w-full rounded border border-edge bg-base p-2 font-mono text-xs text-ink"
        />
      )}

      {source === "txt" && (
        <div className="mb-3 space-y-2">
          <div className="flex items-center gap-2">
            <label className="text-xs uppercase tracking-wide text-ink-muted">
              Separator
            </label>
            <select
              value={separatorKind}
              onChange={(e) => setSeparatorKind(e.target.value as SeparatorKind)}
              className="rounded border border-edge bg-base px-2 py-1 text-ink"
            >
              <option value="hyphen">{" - "} (default)</option>
              <option value="en_dash">{" – "} (en dash)</option>
              <option value="em_dash">{" — "} (em dash)</option>
              <option value="by">Title by Artist</option>
              <option value="none">No separator (whole line is a title)</option>
              <option value="custom">Custom…</option>
            </select>
            {separatorKind === "custom" && (
              <input
                type="text"
                value={customSeparator}
                onChange={(e) => setCustomSeparator(e.target.value)}
                placeholder="Custom separator, e.g. ::"
                className="rounded border border-edge bg-base px-2 py-1 text-ink"
              />
            )}
          </div>
          <textarea
            value={txtText}
            onChange={(e) => setTxtText(e.target.value)}
            placeholder={"One per line, split by the separator above"}
            className="h-32 w-full rounded border border-edge bg-base p-2 font-mono text-xs text-ink"
          />
        </div>
      )}

      {source === "csv" && csvHeaders.length > 0 && (
        <div className="mb-3 flex gap-3 rounded border border-edge bg-base p-3">
          <label className="flex items-center gap-2 text-xs text-ink-muted">
            Title column
            <select
              value={titleCol}
              onChange={(e) => setTitleCol(Number(e.target.value))}
              className="rounded border border-edge bg-surface px-2 py-1 text-ink"
            >
              {csvHeaders.map((h, i) => (
                <option key={i} value={i}>
                  {h || `col ${i}`}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-2 text-xs text-ink-muted">
            Artist column
            <select
              value={artistCol}
              onChange={(e) => setArtistCol(Number(e.target.value))}
              className="rounded border border-edge bg-surface px-2 py-1 text-ink"
            >
              <option value={-1}>— none —</option>
              {csvHeaders.map((h, i) => (
                <option key={i} value={i}>
                  {h || `col ${i}`}
                </option>
              ))}
            </select>
          </label>
          <span className="text-xs text-ink-muted">
            {csvRowCount} rows · {csvHeaders.length} columns
          </span>
        </div>
      )}

      {results.length > 0 && (
        <div className="mb-3 flex items-center gap-3 rounded border border-edge bg-base px-3 py-2 text-xs">
          <span className="font-medium text-ink">
            {matched.length} / {results.length} tracks matched
          </span>
          <span className="text-ink-muted">
            ({results.filter((r) => r.status === "Exact").length} exact,{" "}
            {results.filter((r) => r.status === "Fuzzy").length} fuzzy)
          </span>
          <div className="flex-1" />
          <button
            onClick={doExportUnmatched}
            className="rounded bg-elevated px-2 py-1 text-ink hover:bg-edge"
            disabled={unmatched.length === 0}
          >
            Export unmatched
          </button>
          <button
            onClick={doCreatePlaylist}
            disabled={matchedIds.length === 0}
            className="rounded bg-accent px-3 py-1 font-medium text-base hover:opacity-90 disabled:opacity-50"
          >
            Create playlist ({matchedIds.length})
          </button>
        </div>
      )}

      <div className="flex-1 overflow-auto rounded-lg border border-edge bg-base">
        {results.length === 0 ? (
          <div className="flex h-full items-center justify-center text-ink-muted">
            Paste or upload a list, then click Match.
          </div>
        ) : (
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-surface text-ink-muted">
              <tr>
                <th className="w-6 py-1 px-2 text-left"> </th>
                <th className="py-1 px-2 text-left">Input</th>
                <th className="py-1 px-2 text-left">Matched to</th>
                <th className="py-1 px-2 text-left">Score</th>
              </tr>
            </thead>
            <tbody>
              {results.map((r, idx) => (
                <tr key={idx} className="border-t border-edge">
                  <td className="py-1 px-2">{statusIcon(r.status)}</td>
                  <td className="py-1 px-2 truncate max-w-[280px] text-ink">
                    {r.input_artist ? `${r.input_artist} — ` : ""}
                    {r.input_title}
                  </td>
                  <td className="py-1 px-2 truncate max-w-[280px] text-ink">
                    {r.track
                      ? `${r.track.artist ? `${r.track.artist} — ` : ""}${r.track.title}`
                      : "—"}
                  </td>
                  <td className="py-1 px-2 tabular-nums text-ink-muted">
                    {r.score > 0 ? `${(r.score * 100).toFixed(0)}%` : ""}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {unmatched.length > 0 && (
        <div className="mt-3 rounded border border-edge bg-base p-3 text-xs">
          <div className="mb-2 flex items-center justify-between gap-2">
            <span className="font-medium text-ink">
              {unmatched.length} unmatched — search other stores
            </span>
            <button
              onClick={() => void doFindStoreLinks()}
              disabled={findingLinks}
              className="rounded bg-elevated px-2 py-1 text-ink hover:bg-edge disabled:opacity-50"
            >
              {findingLinks ? "Searching…" : "Find store links"}
            </button>
          </div>
          {/* These are query-string search URLs, not a purchase or
              playlist-push integration — either would need a registered
              developer app plus a token scoped to this user, which is a
              real setup burden this local-first tool has no business
              asking for just to save someone a manual search. */}
          <p className="mb-2 text-ink-muted">
            Search links only — decks does not compare prices or push
            playlists to a store. Both need a registered developer app and a
            per-user token, so they open the store's search page for you to
            take it from there.
          </p>
          {storeLinks.length > 0 && (
            <ul className="space-y-1.5">
              {unmatched.map((r, idx) => {
                const entry = storeLinks[idx];
                return (
                  <li
                    key={idx}
                    className="flex flex-wrap items-center gap-x-2 gap-y-1"
                  >
                    <span className="text-ink">
                      {r.input_artist ? `${r.input_artist} — ` : ""}
                      {r.input_title}
                    </span>
                    {entry?.links.map(([label, url]) => (
                      <a
                        key={label}
                        href={url}
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent underline"
                      >
                        {label}
                      </a>
                    ))}
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}

function statusIcon(s: "Exact" | "Fuzzy" | "Unmatched") {
  if (s === "Exact") return <span className="text-green-500">✓</span>;
  if (s === "Fuzzy") return <span className="text-yellow-500">~</span>;
  return <span className="text-orange-500">—</span>;
}
