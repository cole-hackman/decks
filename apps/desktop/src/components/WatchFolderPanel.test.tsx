import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { WatchFolderPanel } from "./WatchFolderPanel";
import {
  addWatchFolder,
  clearDismissedArrivals,
  dismissArrivals,
  listWatchFolders,
  removeWatchFolder,
  scanArrivals,
  stageArrivalImports,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { WatchScan } from "../types";

vi.mock("../ipc", () => ({
  listWatchFolders: vi.fn(),
  addWatchFolder: vi.fn(),
  removeWatchFolder: vi.fn(),
  scanArrivals: vi.fn(),
  stageArrivalImports: vi.fn(),
  dismissArrivals: vi.fn(),
  clearDismissedArrivals: vi.fn(),
}));

const SCAN: WatchScan = {
  arrivals: [
    { path: "/Watch/new track.mp3", size_bytes: 8_000_000, age_secs: 60 },
    { path: "/Watch/another.flac", size_bytes: 30_000_000, age_secs: 30 },
  ],
  pending: [{ path: "/Watch/copying.mp3", size_bytes: 1000, age_secs: 1 }],
  errors: [],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listWatchFolders).mockResolvedValue([
    { id: "w1", path: "/Watch" },
  ]);
  vi.mocked(scanArrivals).mockResolvedValue(SCAN);
  vi.mocked(addWatchFolder).mockResolvedValue("w2");
  vi.mocked(removeWatchFolder).mockResolvedValue(true);
  vi.mocked(dismissArrivals).mockResolvedValue(1);
  vi.mocked(clearDismissedArrivals).mockResolvedValue(3);
  vi.mocked(stageArrivalImports).mockResolvedValue({
    staged: ["c1", "c2"],
    failed: [],
    analysed: [],
    tagged: [],
    tag_skipped: [],
  });
});

afterEach(() => {
  vi.useRealTimers();
});

function renderPanel() {
  render(
    <WithProviders>
      <WatchFolderPanel libraryPath="/lib.db" />
    </WithProviders>,
  );
}

describe("WatchFolderPanel", () => {
  it("says so when nothing is being watched", async () => {
    vi.mocked(listWatchFolders).mockResolvedValue([]);
    renderPanel();
    expect(await screen.findByTestId("no-watch-folders")).toBeInTheDocument();
  });

  it("lists arrivals with their size", async () => {
    renderPanel();
    expect(await screen.findByTestId("watch-arrivals")).toBeInTheDocument();
    expect(screen.getByText("new track.mp3")).toBeInTheDocument();
    expect(screen.getByText("2 new file(s)")).toBeInTheDocument();
  });

  it("holds back files that are still being written, and says so", async () => {
    renderPanel();
    expect(await screen.findByTestId("watch-pending")).toHaveTextContent(
      /1 file\(s\) still being written/,
    );
  });

  it("imports every arrival and explains that sync cannot add tracks", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "Import all" }));

    await waitFor(() => {
      expect(stageArrivalImports).toHaveBeenCalledWith("/lib.db", [
        "/Watch/new track.mp3",
        "/Watch/another.flac",
      ]);
    });
    expect(
      await screen.findByText(/Export the XML and import it in Rekordbox/),
    ).toBeInTheDocument();
  });

  it("imports a single arrival", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByLabelText("Import new track.mp3"));
    await waitFor(() => {
      expect(stageArrivalImports).toHaveBeenCalledWith("/lib.db", [
        "/Watch/new track.mp3",
      ]);
    });
  });

  it("ignoring an arrival dismisses it and rescans", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByLabelText("Ignore new track.mp3"));
    await waitFor(() => {
      expect(dismissArrivals).toHaveBeenCalledWith(["/Watch/new track.mp3"]);
    });
    expect(scanArrivals).toHaveBeenCalledTimes(2);
  });

  it("can un-ignore everything to triage the folder again", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByTestId("watch-arrivals");
    await user.click(screen.getByRole("button", { name: "Un-ignore everything" }));
    await waitFor(() => {
      expect(clearDismissedArrivals).toHaveBeenCalled();
    });
  });

  it("adds and removes watch folders", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.type(screen.getByLabelText("New watch folder"), "/Downloads");
    await user.click(screen.getByRole("button", { name: "Watch" }));
    await waitFor(() => {
      expect(addWatchFolder).toHaveBeenCalledWith("/Downloads");
    });

    await user.click(await screen.findByLabelText("Stop watching /Watch"));
    await waitFor(() => {
      expect(removeWatchFolder).toHaveBeenCalledWith("w1");
    });
  });

  it("reports import failures rather than claiming success", async () => {
    const user = userEvent.setup();
    vi.mocked(stageArrivalImports).mockResolvedValue({
      staged: [],
      failed: [["/Watch/new track.mp3", "unsupported format"]],
      analysed: [],
      tagged: [],
      tag_skipped: [],
    });
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "Import all" }));
    expect(await screen.findByText(/unsupported format/)).toBeInTheDocument();
  });

  it("does not toast on a failed background scan", async () => {
    // A folder unplugged mid-session would otherwise notify every poll.
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    vi.mocked(scanArrivals).mockRejectedValue(new Error("folder gone"));
    renderPanel();
    await waitFor(() => {
      expect(spy).toHaveBeenCalled();
    });
    expect(screen.queryByText(/folder gone/)).not.toBeInTheDocument();
    spy.mockRestore();
  });

  it("says when files were analysed and tagged, and keeps the two apart", async () => {
    // Analysis reads the file; tagging rewrites it. A summary that blurred
    // them would hide the fact that files on disk changed.
    const user = userEvent.setup();
    vi.mocked(stageArrivalImports).mockResolvedValue({
      staged: ["c1", "c2"],
      failed: [],
      analysed: ["/Watch/a.mp3", "/Watch/b.mp3"],
      tagged: ["/Watch/a.mp3"],
      tag_skipped: [],
    });

    renderPanel();
    await user.click(await screen.findByRole("button", { name: "Import all" }));
    expect(
      await screen.findByText(/2 analysed, 1 tagged/),
    ).toBeInTheDocument();
  });

  it("explains a skipped tag write instead of staying quiet about it", async () => {
    // A setting the user turned on that silently does nothing looks broken.
    const user = userEvent.setup();
    vi.mocked(stageArrivalImports).mockResolvedValue({
      staged: ["c1"],
      failed: [],
      analysed: ["/Watch/a.mp3"],
      tagged: [],
      tag_skipped: [["/Watch/a.mp3", "analysis confidence 40% is below the 75% needed to overwrite a tag"]],
    });

    renderPanel();
    await user.click(await screen.findByRole("button", { name: "Import all" }));
    expect(
      await screen.findByText(/below the 75% needed to overwrite a tag/),
    ).toBeInTheDocument();
  });

  it("says nothing extra when neither automation is on", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "Import all" }));
    const toast = await screen.findByText(/Staged 2 new track\(s\)/);
    expect(toast).not.toHaveTextContent(/analysed|tagged/);
  });

  it("survives a shell that does not know the new fields", async () => {
    // An older shell resolves successfully without them. `.length` on
    // undefined would take the whole import down — the same failure the
    // cue-presets null once caused, which cost a bisect to find.
    const user = userEvent.setup();
    vi.mocked(stageArrivalImports).mockResolvedValue({
      staged: ["c1"],
      failed: [],
    } as never);

    renderPanel();
    await user.click(await screen.findByRole("button", { name: "Import all" }));
    expect(await screen.findByText(/Staged 1 new track/)).toBeInTheDocument();
  });
});
