import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTrackCues } from "../hooks/useTrackCues";
import { CueEditor } from "./CueEditor";
import { ColorWaveform } from "@/components/ui/ColorWaveform";
import {
  analyzeTrack,
  getAnlzWaveform,
  getAudioWaveform,
  stageChange,
} from "../ipc";
import type { Track, HotCue, CueKind, AnalysisResult, AnlzWaveform } from "../types";

const STAR_RATINGS = [0, 1, 2, 3, 4, 5] as const;

function formatMs(ms: number): string {
  const totalSec = ms / 1000;
  const m = Math.floor(totalSec / 60);
  const s = (totalSec % 60).toFixed(1).padStart(4, "0");
  return `${m}:${s}`;
}

function formatDuration(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = String(secs % 60).padStart(2, "0");
  return `${m}:${s}`;
}

function cueSlotLabel(kind: CueKind): string {
  if (kind === "MemoryCue") return "M";
  return String(kind.HotCue);
}

const HOT_CUE_COLORS = [
  "bg-red-500",
  "bg-orange-500",
  "bg-yellow-400",
  "bg-green-500",
  "bg-cyan-500",
  "bg-blue-500",
  "bg-violet-500",
  "bg-pink-500",
];

function cueColor(kind: CueKind): string {
  if (kind === "MemoryCue") return "bg-ink-secondary";
  return HOT_CUE_COLORS[(kind.HotCue - 1) % HOT_CUE_COLORS.length];
}

