import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PlaylistToolsView } from "./PlaylistToolsView";
import {
  applyPlaylistMerge,
  applyPlaylistPrefix,
  applyPlaylistSort,
  applyRewriteOrder,
  getPlaylist,
  listPlaylists,
  previewCrossReference,
  previewPlaylistMerge,
  previewPlaylistPrefix,
  previewPlaylistSort,
  previewRewriteOrder,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { Playlist, Track } from "../types";

vi.mock("../ipc", () => ({
  listPlaylists: vi.fn(),
  getPlaylist: vi.fn(),
  previewPlaylistMerge: vi.fn(),
  applyPlaylistMerge: vi.fn(),
  previewPlaylistSort: vi.fn(),
  applyPlaylistSort: vi.fn(),
  previewCrossReference: vi.fn(),
  previewPlaylistPrefix: vi.fn(),
  applyPlaylistPrefix: vi.fn(),
  previewRewriteOrder: vi.fn(),
  applyRewriteOrder: vi.fn(),
}));

const PLAYLISTS: Playlist[] = [
  { id: "f1", name: "Sets", parent_id: null, seq: 1, kind: "Folder" },
  { id: "p1", name: "Warmup", parent_id: "f1", seq: 1, kind: "Playlist" },
  { id: "p2", name: "Peak", parent_id: "f1", seq: 2, kind: "Playlist" },
];

function track(id: string, title: string, energy: number | null): Track {
  return {
    id,
    title,
    artist: "Someone",
    album: null,
    genre: null,
    musical_key: null,
    bpm: 128,
    duration_secs: null,
    rating: null,
    comment: null,
    folder_path: null,
    analysis_data_path: null,
    file_type: null,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    energy,
  } as Track;
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listPlaylists).mockResolvedValue(PLAYLISTS);
  vi.mocked(previewPlaylistMerge).mockResolvedValue({
    track_ids: ["1", "2", "3"],
    source_rows: 5,
  });
  vi.mocked(applyPlaylistMerge).mockResolvedValue(["c1", "c2"]);
  vi.mocked(previewPlaylistSort).mockResolvedValue({
    order: [
      ["p2", "Peak"],
      ["p1", "Warmup"],
    ],
    unchanged: false,
  });
  vi.mocked(applyPlaylistSort).mockResolvedValue("c1");
  vi.mocked(previewCrossReference).mockResolvedValue({
    track_ids: ["2"],
    considered: 4,
  });
  vi.mocked(previewPlaylistPrefix).mockResolvedValue([
    { id: "p1", from: "Warmup", to: "01 - Warmup" },
  ]);
  vi.mocked(applyPlaylistPrefix).mockResolvedValue(["c1"]);
  vi.mocked(getPlaylist).mockResolvedValue({
    playlist: PLAYLISTS[1],
    tracks: [track("1", "Loud", 9), track("2", "Quiet", 2), track("3", "Unknown", null)],
  });
  vi.mocked(previewRewriteOrder).mockResolvedValue({
    playlist_id: "p1",
    order: ["2", "1", "3"],
    unknown: [],
    appended: [],
    unchanged: false,
  });
  vi.mocked(applyRewriteOrder).mockResolvedValue("c1");
});

function renderView() {
  render(
    <WithProviders>
      <PlaylistToolsView libraryPath="/lib.db" />
    </WithProviders>,
  );
}

/** Testing Library matches an accessible name given as a string in full, so
 *  "Sort" reaches the tab and not "Preview sort". */
async function pickTool(name: string) {
  await userEvent.click(screen.getByRole("button", { name }));
}

