import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ArchiveView } from "./ArchiveView";
import {
  cleanupArchived,
  listArchivedTracks,
  selectArchived,
  unarchiveTracks,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({
  listArchivedTracks: vi.fn(),
  cleanupArchived: vi.fn(),
  selectArchived: vi.fn(),
  unarchiveTracks: vi.fn(),
  listTracksWithCues: vi.fn().mockResolvedValue([]),
  listTracksInAnyPlaylist: vi.fn().mockResolvedValue([]),
  listTracksWithMissingFiles: vi.fn().mockResolvedValue([]),
  listTracks: vi.fn().mockResolvedValue([]),
}));

const TRACK = {
  id: "t1",
  title: "Archived",
  artist: null,
  album: null,
  genre: null,
  musical_key: null,
  bpm: 0,
  duration_secs: 0,
  rating: null,
  comment: null,
  folder_path: "/a.mp3",
  analysis_data_path: null,
  file_type: 1,
  sample_rate: 0,
  bit_rate: 0,
  release_year: null,
  dj_play_count: null,
  energy: null,
};

beforeEach(() => {
  vi.clearAllMocks();
});

function render_(onSelectionChange = vi.fn()) {
  render(
    <WithProviders>
      <ArchiveView
        libraryPath="/db"
        selectedTrackIds={new Set(["t1"])}
        onSelectionChange={onSelectionChange}
        onSelect={vi.fn()}
      />
    </WithProviders>,
  );
  return onSelectionChange;
}

describe("ArchiveView", () => {
  it("loads archived tracks", async () => {
    vi.mocked(listArchivedTracks).mockResolvedValue([TRACK]);
    render_();
    expect(await screen.findByText(/1 archived track/)).toBeInTheDocument();
  });

  it("unarchive button calls unarchiveTracks", async () => {
    vi.mocked(listArchivedTracks).mockResolvedValue([TRACK]);
    vi.mocked(unarchiveTracks).mockResolvedValue();
    render_();
    await screen.findByText(/1 archived track/);
    await userEvent.click(screen.getByRole("button", { name: /Unarchive/ }));
    expect(unarchiveTracks).toHaveBeenCalledWith("/db", ["t1"]);
  });

  it("cleanup confirms first, then stages", async () => {
    vi.mocked(listArchivedTracks).mockResolvedValue([TRACK]);
    vi.mocked(cleanupArchived).mockResolvedValue(["c1", "c2"]);
    render_();
    await screen.findByText(/1 archived track/);
    await userEvent.click(
      screen.getByRole("button", { name: /Clean up selection/ }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Stage cleanup" }),
    );
    expect(cleanupArchived).toHaveBeenCalledWith("/db", ["t1"]);
  });

  it("cleanup says the audio files stay where they are", async () => {
    // The one thing a user needs to be sure of before pressing it. Deleting
    // audio is a separate button with its own preview — cleanup never does it
    // as a side effect.
    vi.mocked(listArchivedTracks).mockResolvedValue([TRACK]);
    render_();
    await screen.findByText(/1 archived track/);
    await userEvent.click(
      screen.getByRole("button", { name: /Clean up selection/ }),
    );
    expect(
      await screen.findByText(/The audio files stay where they are/),
    ).toBeInTheDocument();
  });

  it("the selection helper picks tracks by age", async () => {
    vi.mocked(listArchivedTracks).mockResolvedValue([TRACK]);
    vi.mocked(selectArchived).mockResolvedValue(["t1", "t2"]);
    const onSelectionChange = render_();
    await screen.findByText(/1 archived track/);
    await userEvent.click(
      screen.getByRole("button", { name: "Older than 6 months" }),
    );
    expect(selectArchived).toHaveBeenCalledWith("/db", {
      kind: "older_than_days",
      value: 180,
    });
    expect(onSelectionChange).toHaveBeenCalledWith(new Set(["t1", "t2"]));
  });

  it("the selection helper picks tracks with no cues and tracks in no playlist", async () => {
    vi.mocked(listArchivedTracks).mockResolvedValue([TRACK]);
    vi.mocked(selectArchived).mockResolvedValue([]);
    render_();
    await screen.findByText(/1 archived track/);
    await userEvent.click(screen.getByRole("button", { name: "Without cues" }));
    expect(selectArchived).toHaveBeenCalledWith("/db", { kind: "without_cues" });
    await userEvent.click(screen.getByRole("button", { name: "In no playlist" }));
    expect(selectArchived).toHaveBeenCalledWith("/db", { kind: "in_no_playlist" });
  });

  it("a helper that matched nothing says so rather than silently clearing", async () => {
    vi.mocked(listArchivedTracks).mockResolvedValue([TRACK]);
    vi.mocked(selectArchived).mockResolvedValue([]);
    render_();
    await screen.findByText(/1 archived track/);
    await userEvent.click(screen.getByRole("button", { name: "Without cues" }));
    expect(await screen.findByText(/0 archived track\(s\) have no cues/)).toBeInTheDocument();
  });
});
