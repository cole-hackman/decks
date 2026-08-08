import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { IncomingView } from "./IncomingView";
import {
  archiveTracks,
  clearIncoming,
  listIncomingTracks,
  markIncomingReviewed,
  planDeleteFromDisk,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({
  listIncomingTracks: vi.fn(),
  clearIncoming: vi.fn(),
  archiveTracks: vi.fn(),
  markIncomingReviewed: vi.fn(),
  planDeleteFromDisk: vi.fn(),
  deleteFromDisk: vi.fn(),
  // Touched transitively via useFilterContext / TrackTable:
  listTracksWithCues: vi.fn().mockResolvedValue([]),
  listTracksInAnyPlaylist: vi.fn().mockResolvedValue([]),
  listTracksWithMissingFiles: vi.fn().mockResolvedValue([]),
  listTracks: vi.fn().mockResolvedValue([]),
}));

const TRACK = {
  id: "t1",
  title: "Fresh Track",
  artist: "X",
  album: null,
  genre: null,
  musical_key: null,
  bpm: 128,
  duration_secs: 200,
  rating: null,
  comment: null,
  folder_path: "/x.mp3",
  analysis_data_path: null,
  file_type: 1,
  sample_rate: 44100,
  bit_rate: 320,
  release_year: null,
  dj_play_count: null,
  label: null,
  remixer: null,
  mix: null,
  color: null,
  date_added: null,
  energy: null,
};

beforeEach(() => {
  vi.clearAllMocks();
});

function render_() {
  return render(
    <WithProviders>
      <IncomingView
        libraryPath="/db"
        selectedTrackIds={new Set(["t1"])}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />
    </WithProviders>,
  );
}

describe("IncomingView", () => {
  it("fetches and renders incoming tracks on mount", async () => {
    vi.mocked(listIncomingTracks).mockResolvedValue([TRACK]);
    render_();
    expect(await screen.findByText(/1 new track/)).toBeInTheDocument();
    expect(listIncomingTracks).toHaveBeenCalledWith("/db");
  });

  it("archive-selected button calls archiveTracks", async () => {
    vi.mocked(listIncomingTracks).mockResolvedValue([TRACK]);
    vi.mocked(archiveTracks).mockResolvedValue();
    render_();
    await screen.findByText(/1 new track/);
    await userEvent.click(screen.getByRole("button", { name: /Archive selected/ }));
    expect(archiveTracks).toHaveBeenCalledWith("/db", ["t1"]);
  });

  it("mark-all-reviewed opens a confirm and calls clearIncoming on Confirm", async () => {
    vi.mocked(listIncomingTracks).mockResolvedValue([TRACK]);
    vi.mocked(clearIncoming).mockResolvedValue();
    render_();
    await screen.findByText(/1 new track/);
    await userEvent.click(screen.getByRole("button", { name: /Mark all reviewed/ }));
    // Dialog opens — click its Clear button
    await userEvent.click(await screen.findByRole("button", { name: "Clear" }));
    expect(clearIncoming).toHaveBeenCalledWith("/db");
  });
});

const SECOND = { ...TRACK, id: "t2", title: "Next Track" };
const THIRD = { ...TRACK, id: "t3", title: "Third Track" };

describe("IncomingView — Selected done", () => {
  function renderWith(
    selected: string[],
    onSelectionChange = vi.fn(),
    onSelect = vi.fn(),
  ) {
    render(
      <WithProviders>
        <IncomingView
          libraryPath="/db"
          selectedTrackIds={new Set(selected)}
          onSelectionChange={onSelectionChange}
          onSelect={onSelect}
        />
      </WithProviders>,
    );
    return { onSelectionChange, onSelect };
  }

  beforeEach(() => {
    vi.mocked(listIncomingTracks).mockResolvedValue([TRACK, SECOND, THIRD]);
    vi.mocked(markIncomingReviewed).mockResolvedValue(1);
  });

  it("is unavailable with nothing selected", async () => {
    renderWith([]);
    expect(
      await screen.findByRole("button", { name: /Selected done \(0\)/ }),
    ).toBeDisabled();
  });

  it("marks the selection reviewed and advances to the next track", async () => {
    const user = userEvent.setup();
    const { onSelectionChange, onSelect } = renderWith(["t1"]);
    await user.click(
      await screen.findByRole("button", { name: /Selected done \(1\)/ }),
    );

    expect(markIncomingReviewed).toHaveBeenCalledWith("/db", ["t1"]);
    expect(onSelectionChange).toHaveBeenCalledWith(new Set(["t2"]));
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "t2" }));
  });

  it("advances past the whole selection, not just the first of it", async () => {
    const user = userEvent.setup();
    const { onSelectionChange } = renderWith(["t1", "t2"]);
    await user.click(
      await screen.findByRole("button", { name: /Selected done \(2\)/ }),
    );
    expect(onSelectionChange).toHaveBeenCalledWith(new Set(["t3"]));
  });

  it("clears the selection when there is nothing after it", async () => {
    const user = userEvent.setup();
    const { onSelectionChange, onSelect } = renderWith(["t3"]);
    await user.click(
      await screen.findByRole("button", { name: /Selected done \(1\)/ }),
    );
    expect(onSelectionChange).toHaveBeenCalledWith(new Set());
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("the D key does the same thing, so triage is one keystroke per track", async () => {
    const user = userEvent.setup();
    const { onSelectionChange } = renderWith(["t1"]);
    await screen.findByRole("button", { name: /Selected done \(1\)/ });
    await user.keyboard("d");
    expect(markIncomingReviewed).toHaveBeenCalledWith("/db", ["t1"]);
    expect(onSelectionChange).toHaveBeenCalledWith(new Set(["t2"]));
  });

  it("offers delete-from-disk, and previews before it asks", async () => {
    // Triage's third outcome. It goes through the same quarantine preview as
    // everywhere else — the button opens a plan, not a deletion.
    vi.mocked(listIncomingTracks).mockResolvedValue([TRACK]);
    vi.mocked(planDeleteFromDisk).mockResolvedValue({
      deletable: [{ track_id: "t1", source: "/x.mp3", bytes: 1024 }],
      refused: [],
      total_bytes: 1024,
      labels: { t1: "X — Fresh Track" },
      no_roots_configured: false,
    });
    const user = userEvent.setup();
    render_();
    await screen.findByText(/1 new track/);

    await user.click(screen.getByRole("button", { name: "Delete from disk" }));
    expect(
      await screen.findByRole("button", { name: /Delete 1 from disk/ }),
    ).toBeInTheDocument();
    expect(planDeleteFromDisk).toHaveBeenCalledWith(
      "/db",
      ["t1"],
      "Incoming triage",
      false,
    );
  });

  it("does not advance when marking reviewed failed", async () => {
    // Advancing past a track that is still in the inbox would lose it.
    const user = userEvent.setup();
    vi.mocked(markIncomingReviewed).mockRejectedValue(new Error("cache locked"));
    const { onSelectionChange } = renderWith(["t1"]);
    await user.click(
      await screen.findByRole("button", { name: /Selected done \(1\)/ }),
    );
    expect(await screen.findByText(/cache locked/)).toBeInTheDocument();
    expect(onSelectionChange).not.toHaveBeenCalled();
  });
});
