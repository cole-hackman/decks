import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DuplicatesView } from "./DuplicatesView";
import {
  listLibraryDuplicateGroups,
  planDuplicateResolution,
  preselectKeepers,
  resolveDuplicates,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { DuplicateGroup, Track } from "../types";

vi.mock("../ipc", () => ({
  listLibraryDuplicateGroups: vi.fn(),
  planDuplicateResolution: vi.fn(),
  resolveDuplicates: vi.fn(),
  preselectKeepers: vi.fn(),
}));

function track(id: string, title: string): Track {
  return {
    id,
    title,
    artist: "Artist",
    album: null,
    genre: null,
    musical_key: null,
    bpm: 128,
    duration_secs: 300,
    rating: null,
    comment: null,
    folder_path: `/music/${id}.mp3`,
    analysis_data_path: null,
    file_type: 1,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    label: null,
    remixer: null,
    mix: null,
    color: null,
    date_added: null,
    energy: null,
  };
}

const GROUPS: DuplicateGroup[] = [
  {
    title: "Strobe",
    artist: "Deadmau5",
    tracks: [track("e1", "Strobe"), track("e2", "Strobe")],
    kind: "ExactTitleArtist",
    confidence: 1.0,
  },
  {
    title: "Anthem",
    artist: "A",
    tracks: [
      track("f1", "Anthem"),
      track("f2", "Anthem (Original Mix)"),
      track("f3", "Anthem (Extended)"),
    ],
    kind: "FuzzyTitle",
    confidence: 0.85,
  },
  {
    title: "Sample (Audio Match)",
    artist: "X",
    tracks: [track("a1", "Sample A"), track("a2", "Sample B")],
    kind: "AudioFingerprint",
    confidence: 0.93,
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(planDuplicateResolution).mockResolvedValue({
    keeper_id: "f1",
    loser_ids: ["f2", "f3"],
    repoint: [],
    already_present: [],
  });
  vi.mocked(resolveDuplicates).mockResolvedValue({ archived: [], staged: [] });
  vi.mocked(preselectKeepers).mockResolvedValue([]);
});

function render_() {
  return render(
    <WithProviders>
      <DuplicatesView libraryPath="/db" onOpenInspector={vi.fn()} />
    </WithProviders>,
  );
}

describe("DuplicatesView", () => {
  it("renders one section per group with kind labels", async () => {
    vi.mocked(listLibraryDuplicateGroups).mockResolvedValue(GROUPS);
    render_();
    const sections = await screen.findAllByTestId("duplicate-group");
    expect(sections).toHaveLength(3);
    expect(screen.getByText("Exact title + artist")).toBeInTheDocument();
    expect(screen.getByText("Fuzzy title match")).toBeInTheDocument();
    expect(screen.getByText("Audio fingerprint match")).toBeInTheDocument();
  });

  it("archives the non-kept tracks when Keep one, archive rest is clicked", async () => {
    vi.mocked(listLibraryDuplicateGroups).mockResolvedValue([GROUPS[1]]);
    render_();
    const section = await screen.findByTestId("duplicate-group");
    // Default keep = first track (f1). Pick the first track explicitly anyway
    // to confirm the radio control is wired.
    const radios = within(section).getAllByRole("radio");
    await userEvent.click(radios[0]);
    await userEvent.click(within(section).getByTestId("archive-rest"));
    await waitFor(() => {
      expect(planDuplicateResolution).toHaveBeenCalledWith("/db", "f1", [
        "f2",
        "f3",
      ]);
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Archive and re-point" }),
    );
    await waitFor(() => expect(resolveDuplicates).toHaveBeenCalled());
  });

  it("the review step names the playlists that will be re-pointed", async () => {
    // Archiving a loser without re-pointing leaves a hole in every set it was
    // in — so the confirm has to say which sets are affected.
    vi.mocked(listLibraryDuplicateGroups).mockResolvedValue([GROUPS[1]]);
    vi.mocked(planDuplicateResolution).mockResolvedValue({
      keeper_id: "f1",
      loser_ids: ["f2", "f3"],
      repoint: [["p1", "Techno Set", "f2"]],
      already_present: [],
    });
    render_();
    const section = await screen.findByTestId("duplicate-group");
    await userEvent.click(within(section).getByTestId("archive-rest"));
    expect(await screen.findByText(/Techno Set/)).toBeInTheDocument();
  });

  it("says so when no playlist needs re-pointing", async () => {
    vi.mocked(listLibraryDuplicateGroups).mockResolvedValue([GROUPS[1]]);
    render_();
    const section = await screen.findByTestId("duplicate-group");
    await userEvent.click(within(section).getByTestId("archive-rest"));
    expect(
      await screen.findByText(/No playlist held a duplicate/),
    ).toBeInTheDocument();
  });

  it("cancelling the review archives nothing", async () => {
    vi.mocked(listLibraryDuplicateGroups).mockResolvedValue([GROUPS[1]]);
    render_();
    const section = await screen.findByTestId("duplicate-group");
    await userEvent.click(within(section).getByTestId("archive-rest"));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(resolveDuplicates).not.toHaveBeenCalled();
  });

  it("a Prefer rule preselects the keeper in every group", async () => {
    vi.mocked(listLibraryDuplicateGroups).mockResolvedValue([GROUPS[1]]);
    vi.mocked(preselectKeepers).mockResolvedValue(["f3"]);
    render_();
    await screen.findByTestId("duplicate-group");
    await userEvent.selectOptions(
      screen.getByLabelText("Prefer rule"),
      "highest_bitrate",
    );
    await waitFor(() => {
      expect(preselectKeepers).toHaveBeenCalledWith(
        expect.any(Array),
        "highest_bitrate",
      );
    });
    // The chosen keeper becomes the selected radio, so the next archive keeps it.
    const section = screen.getByTestId("duplicate-group");
    await userEvent.click(within(section).getByTestId("archive-rest"));
    await waitFor(() => {
      expect(planDuplicateResolution).toHaveBeenCalledWith("/db", "f3", [
        "f1",
        "f2",
      ]);
    });
  });

  it("renders empty state when no duplicates exist", async () => {
    vi.mocked(listLibraryDuplicateGroups).mockResolvedValue([]);
    render_();
    expect(
      await screen.findByText(/No duplicate candidates found/i),
    ).toBeInTheDocument();
  });

  it("Open in inspector callback fires per row", async () => {
    vi.mocked(listLibraryDuplicateGroups).mockResolvedValue([GROUPS[0]]);
    const onOpen = vi.fn();
    render(
      <WithProviders>
        <DuplicatesView libraryPath="/db" onOpenInspector={onOpen} />
      </WithProviders>,
    );
    const buttons = await screen.findAllByTestId("open-inspector");
    await userEvent.click(buttons[0]);
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen.mock.calls[0][0].id).toBe("e1");
  });
});