function MetaRow({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  if (value === null || value === undefined || value === "") return null;
  return (
    <div className="flex items-baseline gap-3 py-1">
      <span className="w-16 shrink-0 text-[10px] font-medium uppercase tracking-[0.08em] text-ink-muted">
        {label}
      </span>
      <span
        className={
          mono
            ? "truncate font-mono text-[13px] tabular-nums text-ink"
            : "truncate text-sm text-ink"
        }
      >
        {value}
      </span>
    </div>
  );
}

function CueRow({ cue }: { cue: HotCue }) {
  const slot = cueSlotLabel(cue.kind);
  const color = cueColor(cue.kind);
  return (
    <div className="flex items-center gap-3 rounded px-2 py-1.5 transition-colors duration-150 hover:bg-surface">
      <span
        className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-sm font-mono text-[10px] font-bold text-base ${color}`}
      >
        {slot}
      </span>
      <span className="w-16 shrink-0 font-mono text-[11px] tabular-nums text-ink-secondary">
        {cue.in_msec != null ? formatMs(cue.in_msec) : "—"}
      </span>
      <span className="truncate text-xs text-ink-secondary">
        {cue.comment ?? ""}
      </span>
    </div>
  );
}

/** Convert a Tailwind `bg-*` class for a cue color to a hex value for SVG
 *  rendering. Keeps the cue palette as the single source of truth. */
const CUE_HEX: Record<string, string> = {
  "bg-red-500": "#ef4444",
  "bg-orange-500": "#f97316",
  "bg-yellow-400": "#facc15",
  "bg-green-500": "#22c55e",
  "bg-cyan-500": "#06b6d4",
  "bg-blue-500": "#3b82f6",
  "bg-violet-500": "#8b5cf6",
  "bg-pink-500": "#ec4899",
  "bg-ink-secondary": "rgb(var(--text-secondary))",
};

/**
 * Renders the track's cue points over the decoded audio waveform. The
 * waveform is computed from the actual audio file via the Rust backend
 * (`get_audio_waveform`); cue positions and seek interactions are real.
 */
function CuePositionBar({
  trackId,
  libraryPath,
  folderPath,
  cues,
  durationSecs,
  currentTimeSecs = 0,
  onSeekFraction,
}: {
  trackId: string;
  libraryPath: string | null;
  folderPath: string | null;
  cues: HotCue[];
  durationSecs: number | null;
  currentTimeSecs?: number;
  onSeekFraction?: (fraction: number) => void;
}) {
  const durationMs =
    durationSecs != null && durationSecs > 0 ? durationSecs * 1000 : null;

  // Sorted positions of valid cues for region rendering.
  const positioned = (cues ?? [])
    .filter((c) => c.in_msec != null && c.in_msec >= 0)
    .sort((a, b) => (a.in_msec ?? 0) - (b.in_msec ?? 0));

  const waveformQuery = useQuery<AnlzWaveform, Error>({
    queryKey: ["waveform", libraryPath, trackId],
    queryFn: () => getAnlzWaveform(libraryPath!, trackId),
    enabled: !!libraryPath && !!trackId,
    staleTime: Infinity,
    gcTime: 10 * 60 * 1000,
    retry: false,
  });

  const anlzData = waveformQuery.data;
  const anlzHasData =
    !!anlzData &&
    ((anlzData.preview?.length ?? 0) > 0 ||
      (anlzData.detail?.length ?? 0) > 0);

  // Fallback: when Rekordbox never analysed this track, ANLZ returns
  // empty arrays. Auto-decode the underlying audio file with symphonia
  // so the waveform area renders something real instead of a placeholder.
  // Gated on the ANLZ query having completed, so we don't decode if
  // ANLZ would have given us better Pioneer color data.
  const peaksQuery = useQuery<number[], Error>({
    queryKey: ["audio-peaks", folderPath],
    queryFn: () => getAudioWaveform(folderPath!, 1200),
    enabled:
      !!folderPath &&
      !waveformQuery.isFetching &&
      !waveformQuery.isPending &&
      !anlzHasData,
    staleTime: Infinity,
    gcTime: 10 * 60 * 1000,
    retry: false,
  });

  // Merge ANLZ + audio-peaks fallback into a single AnlzWaveform-shaped
  // payload so <ColorWaveform> can render either path.
  const renderData: AnlzWaveform | null = anlzHasData
    ? anlzData!
    : peaksQuery.data
      ? {
          preview: [],
          detail: [],
          beat_grid: [],
          peaks: peaksQuery.data,
        }
      : null;

  const hasRealPeaks = !!renderData;
  const peaksLoading = peaksQuery.isFetching;
  const peaksError = peaksQuery.error?.message ?? null;

  const showUnanalysedNotice =
    !waveformQuery.isFetching &&
    !anlzHasData &&
    !peaksQuery.data &&
    !peaksLoading &&
    (!folderPath || !!peaksError);

  function handleClick(e: React.MouseEvent<HTMLDivElement>) {
    if (!onSeekFraction) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const fraction = Math.max(0, Math.min(1, x / rect.width));
    onSeekFraction(fraction);
  }

  return (
    <div className="mx-4 mt-4 select-none">
      <div className="mb-1.5 flex items-center justify-between font-mono text-[10px] tabular-nums text-ink-faint">
        <span>
          {currentTimeSecs > 0 ? formatDuration(Math.floor(currentTimeSecs)) : "0:00"}
        </span>
        {(waveformQuery.isFetching || peaksLoading) && (
          <span className="font-mono text-[9px] uppercase tracking-wider text-ink-faint">
            {peaksLoading ? "decoding…" : "loading…"}
          </span>
        )}
        {hasRealPeaks && !waveformQuery.isFetching && !peaksLoading && (
          <span
            className="font-mono text-[9px] uppercase tracking-wider text-ink-faint"
            title={anlzHasData ? "ANLZ Color Waveform" : "Decoded Audio Peaks"}
          >
            {anlzHasData ? "anlz" : "peaks"}
          </span>
        )}
        <span>
          {durationSecs != null && durationSecs > 0
            ? formatDuration(durationSecs)
            : "—:—"}
        </span>
      </div>

      {/* Timeline */}
      <div
        className={`relative h-16 overflow-hidden rounded-md border border-edge bg-elevated ${onSeekFraction ? "cursor-pointer" : ""}`}
        onClick={onSeekFraction ? handleClick : undefined}
        role={onSeekFraction ? "slider" : undefined}
        aria-label={onSeekFraction ? "Seek track" : undefined}
        aria-valuemin={0}
        aria-valuemax={durationSecs ?? 0}
      >
        {/* Waveform background */}
        <div className="absolute inset-0 opacity-80">
          {renderData && (
            <ColorWaveform
              data={renderData}
              barWidth={2}
              barGap={1}
              fadeEdges
              className="h-full w-full"
            />
          )}
        </div>

        {/* Cue regions — faint colored band starting at each cue */}
        {durationMs &&
          positioned.map((cue, i) => {
            const startPct = Math.min(
              100,
              Math.max(0, ((cue.in_msec ?? 0) / durationMs) * 100),
            );
            const next = positioned[i + 1];
            const endPct = next
              ? Math.min(100, ((next.in_msec ?? 0) / durationMs) * 100)
              : 100;
            const width = Math.max(0, endPct - startPct);
            const hex = CUE_HEX[cueColor(cue.kind)] ?? "rgb(var(--text-muted))";
            return (
              <div
                key={`region-${cue.id}`}
                aria-hidden
                className="absolute top-0 bottom-0"
                style={{
                  left: `${startPct}%`,
                  width: `${width}%`,
                  background: `linear-gradient(to bottom, ${hex}1f 0%, ${hex}0a 60%, transparent 100%)`,
                }}
              />
            );
          })}

        {/* Quarter tick marks */}
        {[0.25, 0.5, 0.75].map((p) => (
          <div
            key={p}
            aria-hidden
            className="absolute bottom-0 h-1 w-px bg-edge-strong/60"
            style={{ left: `${p * 100}%` }}
          />
        ))}

        {/* Cue markers (vertical bar + top badge). Clickable when seek is wired. */}
        {durationMs &&
          positioned.map((cue) => {
            const pos = cue.in_msec ?? 0;
            const fraction = Math.min(1, Math.max(0, pos / durationMs));
            const pct = fraction * 100;
            const color = cueColor(cue.kind);
            const label = cueSlotLabel(cue.kind);
            const title = `${label} · ${cue.in_msec != null ? formatMs(cue.in_msec) : ""}${cue.comment ? ` — ${cue.comment}` : ""}`;
            return (
              <button
                key={cue.id}
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  if (onSeekFraction) onSeekFraction(fraction);
                }}
                disabled={!onSeekFraction}
                aria-label={`Jump to ${title}`}
                className="absolute top-0 bottom-0 flex flex-col items-center bg-transparent p-0 outline-none focus:ring-1 focus:ring-accent disabled:cursor-default enabled:cursor-pointer enabled:hover:brightness-125"
                style={{ left: `${pct}%`, transform: "translateX(-50%)" }}
                title={title}
              >
                {/* Top label badge */}
                <span
                  className={`mt-0.5 flex h-3 min-w-[12px] items-center justify-center rounded-sm px-[3px] font-mono text-[9px] font-bold leading-none text-base ${color}`}
                >
                  {label}
                </span>
                {/* Vertical line */}
                <div className={`mt-0.5 w-[2px] flex-1 ${color}`} />
              </button>
            );
          })}

        {/* Playhead */}
        {durationMs !== null && currentTimeSecs > 0 && (
          <div
            aria-hidden
            className="pointer-events-none absolute top-0 bottom-0 w-[2px] bg-accent"
            style={{
              left: `${Math.min(100, Math.max(0, ((currentTimeSecs * 1000) / durationMs) * 100))}%`,
              transform: "translateX(-50%)",
            }}
          />
        )}

        {/* Empty state */}
        {durationMs === null && (
          <div className="absolute inset-0 flex items-center justify-center bg-elevated/80 font-mono text-[10px] uppercase tracking-wider text-ink-faint">
            no duration
          </div>
        )}

        {/* Not-analysed-in-Rekordbox notice. Shown only when there is no
            audio file to decode, or when symphonia decode failed. */}
        {showUnanalysedNotice && durationMs !== null && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-1 bg-elevated/70 px-3 text-center">
            <span className="text-[11px] text-ink-secondary">
              {peaksError ? "Could not decode audio" : "No waveform"}
            </span>
            {peaksError && (
              <span className="font-mono text-[9px] text-ink-faint">
                {peaksError}
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

interface Props {
  track: Track | null;
  libraryPath: string;
  isPlaying: boolean;
  onTogglePlay: () => void;
  /** Current playback position in seconds (0 if not playing this track). */
  currentTime?: number;
  /** Loaded source duration in seconds (0 if unknown). */
  playbackDuration?: number;
  /** Seek the live audio sink to `secs`. */
  onSeek?: (secs: number) => void;
}

export function TrackDetailPanel({
  track,
  libraryPath,
  isPlaying,
  onTogglePlay,
  currentTime = 0,
  playbackDuration = 0,
  onSeek,
}: Props) {
  if (!track) {
    return <TrackDetailEmptyState />;
  }
  return (
    <TrackDetailContent
      key={track.id}
      track={track}
      libraryPath={libraryPath}
      isPlaying={isPlaying}
      onTogglePlay={onTogglePlay}
      currentTime={currentTime}
      playbackDuration={playbackDuration}
      onSeek={onSeek}
    />
  );
}

function TrackDetailEmptyState() {
  return (
    <aside className="flex h-full w-full flex-col items-center justify-center bg-base px-6 text-center animate-[slideInRight_150ms_ease-out]">
      <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-full border border-edge bg-surface text-ink-muted">
        <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
          <path d="M2.5 2A1.5 1.5 0 001 3.5v9A1.5 1.5 0 002.5 14h11a1.5 1.5 0 001.5-1.5v-9A1.5 1.5 0 0013.5 2h-11zM2 3.5a.5.5 0 01.5-.5h2a.5.5 0 01.5.5v9a.5.5 0 01-.5.5h-2a.5.5 0 01-.5-.5v-9zm4 0a.5.5 0 01.5-.5h2a.5.5 0 01.5.5v9a.5.5 0 01-.5.5h-2a.5.5 0 01-.5-.5v-9zm5 0a.5.5 0 01.5-.5h2a.5.5 0 01.5.5v9a.5.5 0 01-.5.5h-2a.5.5 0 01-.5-.5v-9z" />
        </svg>
      </div>
      <h3 className="text-[13px] font-semibold text-ink">No track selected</h3>
      <p className="mt-1.5 max-w-[14rem] text-[12px] leading-relaxed text-ink-muted">
        Pick a row from the library or a playlist to see its metadata, cue
        points, and playback controls.
      </p>
    </aside>
  );
}

function TrackDetailContent({
  track,
  libraryPath,
  isPlaying,
  onTogglePlay,
  currentTime,
  playbackDuration,
  onSeek,
}: {
  track: Track;
  libraryPath: string;
  isPlaying: boolean;
  onTogglePlay: () => void;
  currentTime: number;
  playbackDuration: number;
  onSeek?: (secs: number) => void;
}) {
  const {
    data: cues = [],
    isLoading: cuesLoading,
    error: cuesError,
    refetch: refetchCues,
  } = useTrackCues(libraryPath, track.id);

  const [analysis, setAnalysis] = useState<AnalysisResult | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [analyzeError, setAnalyzeError] = useState<string | null>(null);

  async function handleAnalyze() {
    setAnalyzing(true);
    setAnalyzeError(null);
    try {
      const result = await analyzeTrack(libraryPath, track.id);
      setAnalysis(result);
    } catch (e) {
      setAnalyzeError(e instanceof Error ? e.message : String(e));
    } finally {
      setAnalyzing(false);
    }
  }

  async function proposeCorrection(field: "bpm" | "musical_key") {
    if (!analysis) return;
    const newValue = field === "bpm" ? analysis.bpm : analysis.musical_key;
    const oldValue = field === "bpm" ? track.bpm : track.musical_key;
    await stageChange({
      library_path: libraryPath,
      kind: "TrackMetadataEdit",
      target_id: track.id,
      field,
      old_value: oldValue,
      new_value: newValue,
      reason: `Detected by stratum-dsp (confidence ${Math.round((field === "bpm" ? analysis.bpm_confidence : analysis.key_confidence) * 100)}%)`,
      confidence: field === "bpm" ? analysis.bpm_confidence : analysis.key_confidence,
    });
  }

  const sortedCues = [...cues].sort(
    (a, b) => (a.in_msec ?? 0) - (b.in_msec ?? 0),
  );

  return (
    <aside className="flex h-full w-full flex-col overflow-y-auto bg-base animate-[slideInRight_150ms_ease-out]">
      {/* Header */}
      <div className="border-b border-edge p-4">
        <div className="flex items-start gap-3">
          <button
            onClick={onTogglePlay}
            disabled={!track.folder_path}
            aria-label={isPlaying ? "Pause" : "Play"}
            className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-accent text-base transition-colors duration-150 hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
          >
            {isPlaying ? (
              <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
                <rect x="3" y="2" width="4" height="12" rx="1" />
                <rect x="9" y="2" width="4" height="12" rx="1" />
              </svg>
            ) : (
              <svg viewBox="0 0 16 16" fill="currentColor" className="ml-0.5 h-4 w-4">
                <path d="M4 2.5l9 5.5-9 5.5V2.5z" />
              </svg>
            )}
          </button>
          <div className="min-w-0 flex-1">
            <h2
              className="truncate text-[15px] font-semibold leading-tight tracking-tight text-ink"
              title={track.title}
            >
              {track.title}
            </h2>
            {track.artist && (
              <p className="mt-1 truncate text-[13px] text-ink-secondary">
                {track.artist}
              </p>
            )}
          </div>
          {track.folder_path && (
            <button
              onClick={handleAnalyze}
              disabled={analyzing}
              title="Analyze BPM + key from audio file"
              className="flex h-7 shrink-0 items-center gap-1 rounded border border-edge px-2 text-[11px] font-medium text-ink-secondary transition-colors hover:border-edge-strong hover:text-ink disabled:opacity-50"
            >
              {analyzing ? (
                <span className="h-3 w-3 animate-spin rounded-full border border-edge-strong border-t-accent-hover" />
              ) : (
                <svg viewBox="0 0 16 16" fill="currentColor" className="h-3 w-3 text-accent-hover">
                  <path d="M8 1a7 7 0 100 14A7 7 0 008 1zM7 5a1 1 0 112 0v3.586l2.207 2.207a1 1 0 01-1.414 1.414l-2.5-2.5A1 1 0 017 9V5z"/>
                </svg>
              )}
              Analyze
            </button>
          )}
        </div>
      </div>

      <CueEditor
        libraryPath={libraryPath}
        track={track}
        cues={sortedCues}
        positionMs={currentTime * 1000}
        onSeek={(ms) => onSeek?.(ms / 1000)}
        onChanged={() => void refetchCues()}
      />

      {/* Cue position bar */}
      <CuePositionBar
        trackId={track.id}
        libraryPath={libraryPath}
        folderPath={track.folder_path}
        cues={sortedCues}
        durationSecs={
          playbackDuration > 0 ? playbackDuration : track.duration_secs ?? null
        }
        currentTimeSecs={currentTime}
        onSeekFraction={
          onSeek && (playbackDuration > 0 || (track.duration_secs ?? 0) > 0)
            ? (f) => {
                const total =
                  playbackDuration > 0
                    ? playbackDuration
                    : (track.duration_secs as number);
                onSeek(f * total);
              }
            : undefined
        }
      />

      {/* Metadata */}
      <section className="border-b border-edge/60 px-4 pb-4 pt-4">
        <h3 className="mb-2 text-[10px] font-semibold uppercase tracking-[0.1em] text-ink-muted">
          Metadata
        </h3>
        <div className="flex flex-col">
          <MetaRow label="Album" value={track.album} />
          <MetaRow label="Genre" value={track.genre} />
          <MetaRow
            label="BPM"
            mono
            value={track.bpm != null && track.bpm > 0 ? track.bpm.toFixed(1) : null}
          />
          <MetaRow label="Key" mono value={track.musical_key} />
          <MetaRow
            label="Duration"
            mono
            value={track.duration_secs != null && track.duration_secs > 0 ? formatDuration(track.duration_secs) : null}
          />
          <MetaRow
            label="Rating"
            value={
              track.rating != null && track.rating > 0 ? (
                <span aria-label={`${track.rating} stars`}>
                  {STAR_RATINGS.slice(1).map((n) => (
                    <span key={n} className={n <= track.rating! ? "text-accent-hover" : "text-ink-faint"}>
                      ★
                    </span>
                  ))}
                </span>
              ) : null
            }
          />
          <MetaRow
            label="Year"
            mono
            value={track.release_year != null && track.release_year > 0 ? track.release_year : null}
          />
          <MetaRow label="Plays" mono value={track.dj_play_count} />
        </div>
        {track.comment && (
          <div className="mt-3 border-t border-edge/60 pt-3">
            <p className="mb-1 text-[10px] font-semibold uppercase tracking-[0.1em] text-ink-muted">
              Comment
            </p>
            <p className="text-[13px] leading-relaxed text-ink-secondary break-words">
              {track.comment}
            </p>
          </div>
        )}
      </section>

      {/* Analysis results */}
      {(analysis || analyzeError) && (
        <section className="border-b border-edge/60 px-4 pb-4 pt-4">
          <h3 className="mb-2 text-[10px] font-semibold uppercase tracking-[0.1em] text-ink-muted">
            Analysis
          </h3>
          {analyzeError ? (
            <p className="text-[12px] text-red-400">{analyzeError}</p>
          ) : analysis ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex gap-4">
                  <span className="font-mono text-[13px] tabular-nums text-ink">
                    {analysis.bpm.toFixed(1)} <span className="text-ink-muted">BPM</span>
                  </span>
                  <span className="font-mono text-[13px] text-ink">
                    {analysis.musical_key} <span className="text-ink-muted">key</span>
                  </span>
                </div>
                <span className="font-mono text-[10px] tabular-nums text-ink-faint">
                  {Math.round(analysis.confidence * 100)}%
                </span>
              </div>
              {/* Confidence bar */}
              <div className="h-1 w-full overflow-hidden rounded-full bg-elevated">
                <div
                  className="h-full rounded-full bg-accent/60"
                  style={{ width: `${Math.round(analysis.confidence * 100)}%` }}
                />
              </div>
              {analysis.cached && (
                <p className="text-[10px] text-ink-faint">from cache</p>
              )}
              {/* Propose corrections if values differ from DB */}
              {(track.bpm == null || Math.abs((track.bpm ?? 0) - analysis.bpm) > 0.5 ||
                (track.musical_key == null || track.musical_key !== analysis.musical_key)) && (
                <div className="flex gap-2 pt-1">
                  {(track.bpm == null || Math.abs((track.bpm ?? 0) - analysis.bpm) > 0.5) && (
                    <button
                      onClick={() => proposeCorrection("bpm")}
                      className="rounded border border-edge px-2 py-0.5 text-[10px] text-ink-secondary transition-colors hover:border-accent/60 hover:text-accent-hover"
                    >
                      Propose BPM {analysis.bpm.toFixed(1)}
                    </button>
                  )}
                  {(track.musical_key == null || track.musical_key !== analysis.musical_key) && (
                    <button
                      onClick={() => proposeCorrection("musical_key")}
                      className="rounded border border-edge px-2 py-0.5 text-[10px] text-ink-secondary transition-colors hover:border-accent/60 hover:text-accent-hover"
                    >
                      Propose key {analysis.musical_key}
                    </button>
                  )}
                </div>
              )}
            </div>
          ) : null}
        </section>
      )}

      {/* Hot cues */}
      <section className="px-4 py-4">
        <div className="mb-2 flex items-center justify-between">
          <h3 className="text-[10px] font-semibold uppercase tracking-[0.1em] text-ink-muted">
            Cues
          </h3>
          {sortedCues.length > 0 && (
            <span className="font-mono text-[10px] tabular-nums text-ink-faint">
              {sortedCues.length}
            </span>
          )}
        </div>
        {cuesLoading ? (
          <div className="flex justify-center py-2">
            <div className="h-4 w-4 animate-spin rounded-full border border-edge-strong border-t-accent-hover" />
          </div>
        ) : cuesError ? (
          <div className="rounded border border-red-900/50 bg-red-950/30 px-2 py-2 text-xs text-red-300">
            <p className="font-medium">Cue load failed</p>
            <p className="mt-1 break-words text-red-300/80">{cuesError.message}</p>
          </div>
        ) : sortedCues.length === 0 ? (
          <p className="text-xs text-ink-faint">No cues.</p>
        ) : (
          <div className="flex flex-col gap-0.5">
            {sortedCues.map((cue) => (
              <CueRow key={cue.id} cue={cue} />
            ))}
          </div>
        )}
      </section>
    </aside>
  );
}
