import { useMemo, useState } from "react";
import {
  createPlaylistFromTracks,
  matchTracks,
  parseCsvForMatcher,
  parseCsvHeadersForMatcher,
  type MatchInput,
  type MatchResult,
} from "../ipc";
import { useDialog } from "../hooks/useDialog";
import { readTextFile } from "../lib/read-file";
import { useToast } from "./Toast";

interface Props {
  libraryPath: string;
  onGoToSync?: () => void;
}

type Source = "paste" | "txt" | "csv";

export function TrackMatcherView({ libraryPath, onGoToSync }: Props) {
  const dialog = useDialog();
  const { toast } = useToast();

  const [source, setSource] = useState<Source>("paste");
  const [pasted, setPasted] = useState("");
  const [csvText, setCsvText] = useState<string>("");
  const [csvHeaders, setCsvHeaders] = useState<string[]>([]);
  const [csvRowCount, setCsvRowCount] = useState<number>(0);
  const [titleCol, setTitleCol] = useState<number>(0);
  const [artistCol, setArtistCol] = useState<number>(-1);
  const [results, setResults] = useState<MatchResult[]>([]);
  const [matching, setMatching] = useState(false);

  const matched = useMemo(
    () => results.filter((r) => r.track !== null),
    [results],
  );
  const matchedIds = useMemo(
    () => matched.map((r) => r.track!.id),
    [matched],
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

  const onTxtUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0];
    if (!f) return;
    const text = await readTextFile(f);
    setPasted(text);
    setSource("paste");
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
      } else {
        inputs = parsePasted();
      }
      if (inputs.length === 0) {
        toast({ variant: "info", message: "No input rows to match." });
        return;
      }
      const res = await matchTracks(libraryPath, inputs);
      setResults(res);
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
        source === "csv" ? "Imported (CSV)" : "Imported (paste)",
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
    const lines = results
      .filter((r) => r.status === "Unmatched")
      .map((r) =>
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
          <option value="txt">.txt upload</option>
          <option value="csv">.csv upload</option>
        </select>
        {source === "txt" && (
          <input type="file" accept=".txt" onChange={onTxtUpload} />
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

      {source !== "csv" && (
        <textarea
          value={pasted}
          onChange={(e) => setPasted(e.target.value)}
          placeholder={"One per line:\nArtist - Title\nor just Title"}
          className="mb-3 h-32 w-full rounded border border-edge bg-base p-2 font-mono text-xs text-ink"
        />
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
            disabled={results.every((r) => r.status !== "Unmatched")}
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
    </div>
  );
}

function statusIcon(s: "Exact" | "Fuzzy" | "Unmatched") {
  if (s === "Exact") return <span className="text-green-500">✓</span>;
  if (s === "Fuzzy") return <span className="text-yellow-500">~</span>;
  return <span className="text-orange-500">—</span>;
}

