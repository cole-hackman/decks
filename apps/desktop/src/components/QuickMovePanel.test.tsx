import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QuickMovePanel } from "./QuickMovePanel";
import {
  applyOrganize,
  deleteQuickMoveFolder,
  listQuickMoveFolders,
  previewOrganize,
  recordQuickMoveFolder,
  toggleQuickMoveFavourite,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { OrganizeRow, QuickMoveFolder } from "../types";

vi.mock("../ipc", () => ({
  listQuickMoveFolders: vi.fn(),
  recordQuickMoveFolder: vi.fn(),
  toggleQuickMoveFavourite: vi.fn(),
  deleteQuickMoveFolder: vi.fn(),
  previewOrganize: vi.fn(),
  applyOrganize: vi.fn(),
}));

const FOLDERS: QuickMoveFolder[] = [
  { id: "q1", path: "/Music/Techno", favourite: true, last_used_at: 2 },
  { id: "q2", path: "/Music/House", favourite: false, last_used_at: 1 },
];

const ROW: OrganizeRow = {
  track_id: "t1",
  source: "/Incoming/t1.mp3",
  destination: "/Music/Techno/t1.mp3",
  title: "A",
  artist: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listQuickMoveFolders).mockResolvedValue(FOLDERS);
  vi.mocked(recordQuickMoveFolder).mockResolvedValue("q1");
  vi.mocked(toggleQuickMoveFavourite).mockResolvedValue(true);
  vi.mocked(deleteQuickMoveFolder).mockResolvedValue(true);
  vi.mocked(previewOrganize).mockResolvedValue([ROW]);
  vi.mocked(applyOrganize).mockResolvedValue({
    moved: ["t1"],
    failed: [],
    staged: ["c1"],
  });
});

function renderPanel(trackIds = ["t1"]) {
  render(
    <WithProviders>
      <QuickMovePanel libraryPath="/lib.db" trackIds={trackIds} />
    </WithProviders>,
  );
}

describe("QuickMovePanel", () => {
  it("says so when no folders are remembered yet", async () => {
    vi.mocked(listQuickMoveFolders).mockResolvedValue([]);
    renderPanel();
    expect(
      await screen.findByTestId("no-quick-move-folders"),
    ).toBeInTheDocument();
  });

  it("numbers favourites for their hotkeys and leaves plain recents unnumbered", async () => {
    renderPanel();
    const favourite = await screen.findByRole("button", { name: "/Music/Techno" });
    expect(favourite.parentElement).toHaveTextContent("1");
    const plain = screen.getByRole("button", { name: "/Music/House" });
    expect(plain.parentElement?.textContent).not.toMatch(/^\s*2/);
  });

  it("moves to a folder when it is clicked, and remembers it", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "/Music/Techno" }));

    await waitFor(() => {
      expect(previewOrganize).toHaveBeenCalledWith(
        "/lib.db",
        ["t1"],
        expect.objectContaining({ target_folder: "/Music/Techno" }),
      );
    });
    expect(applyOrganize).toHaveBeenCalledWith("/lib.db", [ROW]);
    expect(recordQuickMoveFolder).toHaveBeenCalledWith("/Music/Techno");
  });

  it("reminds the user that a full sync is needed", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "/Music/Techno" }));
    expect(
      await screen.findByText(/full sync clears the old locations/),
    ).toBeInTheDocument();
  });

  it("hotkey 1 moves to the first favourite", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByRole("button", { name: "/Music/Techno" });
    await user.keyboard("1");
    await waitFor(() => {
      expect(previewOrganize).toHaveBeenCalledWith(
        "/lib.db",
        ["t1"],
        expect.objectContaining({ target_folder: "/Music/Techno" }),
      );
    });
  });

  it("a hotkey with no favourite behind it does nothing", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByRole("button", { name: "/Music/Techno" });
    await user.keyboard("5");
    expect(previewOrganize).not.toHaveBeenCalled();
  });

  it("typing a folder path does not fire a hotkey move", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByRole("button", { name: "/Music/Techno" });
    await user.type(screen.getByLabelText("New quick move folder"), "/Music/1");
    expect(previewOrganize).not.toHaveBeenCalled();
  });

  it("says when everything is already in place rather than claiming a move", async () => {
    const user = userEvent.setup();
    vi.mocked(previewOrganize).mockResolvedValue([{ ...ROW, destination: null }]);
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "/Music/Techno" }));
    expect(await screen.findByText(/already there/)).toBeInTheDocument();
    expect(applyOrganize).not.toHaveBeenCalled();
  });

  it("reports partial failures rather than claiming success", async () => {
    const user = userEvent.setup();
    vi.mocked(applyOrganize).mockResolvedValue({
      moved: [],
      failed: [["t1", "permission denied"]],
      staged: [],
    });
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "/Music/Techno" }));
    expect(await screen.findByText(/permission denied/)).toBeInTheDocument();
  });

  it("favourites and forgets folders", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByLabelText("Favourite /Music/House"));
    expect(toggleQuickMoveFavourite).toHaveBeenCalledWith("q2");

    await user.click(screen.getByLabelText("Forget /Music/House"));
    expect(deleteQuickMoveFolder).toHaveBeenCalledWith("q2");
  });

  it("moving is unavailable with nothing selected", async () => {
    renderPanel([]);
    expect(
      await screen.findByRole("button", { name: "/Music/Techno" }),
    ).toBeDisabled();
  });
});
