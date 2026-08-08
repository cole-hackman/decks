import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RelocateBanner } from "./RelocateBanner";
import {
  applyMergeRelocate,
  classifyRelocateTarget,
  planMergeRelocate,
  relocateScan,
  stageChange,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({
  relocateScan: vi.fn(),
  stageChange: vi.fn(),
  classifyRelocateTarget: vi.fn(),
  planMergeRelocate: vi.fn(),
  applyMergeRelocate: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue(["/music"]),
}));

const CANDIDATE = {
  track_id: "t1",
  original_path: "/old/gone.mp3",
  matches: [{ path: "/music/found.mp3", score: 0.9, reasons: ["filename"] }],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(relocateScan).mockResolvedValue([CANDIDATE]);
  vi.mocked(stageChange).mockResolvedValue({ id: "c1" } as never);
  vi.mocked(classifyRelocateTarget).mockResolvedValue("Free");
  vi.mocked(planMergeRelocate).mockResolvedValue({
    keeper_id: "t1",
    loser_id: "t2",
    relocate_to: "/music/found.mp3",
    resolution: {},
  });
  vi.mocked(applyMergeRelocate).mockResolvedValue({ archived: [], staged: [] });
});

function renderBanner() {
  render(
    <WithProviders>
      <RelocateBanner libraryPath="/db" />
    </WithProviders>,
  );
}

/** Scan, then accept the first suggested match. */
async function scanAndAccept() {
  await userEvent.click(screen.getByRole("button", { name: /Scan|Select/i }));
  const accept = await screen.findByRole("button", { name: /^Accept/i });
  await userEvent.click(accept);
}

describe("RelocateBanner", () => {
  it("stages a plain relocate when nothing else claims the file", async () => {
    renderBanner();
    await scanAndAccept();

    expect(vi.mocked(stageChange)).toHaveBeenCalled();
    expect(screen.queryByTestId("relocate-merge")).toBeNull();
  });

  it("offers a merge instead of staging when the file is already in the library", async () => {
    // Two rows pointing at one file is the state the spec's constraint exists
    // to prevent. The merge is the way out, not an error message.
    vi.mocked(classifyRelocateTarget).mockResolvedValue({
      Occupied: { track_id: "t2", title: "Other", artist: "Someone" },
    });
    renderBanner();
    await scanAndAccept();

    const dialog = await screen.findByTestId("relocate-merge");
    expect(dialog).toHaveTextContent("Someone — Other");
    // Nothing is written until the user picks which entry survives.
    expect(vi.mocked(stageChange)).not.toHaveBeenCalled();
  });

  it("keeps the missing entry, which takes over the found file's path", async () => {
    vi.mocked(classifyRelocateTarget).mockResolvedValue({
      Occupied: { track_id: "t2", title: "Other", artist: null },
    });
    renderBanner();
    await scanAndAccept();

    await userEvent.click(
      await screen.findByRole("button", { name: "Keep the missing entry" }),
    );
    expect(vi.mocked(planMergeRelocate)).toHaveBeenCalledWith(
      "/db",
      "t1",
      "t2",
      "/music/found.mp3",
      true,
    );
    expect(vi.mocked(applyMergeRelocate)).toHaveBeenCalled();
  });

  it("keeps the existing entry, which needs no path change at all", async () => {
    vi.mocked(classifyRelocateTarget).mockResolvedValue({
      Occupied: { track_id: "t2", title: "Other", artist: null },
    });
    renderBanner();
    await scanAndAccept();

    await userEvent.click(
      await screen.findByRole("button", { name: /^Keep Other$/ }),
    );
    expect(vi.mocked(planMergeRelocate)).toHaveBeenCalledWith(
      "/db",
      "t1",
      "t2",
      "/music/found.mp3",
      false,
    );
  });

  it("cancelling the merge writes nothing", async () => {
    vi.mocked(classifyRelocateTarget).mockResolvedValue({
      Occupied: { track_id: "t2", title: "Other", artist: null },
    });
    renderBanner();
    await scanAndAccept();

    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(screen.queryByTestId("relocate-merge")).toBeNull();
    expect(vi.mocked(applyMergeRelocate)).not.toHaveBeenCalled();
    expect(vi.mocked(stageChange)).not.toHaveBeenCalled();
  });
});
