import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Track, HotCue, AnalysisResult } from "../types";

vi.mock("../hooks/useTrackCues");
import { useTrackCues } from "../hooks/useTrackCues";

vi.mock("../ipc", () => ({
  analyzeTrack: vi.fn(),
  stageChange: vi.fn(),
  getAnlzWaveform: vi.fn(),
  getAudioWaveform: vi.fn(),
  // The panel now embeds the cue editor and cue generator, which reach for
  // these on mount.
  stageCueAdd: vi.fn(),
  stageCueDelete: vi.fn(),
  stageCueEdit: vi.fn(),
  stageGridShift: vi.fn(),
  beatJumpPosition: vi.fn(),
  suggestAnchorRules: vi.fn(),
  previewGeneratedCues: vi.fn(),
  applyGeneratedCues: vi.fn(),
}));
import {
  analyzeTrack,
  stageChange,
  getAnlzWaveform,
  getAudioWaveform,
  suggestAnchorRules,
} from "../ipc";

const EMPTY_ANLZ = { preview: [], detail: [], beat_grid: [], peaks: null };

import { TrackDetailPanel } from "./TrackDetailPanel";

const BASE_TRACK: Track = {
  id: "1",
  title: "Dark Matter",
  artist: "Surgeon",
  album: "Force + Form",
  genre: "Techno",
  musical_key: "8A",
  bpm: 140.0,
  duration_secs: 360,
  rating: 3,
  comment: "Opener banger",
  folder_path: "/music/dark-matter.mp3",
  analysis_data_path: null,
  file_type: null,
  sample_rate: null,
  bit_rate: null,
  release_year: 2014,
  dj_play_count: 5,
  energy: null,
};

const CUES: HotCue[] = [
  { id: "1", content_id: "1", in_msec: 8000, out_msec: null, kind: { HotCue: 1 }, color: null, comment: "Intro" },
  { id: "2", content_id: "1", in_msec: 45000, out_msec: null, kind: "MemoryCue", color: null, comment: null },
  { id: "3", content_id: "1", in_msec: 120500, out_msec: null, kind: { HotCue: 2 }, color: null, comment: "Drop" },
];

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient();
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

const ANALYSIS: AnalysisResult = {
  bpm: 132.0,
  musical_key: "9B",
  confidence: 0.87,
  bpm_confidence: 0.9,
  key_confidence: 0.84,
  cached: false,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(useTrackCues).mockReturnValue({
    data: CUES,
    isLoading: false,
    error: null,
  } as unknown as ReturnType<typeof useTrackCues>);
  vi.mocked(analyzeTrack).mockResolvedValue(ANALYSIS);
  vi.mocked(stageChange).mockResolvedValue(undefined as never);
  vi.mocked(getAnlzWaveform).mockResolvedValue(EMPTY_ANLZ as never);
  vi.mocked(getAudioWaveform).mockResolvedValue([]);
  vi.mocked(suggestAnchorRules).mockResolvedValue([]);
});

