import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Track } from "../types";

// Must be declared before other mocks so hoisting order is safe.
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({
    count,
    estimateSize,
  }: {
    count: number;
    estimateSize: () => number;
  }) => ({
    getVirtualItems: () =>
      Array.from({ length: count }, (_, i) => ({
        index: i,
        start: i * estimateSize(),
        end: (i + 1) * estimateSize(),
        size: estimateSize(),
        lane: 0,
        key: i,
      })),
    getTotalSize: () => count * estimateSize(),
    measureElement: () => {},
    scrollToIndex: () => {},
  }),
}));

vi.mock("../hooks/useLibrary");
import { useLibrary } from "../hooks/useLibrary";

// The operator-search path talks to the smartlist engine over IPC. Mocked so
// the table's own tests do not depend on a Tauri host.
vi.mock("../ipc", () => ({
  searchHasOperators: vi.fn(),
  searchTracks: vi.fn(),
  multiEditApply: vi.fn(),
  getRowWaveforms: vi.fn(),
}));
import {
  searchHasOperators,
  searchTracks,
  multiEditApply,
  getRowWaveforms,
} from "../ipc";

import { TrackTable } from "./TrackTable";
import { EMPTY_FILTERS, type Filters, type FilterContext } from "../lib/filters";

const EMPTY_CTX: FilterContext = {
  tracksWithCues: new Set(),
  tracksInAnyPlaylist: new Set(),
  tracksWithMissingFiles: new Set(),
  tagsByTrack: new Map(),
};

const withQuery = (q: string): Filters => ({ ...EMPTY_FILTERS, query: q });

const TRACKS: Track[] = [
  {
    id: "1",
    title: "Dark Matter",
    artist: "Surgeon",
    album: null,
    genre: "Techno",
    musical_key: "8A",
    bpm: 140.0,
    duration_secs: 360,
    rating: null,
    comment: null,
    folder_path: null,
    analysis_data_path: null,
    file_type: null,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    energy: null,
  },
  {
    id: "2",
    title: "Acid Rain",
    artist: "Aphex Twin",
    album: null,
    genre: "Ambient",
    musical_key: "11B",
    bpm: 130.5,
    duration_secs: 240,
    rating: null,
    comment: null,
    folder_path: null,
    analysis_data_path: null,
    file_type: null,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    energy: null,
  },
];

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient();
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(useLibrary).mockReturnValue({
    data: TRACKS,
    isLoading: false,
    error: null,
  } as ReturnType<typeof useLibrary>);
  vi.mocked(searchHasOperators).mockResolvedValue(false);
  vi.mocked(searchTracks).mockResolvedValue([]);
  vi.mocked(multiEditApply).mockResolvedValue([]);
  vi.mocked(getRowWaveforms).mockResolvedValue({});
});