describe("PlaylistToolsView", () => {
  it("lists playlists but not folders in the picker", async () => {
    renderView();
    const picker = await screen.findByTestId("playlist-picker");
    expect(picker).toHaveTextContent("Warmup");
    expect(picker).toHaveTextContent("Peak");
    // Folders are not things you merge or rename with a prefix.
    expect(picker).not.toHaveTextContent("Sets");
  });

  it("merge needs two playlists and reports the duplicates it dropped", async () => {
    renderView();
    await screen.findByTestId("playlist-picker");
    expect(screen.getByRole("button", { name: "Preview merge" })).toBeDisabled();

    await userEvent.click(screen.getByRole("checkbox", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Peak" }));
    await userEvent.click(screen.getByRole("button", { name: "Preview merge" }));

    // "3 tracks" alone hides that two rows were duplicates.
    expect(await screen.findByTestId("merge-preview")).toHaveTextContent(
      "3 track(s) from 5 row(s) — 2 duplicate(s) dropped. Sources are left alone.",
    );
  });

  it("merge will not stage without a name", async () => {
    renderView();
    await screen.findByTestId("playlist-picker");
    await userEvent.click(screen.getByRole("checkbox", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Peak" }));
    await userEvent.click(screen.getByRole("button", { name: "Preview merge" }));
    await screen.findByTestId("merge-preview");

    expect(screen.getByRole("button", { name: /Stage playlist/ })).toBeDisabled();
    await userEvent.type(
      screen.getByRole("textbox", { name: "Merged playlist name" }),
      "Combined",
    );
    await userEvent.click(screen.getByRole("button", { name: /Stage playlist/ }));
    expect(applyPlaylistMerge).toHaveBeenCalledWith("/lib.db", "Combined", null, [
      "1",
      "2",
      "3",
    ]);
  });

  it("changing the selection drops a stale preview", async () => {
    // A preview computed from a different selection is a wrong answer that
    // still looks like an answer.
    renderView();
    await screen.findByTestId("playlist-picker");
    await userEvent.click(screen.getByRole("checkbox", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Peak" }));
    await userEvent.click(screen.getByRole("button", { name: "Preview merge" }));
    await screen.findByTestId("merge-preview");

    await userEvent.click(screen.getByRole("checkbox", { name: "Peak" }));
    await waitFor(() =>
      expect(screen.queryByTestId("merge-preview")).not.toBeInTheDocument(),
    );
  });

  it("sort previews the new playlist order and stages it", async () => {
    renderView();
    await pickTool("Sort");
    await userEvent.click(screen.getByRole("button", { name: "Preview sort" }));

    const preview = await screen.findByTestId("sort-preview");
    expect(preview).toHaveTextContent("Peak");
    await userEvent.click(screen.getByRole("button", { name: "Stage order" }));
    expect(applyPlaylistSort).toHaveBeenCalledWith("/lib.db", null, ["p2", "p1"]);
  });

  it("sort stages nothing when the order already matches", async () => {
    vi.mocked(previewPlaylistSort).mockResolvedValue({
      order: [["p1", "Warmup"]],
      unchanged: true,
    });
    renderView();
    await pickTool("Sort");
    await userEvent.click(screen.getByRole("button", { name: "Preview sort" }));
    expect(await screen.findByTestId("sort-preview")).toHaveTextContent(
      "Already in that order",
    );
    expect(screen.getByRole("button", { name: "Stage order" })).toBeDisabled();
  });

  it("cross reference warns before the mode that can return the library", async () => {
    renderView();
    await pickTool("Cross Reference");
    expect(screen.queryByTestId("xref-warning")).not.toBeInTheDocument();

    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Cross reference mode" }),
      "in_none",
    );
    expect(screen.getByTestId("xref-warning")).toHaveTextContent(
      /can return most of the library/,
    );
  });

  it("cross reference reports the match count against what it weighed", async () => {
    renderView();
    await screen.findByTestId("playlist-picker");
    await pickTool("Cross Reference");
    await userEvent.click(screen.getByRole("checkbox", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("button", { name: "Run cross reference" }));
    expect(await screen.findByTestId("xref-result")).toHaveTextContent(
      "1 of 4 track(s) match across 1 playlist(s).",
    );
  });

  it("prefix numbering is hidden until asked for and follows tick order", async () => {
    renderView();
    await screen.findByTestId("playlist-picker");
    await pickTool("Prefix");
    expect(screen.queryByTestId("numbering")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("checkbox", { name: "Peak" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Number them" }));
    expect(screen.getByTestId("numbering")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Preview names" }));
    // Peak was ticked first, so it is the playlist numbered first.
    expect(previewPlaylistPrefix).toHaveBeenCalledWith(
      "/lib.db",
      ["p2", "p1"],
      expect.objectContaining({
        numbering: { start: 1, pad: 2, replace_existing: true },
      }),
    );
  });

  it("prefix says so when every name is already right", async () => {
    vi.mocked(previewPlaylistPrefix).mockResolvedValue([]);
    renderView();
    await screen.findByTestId("playlist-picker");
    await pickTool("Prefix");
    await userEvent.click(screen.getByRole("checkbox", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("button", { name: "Preview names" }));
    expect(await screen.findByTestId("prefix-preview")).toHaveTextContent(
      "Every name is already what it would become",
    );
    expect(
      screen.getByRole("button", { name: /Stage 0 rename/ }),
    ).toBeDisabled();
  });

  it("rewrite order sorts by the chosen field before asking the backend", async () => {
    renderView();
    await screen.findByTestId("playlist-picker");
    await pickTool("Rewrite Order");
    await userEvent.click(screen.getByRole("radio", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("button", { name: "Preview order" }));

    // Energy ascending: Quiet(2), Loud(9), then the un-analysed track last.
    await waitFor(() =>
      expect(previewRewriteOrder).toHaveBeenCalledWith("/lib.db", "p1", [
        "2",
        "1",
        "3",
      ]),
    );
  });

  it("rewrite order sorts tracks with no value last in either direction", async () => {
    renderView();
    await screen.findByTestId("playlist-picker");
    await pickTool("Rewrite Order");
    await userEvent.click(screen.getByRole("radio", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Descending" }));
    await userEvent.click(screen.getByRole("button", { name: "Preview order" }));

    // Descending flips Loud/Quiet but the un-analysed track stays at the end —
    // it should not lead a set just because null compares low.
    await waitFor(() =>
      expect(previewRewriteOrder).toHaveBeenCalledWith("/lib.db", "p1", [
        "1",
        "2",
        "3",
      ]),
    );
  });

  it("rewrite order says when tracks were appended rather than dropped", async () => {
    vi.mocked(previewRewriteOrder).mockResolvedValue({
      playlist_id: "p1",
      order: ["2", "1", "3"],
      unknown: [],
      appended: ["3"],
      unchanged: false,
    });
    renderView();
    await screen.findByTestId("playlist-picker");
    await pickTool("Rewrite Order");
    await userEvent.click(screen.getByRole("radio", { name: "Warmup" }));
    await userEvent.click(screen.getByRole("button", { name: "Preview order" }));
    expect(await screen.findByTestId("rewrite-appended")).toHaveTextContent(
      "1 track(s) were not in the sorted view and were appended rather than dropped.",
    );
  });

  it("each tool says what it does before you use it", async () => {
    renderView();
    expect(screen.getByTestId("tool-blurb")).toHaveTextContent(
      /Combine playlists into one new playlist/,
    );
    await pickTool("Rewrite Order");
    expect(screen.getByTestId("tool-blurb")).toHaveTextContent(
      /so it reaches the CDJ that way/,
    );
  });
});