describe("TrackDetailPanel", () => {
  it("displays track title and artist", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByText("Dark Matter")).toBeInTheDocument();
    expect(screen.getByText("Surgeon")).toBeInTheDocument();
  });

  it("displays metadata fields", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByText("Force + Form")).toBeInTheDocument();
    expect(screen.getByText("Techno")).toBeInTheDocument();
    expect(screen.getByText("140.0")).toBeInTheDocument();
    expect(screen.getByText("8A")).toBeInTheDocument();
    // 6:00 now appears in both the Duration metadata and the cue position
    // timeline (end-of-track label), so both renders should be present.
    expect(screen.getAllByText("6:00").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("2014")).toBeInTheDocument();
    // Rating 5. The cue editor's slot buttons are also labelled 1–8, so "5"
    // now appears more than once — same situation as "6:00" above.
    expect(screen.getAllByText("5").length).toBeGreaterThanOrEqual(1);
  });

  it("displays comment", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByText("Opener banger")).toBeInTheDocument();
  });

  it("displays cue timestamps formatted as M:SS.s", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByText("0:08.0")).toBeInTheDocument();
    expect(screen.getByText("0:45.0")).toBeInTheDocument();
    expect(screen.getByText("2:00.5")).toBeInTheDocument();
  });

  it("shows slot labels for hot cues and memory cue", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    // Labels appear in both the cue list rows and the cue position timeline.
    expect(screen.getAllByText("1").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("M").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("2").length).toBeGreaterThanOrEqual(1);
  });

  it("shows cue comments", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByText("Intro")).toBeInTheDocument();
    expect(screen.getByText("Drop")).toBeInTheDocument();
  });

  it("shows 'No cues.' when cue list is empty", () => {
    vi.mocked(useTrackCues).mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    } as unknown as ReturnType<typeof useTrackCues>);
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByText("No cues.")).toBeInTheDocument();
  });

  it("shows cue load errors instead of silently hiding failures", () => {
    vi.mocked(useTrackCues).mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error("no such column: InMsec"),
    } as unknown as ReturnType<typeof useTrackCues>);
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByText(/Cue load failed/i)).toBeInTheDocument();
    expect(screen.getByText(/no such column: InMsec/i)).toBeInTheDocument();
  });

  it("shows rating stars", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByLabelText("3 stars")).toBeInTheDocument();
  });

  it("shows cue position timeline with start and end timestamps", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={() => {}} />, { wrapper });
    expect(screen.getByText("0:00")).toBeInTheDocument();
    // End timestamp matches the track duration formatting.
    expect(screen.getAllByText("6:00").length).toBeGreaterThanOrEqual(1);
  });

  it("shows play button when not playing", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
  });

  it("shows pause button when playing", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={true} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
  });

  it("calls onTogglePlay when play button is clicked", () => {
    const onTogglePlay = vi.fn();
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={onTogglePlay} />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(onTogglePlay).toHaveBeenCalledOnce();
  });

  it("disables play button when track has no folder_path", () => {
    const noPathTrack = { ...BASE_TRACK, folder_path: null };
    render(<TrackDetailPanel track={noPathTrack} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByRole("button", { name: "Play" })).toBeDisabled();
  });

  it("shows Analyze button only when track has folder_path", () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.getByRole("button", { name: /analyze/i })).toBeInTheDocument();
  });

  it("hides Analyze button when track has no folder_path", () => {
    const noPathTrack = { ...BASE_TRACK, folder_path: null };
    render(<TrackDetailPanel track={noPathTrack} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    expect(screen.queryByRole("button", { name: /analyze/i })).not.toBeInTheDocument();
  });

  it("calls analyzeTrack with correct args when Analyze is clicked", async () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: /analyze/i }));
    await waitFor(() => expect(analyzeTrack).toHaveBeenCalledWith("/tmp/master.db", "1"));
  });

  it("disables Analyze button while analysis is in flight", async () => {
    let resolve: (v: AnalysisResult) => void;
    vi.mocked(analyzeTrack).mockReturnValue(new Promise((r) => { resolve = r; }));
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: /analyze/i }));
    await waitFor(() => expect(screen.getByRole("button", { name: /analyze/i })).toBeDisabled());
    resolve!(ANALYSIS);
  });

  it("renders Analysis section with BPM and key after analysis completes", async () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: /analyze/i }));
    // The Analysis section heading only appears once analysis state is set.
    await waitFor(() => expect(screen.getByText("Analysis")).toBeInTheDocument());
    // BPM and key values appear inside inline spans alongside "BPM"/"key" labels.
    expect(screen.getByText(/132\.0.*BPM|BPM.*132\.0/s)).toBeInTheDocument();
    expect(screen.getByText(/9B.*key|key.*9B/s)).toBeInTheDocument();
  });

  it("shows 'Propose BPM' button when analysis BPM differs from track BPM", async () => {
    // BASE_TRACK.bpm is 140.0; analysis returns 132.0 — differs by >0.5
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: /analyze/i }));
    await waitFor(() => expect(screen.getByRole("button", { name: /propose bpm/i })).toBeInTheDocument());
  });

  it("shows 'Propose key' button when analysis key differs from track key", async () => {
    // BASE_TRACK.musical_key is "8A"; analysis returns "9B"
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: /analyze/i }));
    await waitFor(() => expect(screen.getByRole("button", { name: /propose key/i })).toBeInTheDocument());
  });

  it("calls stageChange with TrackMetadataEdit when Propose BPM is clicked", async () => {
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: /analyze/i }));
    await waitFor(() => screen.getByRole("button", { name: /propose bpm/i }));
    fireEvent.click(screen.getByRole("button", { name: /propose bpm/i }));
    await waitFor(() =>
      expect(stageChange).toHaveBeenCalledWith(
        expect.objectContaining({
          kind: "TrackMetadataEdit",
          target_id: "1",
          field: "bpm",
          new_value: 132.0,
        }),
      ),
    );
  });

  it("auto-fetches symphonia peaks when ANLZ data is empty and folder_path resolves", async () => {
    vi.mocked(getAnlzWaveform).mockResolvedValue(EMPTY_ANLZ as never);
    vi.mocked(getAudioWaveform).mockResolvedValue([0.1, 0.5, 0.9, 0.3]);
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    await waitFor(() =>
      expect(getAudioWaveform).toHaveBeenCalledWith("/music/dark-matter.mp3", 1200),
    );
  });

  it("does not fetch symphonia peaks when ANLZ returns real waveform data", async () => {
    vi.mocked(getAnlzWaveform).mockResolvedValue({
      preview: [{ height: 10, color: [200, 100, 50] }],
      detail: [],
      beat_grid: [],
      peaks: null,
    } as never);
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    // Give the ANLZ query a tick to settle; peaks must not fire.
    await waitFor(() => expect(getAnlzWaveform).toHaveBeenCalled());
    expect(getAudioWaveform).not.toHaveBeenCalled();
  });

  it("does not fetch symphonia peaks when track has no folder_path", async () => {
    vi.mocked(getAnlzWaveform).mockResolvedValue(EMPTY_ANLZ as never);
    const noPathTrack = { ...BASE_TRACK, folder_path: null };
    render(<TrackDetailPanel track={noPathTrack} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    await waitFor(() =>
      expect(screen.getByText(/No waveform/i)).toBeInTheDocument(),
    );
    expect(getAudioWaveform).not.toHaveBeenCalled();
  });

  it("shows decode-failure notice when symphonia decode errors", async () => {
    vi.mocked(getAnlzWaveform).mockResolvedValue(EMPTY_ANLZ as never);
    vi.mocked(getAudioWaveform).mockRejectedValue(new Error("unsupported codec"));
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    await waitFor(() =>
      expect(screen.getByText(/Could not decode audio/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/unsupported codec/i)).toBeInTheDocument();
  });

  it("does not show Propose buttons when analysis matches track values", async () => {
    const matchingAnalysis: AnalysisResult = {
      ...ANALYSIS,
      bpm: 140.0,
      musical_key: "8A",
    };
    vi.mocked(analyzeTrack).mockResolvedValue(matchingAnalysis);
    render(<TrackDetailPanel track={BASE_TRACK} libraryPath="/tmp/master.db" isPlaying={false} onTogglePlay={vi.fn()} />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: /analyze/i }));
    await waitFor(() => expect(screen.getByText(/140\.0/)).toBeInTheDocument());
    expect(screen.queryByRole("button", { name: /propose/i })).not.toBeInTheDocument();
  });
});