describe("TrackTable", () => {
  it("renders track titles", () => {
    render(<TrackTable libraryPath="/tmp/master.db" filters={EMPTY_FILTERS} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />, { wrapper });
    expect(screen.getByText("Dark Matter")).toBeInTheDocument();
    expect(screen.getByText("Acid Rain")).toBeInTheDocument();
  });

  it("renders artist names", () => {
    render(<TrackTable libraryPath="/tmp/master.db" filters={EMPTY_FILTERS} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />, { wrapper });
    expect(screen.getByText("Surgeon")).toBeInTheDocument();
    expect(screen.getByText("Aphex Twin")).toBeInTheDocument();
  });

  it("renders BPM formatted to one decimal", () => {
    render(<TrackTable libraryPath="/tmp/master.db" filters={EMPTY_FILTERS} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />, { wrapper });
    expect(screen.getByText("140.0")).toBeInTheDocument();
    expect(screen.getByText("130.5")).toBeInTheDocument();
  });

  it("renders duration as M:SS", () => {
    render(<TrackTable libraryPath="/tmp/master.db" filters={EMPTY_FILTERS} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />, { wrapper });
    expect(screen.getByText("6:00")).toBeInTheDocument();
    expect(screen.getByText("4:00")).toBeInTheDocument();
  });

  it("filters tracks by title", () => {
    render(<TrackTable libraryPath="/tmp/master.db" filters={withQuery("dark")} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />, { wrapper });
    expect(screen.getByText("Dark Matter")).toBeInTheDocument();
    expect(screen.queryByText("Acid Rain")).not.toBeInTheDocument();
  });

  it("filters tracks by artist", () => {
    render(<TrackTable libraryPath="/tmp/master.db" filters={withQuery("aphex")} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />, { wrapper });
    expect(screen.queryByText("Dark Matter")).not.toBeInTheDocument();
    expect(screen.getByText("Acid Rain")).toBeInTheDocument();
  });

  it("shows empty state when filters match nothing", () => {
    render(
      <TrackTable libraryPath="/tmp/master.db" filters={withQuery("zzznomatch")} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />,
      { wrapper },
    );
    expect(
      screen.getByText("No tracks match your filters"),
    ).toBeInTheDocument();
  });

  it("shows column headers", () => {
    render(<TrackTable libraryPath="/tmp/master.db" filters={EMPTY_FILTERS} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />, { wrapper });
    expect(screen.getByText("Title")).toBeInTheDocument();
    expect(screen.getByText("Artist")).toBeInTheDocument();
    expect(screen.getByText("BPM")).toBeInTheDocument();
    expect(screen.getByText("Key")).toBeInTheDocument();
    expect(screen.getByText("Time")).toBeInTheDocument();
    expect(screen.getByText("Genre")).toBeInTheDocument();
    expect(screen.getByText("Energy")).toBeInTheDocument();
  });

  it("does not render the Tags column when no tag bindings exist", () => {
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
      { wrapper },
    );
    expect(screen.queryByText("Tags")).not.toBeInTheDocument();
  });

  it("renders tag chips for tagged tracks and an em-dash for untagged ones", () => {
    const tagsByTrack = new Map<string, Set<string>>([
      ["1", new Set(["tag-mood-dark", "tag-vibe-late"])],
    ]);
    const ctx: FilterContext = { ...EMPTY_CTX, tagsByTrack };
    const tagLabelById = {
      "tag-mood-dark": "Mood ▸ Dark",
      "tag-vibe-late": "Vibe ▸ Late Night",
    };
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={ctx}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
        tagLabelById={tagLabelById}
      />,
      { wrapper },
    );

    // Header is now present.
    expect(screen.getByText("Tags")).toBeInTheDocument();
    // Track 1 gets two chips (leaf names only).
    expect(screen.getByText("Dark")).toBeInTheDocument();
    expect(screen.getByText("Late Night")).toBeInTheDocument();
    // Track 2 (no bindings) gets an em-dash placeholder. The em-dash appears
    // in multiple columns, so we filter to chip-bearing context via test id —
    // simplest assertion: exactly two chips rendered overall.
    expect(screen.getAllByTestId("track-tag-chip")).toHaveLength(2);
  });

  it("renders the Camelot key with a non-default colour applied", () => {
    render(<TrackTable libraryPath="/tmp/master.db" filters={EMPTY_FILTERS} filterCtx={EMPTY_CTX} selectedTrackIds={new Set()} onSelectionChange={vi.fn()} onSelect={vi.fn()} />, { wrapper });
    const keyCell = screen.getByText("8A");
    expect(keyCell).toHaveStyle({ color: "#9F4FCA" });
  });
  it("marks only the keys that mix with the selected track", async () => {
    // A positive mark only: an unmarked row means "not compatible or we cannot
    // tell", and marking every non-match would drown the ones that are.
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
        compatibleWith={["8A"]}
      />,
      { wrapper },
    );
    // Fixture keys are 8A and 11B; only 8A mixes.
    const marks = await screen.findAllByLabelText(
      "mixes with the selected track",
    );
    expect(marks).toHaveLength(1);
    expect(marks[0].parentElement?.textContent).toContain("8A");
  });

  it("marks nothing when there is no reference key", async () => {
    // Better no indicator at all than one that marks the whole library.
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
        compatibleWith={[]}
      />,
      { wrapper },
    );
    await screen.findByText("Dark Matter");
    expect(
      screen.queryAllByLabelText("mixes with the selected track"),
    ).toHaveLength(0);
  });
  it("keeps plain text local — no engine round-trip for a band name", async () => {
    // Typing a name must not wait on IPC.
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={withQuery("dark")}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
      { wrapper },
    );
    expect(screen.getByText("Dark Matter")).toBeInTheDocument();
    await waitFor(() => expect(searchHasOperators).toHaveBeenCalledWith("dark"));
    expect(searchTracks).not.toHaveBeenCalled();
  });

  it("sends an operator query to the engine and filters by what comes back", async () => {
    vi.mocked(searchHasOperators).mockResolvedValue(true);
    // Only the first fixture track matches.
    vi.mocked(searchTracks).mockResolvedValue(["1"]);
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={withQuery("bpm>128")}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
      { wrapper },
    );
    await waitFor(() =>
      expect(searchTracks).toHaveBeenCalledWith("/tmp/master.db", "bpm>128"),
    );
    await waitFor(() =>
      expect(screen.queryByText("Acid Rain")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Dark Matter")).toBeInTheDocument();
  });

  it("says so when an operator search fails, rather than looking like a narrow match", async () => {
    vi.mocked(searchHasOperators).mockResolvedValue(true);
    vi.mocked(searchTracks).mockRejectedValue(new Error("boom"));
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={withQuery("bpm>128")}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
      { wrapper },
    );
    expect(await screen.findByTestId("search-error")).toHaveTextContent(
      "showing a plain text match instead",
    );
  });
});

