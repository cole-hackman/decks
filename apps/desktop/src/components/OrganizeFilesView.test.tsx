import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OrganizeFilesView } from "./OrganizeFilesView";
import {
  applyOrganize,
  patternFields,
  previewOrganize,
  validatePattern,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { OrganizeRow, Track } from "../types";

vi.mock("../ipc", () => ({
  // Used by the path-rewrite section mounted below.
  previewPathRewrite: vi.fn(async () => ({ plan: { rewrites: [], skipped: [] }, considered: 0 })),
  applyPathRewrite: vi.fn(),
  patternFields: vi.fn(),
  validatePattern: vi.fn(),
  previewOrganize: vi.fn(),
  applyOrganize: vi.fn(),
  // Used by the panels mounted below Move & Rename.
  listQuickMoveFolders: vi.fn(async () => []),
  recordQuickMoveFolder: vi.fn(),
  toggleQuickMoveFavourite: vi.fn(),
  deleteQuickMoveFolder: vi.fn(),
  writeTagsBulk: vi.fn(),
  scanUnusedFiles: vi.fn(),
  deleteUnusedFiles: vi.fn(),
  listWatchFolders: vi.fn(async () => []),
  addWatchFolder: vi.fn(),
  removeWatchFolder: vi.fn(),
  scanArrivals: vi.fn(async () => ({ arrivals: [], pending: [], errors: [] })),
  stageArrivalImports: vi.fn(),
  dismissArrivals: vi.fn(),
  clearDismissedArrivals: vi.fn(),
}));

function track(id: string, title: string): Track {
  return {
    id,
    title,
    artist: "Daft Punk",
    album: null,
    genre: "House",
    musical_key: null,
    bpm: 128,
    duration_secs: 300,
    rating: null,
    comment: null,
    folder_path: `/Incoming/${id}.mp3`,
    analysis_data_path: null,
    file_type: 1,
    sample_rate: null,
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
}

const TRACKS = [track("t1", "Get Lucky"), track("t2", "Instant Crush")];

const ROWS: OrganizeRow[] = [
  {
    track_id: "t1",
    source: "/Incoming/t1.mp3",
    destination: "/Music/House/Daft Punk - Get Lucky.mp3",
    title: "Get Lucky",
    artist: "Daft Punk",
  },
  {
    track_id: "t2",
    source: "/Music/House/Daft Punk - Instant Crush.mp3",
    destination: null,
    title: "Instant Crush",
    artist: "Daft Punk",
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(patternFields).mockResolvedValue([
    { name: "artist", supported: true },
    { name: "title", supported: true },
    { name: "remixer", supported: false },
  ]);
  vi.mocked(validatePattern).mockResolvedValue(["artist", "title"]);
  vi.mocked(previewOrganize).mockResolvedValue(ROWS);
  vi.mocked(applyOrganize).mockResolvedValue({
    moved: ["t1"],
    failed: [],
    staged: ["c1"],
  });
});

function renderView(selected: string[] = []) {
  render(
    <WithProviders>
      <OrganizeFilesView
        libraryPath="/lib.db"
        tracks={TRACKS}
        selectedTrackIds={new Set(selected)}
      />
    </WithProviders>,
  );
}

describe("OrganizeFilesView", () => {
  it("targets the selection when there is one", async () => {
    const user = userEvent.setup();
    renderView(["t2"]);
    expect(
      await screen.findByText(/1 selected track\(s\) — everything here writes to disk/),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => {
      expect(previewOrganize).toHaveBeenCalledWith(
        "/lib.db",
        ["t2"],
        expect.anything(),
      );
    });
  });

  it("falls back to every track when nothing is selected", async () => {
    const user = userEvent.setup();
    renderView();
    expect(await screen.findByText(/^All 2 track\(s\)/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => {
      expect(previewOrganize).toHaveBeenCalledWith(
        "/lib.db",
        ["t1", "t2"],
        expect.anything(),
      );
    });
  });

  it("sends an empty target folder as null, meaning rename in place", async () => {
    const user = userEvent.setup();
    renderView();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => {
      expect(previewOrganize).toHaveBeenCalledWith(
        "/lib.db",
        expect.anything(),
        expect.objectContaining({ target_folder: null }),
      );
    });
  });

  it("sends the chosen subfolder levels in order, skipping the empty ones", async () => {
    const user = userEvent.setup();
    renderView();
    await user.selectOptions(screen.getByLabelText("Subfolder level 1"), "genre");
    await user.selectOptions(
      screen.getByLabelText("Subfolder level 3"),
      "bitrate_bucket",
    );
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => {
      expect(previewOrganize).toHaveBeenCalledWith(
        "/lib.db",
        expect.anything(),
        expect.objectContaining({
          subfolders: {
            levels: [{ kind: "field", name: "genre" }, { kind: "bitrate_bucket" }],
          },
        }),
      );
    });
  });

  it("shows the plan including rows that would not change", async () => {
    const user = userEvent.setup();
    renderView();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    expect(await screen.findByTestId("organize-preview")).toBeInTheDocument();
    expect(
      screen.getByText("/Music/House/Daft Punk - Get Lucky.mp3"),
    ).toBeInTheDocument();
    expect(screen.getByText("already in place")).toBeInTheDocument();
  });

  it("only offers to move the rows that actually change", async () => {
    const user = userEvent.setup();
    renderView();
    expect(
      screen.getByRole("button", { name: "Move 0 file(s)" }),
    ).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    const move = await screen.findByRole("button", { name: "Move 1 file(s)" });
    await user.click(move);
    await waitFor(() => {
      expect(applyOrganize).toHaveBeenCalledWith("/lib.db", [ROWS[0]]);
    });
  });

  it("tells the user a sync is still needed after moving", async () => {
    const user = userEvent.setup();
    renderView();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await user.click(await screen.findByRole("button", { name: "Move 1 file(s)" }));
    expect(
      await screen.findByText(/Sync to update Rekordbox/),
    ).toBeInTheDocument();
  });

  it("reports partial failures rather than claiming success", async () => {
    const user = userEvent.setup();
    vi.mocked(applyOrganize).mockResolvedValue({
      moved: [],
      failed: [["t1", "permission denied"]],
      staged: [],
    });
    renderView();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await user.click(await screen.findByRole("button", { name: "Move 1 file(s)" }));
    expect(await screen.findByText(/permission denied/)).toBeInTheDocument();
  });

  it("blocks preview on a malformed pattern", async () => {
    const user = userEvent.setup();
    vi.mocked(validatePattern).mockRejectedValue("unterminated %field% in pattern");
    renderView();
    await user.clear(screen.getByLabelText(/Filename pattern/i));
    await user.type(screen.getByLabelText(/Filename pattern/i), "%artist");
    expect(await screen.findByRole("alert")).toHaveTextContent(/unterminated/);
    expect(screen.getByRole("button", { name: "Preview" })).toBeDisabled();
  });

  it("warns when a pattern uses a field decks cannot fill", async () => {
    const user = userEvent.setup();
    renderView();
    await user.clear(screen.getByLabelText(/Filename pattern/i));
    await user.type(screen.getByLabelText(/Filename pattern/i), "%remixer%");
    expect(
      await screen.findByText(/cannot fill remixer yet/),
    ).toBeInTheDocument();
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(previewOrganize).mockRejectedValue(new Error("library locked"));
    renderView();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    expect(await screen.findByText(/library locked/)).toBeInTheDocument();
  });
});