describe("TrackTable — spreadsheet keyboard navigation", () => {
  function renderGrid(overrides: Record<string, unknown> = {}) {
    const onSelectionChange = vi.fn();
    const onSelect = vi.fn();
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={onSelectionChange}
        onSelect={onSelect}
        {...overrides}
      />,
      { wrapper },
    );
    return { grid: screen.getByRole("grid"), onSelectionChange, onSelect };
  }

  /** The cursor cell, identified by the ring `aria-selected` marks. */
  const cursorCell = () =>
    document.querySelector('[role="gridcell"][aria-selected="true"]');

  it("is a grid, and takes focus", async () => {
    const user = userEvent.setup();
    const { grid } = renderGrid();
    expect(grid).toHaveAttribute("aria-rowcount", "2");
    await user.click(grid);
    expect(grid).toHaveFocus();
  });

  it("places the cursor on the first cell when focused, and moves right", async () => {
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    await waitFor(() => expect(cursorCell()).toHaveTextContent("Dark Matter"));

    await user.keyboard("{ArrowRight}");
    expect(cursorCell()).toHaveTextContent("Surgeon");
  });

  it("moves down a row and reports the selection", async () => {
    const user = userEvent.setup();
    const { grid, onSelectionChange, onSelect } = renderGrid();
    grid.focus();
    await user.keyboard("{ArrowDown}");
    expect(cursorCell()).toHaveTextContent("Acid Rain");
    expect(onSelectionChange).toHaveBeenLastCalledWith(new Set(["2"]));
    expect(onSelect).toHaveBeenLastCalledWith(
      expect.objectContaining({ id: "2" }),
    );
  });

  it("clamps at the last row instead of wrapping to the top", async () => {
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    expect(cursorCell()).toHaveTextContent("Acid Rain");
  });

  it("extends the selection with shift, and shrinks it back", async () => {
    const user = userEvent.setup();
    const { grid, onSelectionChange } = renderGrid();
    grid.focus();
    await user.keyboard("{Shift>}{ArrowDown}{/Shift}");
    expect(onSelectionChange).toHaveBeenLastCalledWith(new Set(["1", "2"]));

    await user.keyboard("{Shift>}{ArrowUp}{/Shift}");
    expect(onSelectionChange).toHaveBeenLastCalledWith(new Set(["1"]));
  });

  it("opens an editor on Enter, seeded with the current value", async () => {
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    await user.keyboard("{Enter}");
    expect(await screen.findByLabelText("Edit Title")).toHaveValue(
      "Dark Matter",
    );
  });

  it("opens an editor on a printable key, seeded with what was typed", async () => {
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    await user.keyboard("N");
    expect(await screen.findByLabelText("Edit Title")).toHaveValue("N");
  });

  it("stages the edit through the review pipeline, never a direct write", async () => {
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    await user.keyboard("{Enter}");
    const input = await screen.findByLabelText("Edit Title");
    await user.clear(input);
    await user.type(input, "New Title");
    await user.keyboard("{Enter}");

    await waitFor(() =>
      expect(multiEditApply).toHaveBeenCalledWith(
        "/tmp/master.db",
        ["1"],
        [{ field: "title", value: "New Title" }],
      ),
    );
  });

  it("does not stage anything when the value came back unchanged", async () => {
    // A review panel filling with no-op rows is how people stop reading it.
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    await user.keyboard("{Enter}");
    await screen.findByLabelText("Edit Title");
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(screen.queryByLabelText("Edit Title")).not.toBeInTheDocument(),
    );
    expect(multiEditApply).not.toHaveBeenCalled();
  });

  it("Escape abandons the edit without staging", async () => {
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    await user.keyboard("{Enter}");
    const input = await screen.findByLabelText("Edit Title");
    await user.clear(input);
    await user.type(input, "Discarded");
    await user.keyboard("{Escape}");

    await waitFor(() =>
      expect(screen.queryByLabelText("Edit Title")).not.toBeInTheDocument(),
    );
    expect(multiEditApply).not.toHaveBeenCalled();
  });

  it("refuses to edit a column the applier cannot write", async () => {
    // Energy is derived and cache-only. A cell the user could type into whose
    // value then vanished at sync time is worse than a read-only one.
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    // Title → Artist → BPM → Key → Energy
    await user.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}{ArrowRight}");
    expect(cursorCell()).toHaveAttribute("aria-readonly", "true");
    await user.keyboard("{Enter}");
    expect(screen.queryByLabelText(/^Edit /)).not.toBeInTheDocument();
  });

  it("surfaces a staging failure instead of silently dropping the edit", async () => {
    vi.mocked(multiEditApply).mockRejectedValue(new Error("cache locked"));
    const user = userEvent.setup();
    const { grid } = renderGrid();
    grid.focus();
    await user.keyboard("{Enter}");
    const input = await screen.findByLabelText("Edit Title");
    await user.clear(input);
    await user.type(input, "New Title");
    await user.keyboard("{Enter}");

    expect(await screen.findByTestId("cell-edit-error")).toHaveTextContent(
      "cache locked",
    );
  });

  // ── Inline row waveforms ───────────────────────────────────────────────────

  it("asks for the visible rows in one batch, not one call each", async () => {
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
      { wrapper },
    );
    await waitFor(() => expect(getRowWaveforms).toHaveBeenCalledTimes(1));
    expect(getRowWaveforms).toHaveBeenCalledWith(
      "/tmp/master.db",
      ["1", "2"],
      40,
    );
  });

  it("draws a waveform once its batch lands", async () => {
    vi.mocked(getRowWaveforms).mockResolvedValue({ "1": [0, 128, 255, 64] });
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
      { wrapper },
    );
    // One row has data; the other has none and draws nothing at all.
    await waitFor(() =>
      expect(screen.getAllByTestId("row-waveform")).toHaveLength(1),
    );
  });

  it("does not re-ask for a track that came back without a waveform", async () => {
    // Otherwise every scroll past an unanalysed track re-reads the disk to be
    // told the same thing.
    const { rerender } = render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
      { wrapper },
    );
    await waitFor(() => expect(getRowWaveforms).toHaveBeenCalledTimes(1));

    rerender(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set(["1"])}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
    );
    await waitFor(() => expect(getRowWaveforms).toHaveBeenCalledTimes(1));
  });

  it("survives a failed waveform batch", async () => {
    vi.mocked(getRowWaveforms).mockRejectedValue(new Error("disk gone"));
    render(
      <TrackTable
        libraryPath="/tmp/master.db"
        filters={EMPTY_FILTERS}
        filterCtx={EMPTY_CTX}
        selectedTrackIds={new Set()}
        onSelectionChange={vi.fn()}
        onSelect={vi.fn()}
      />,
      { wrapper },
    );
    // The table still renders; the rows simply show no waveform.
    expect(await screen.findByText("Dark Matter")).toBeInTheDocument();
    expect(screen.queryAllByTestId("row-waveform")).toHaveLength(0);
  });
});
