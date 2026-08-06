import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DeletedAudioSection } from "./DeletedAudioSection";
import { WithProviders } from "../test-utils/providers";
import * as ipc from "../ipc";
import type { DeleteBatch } from "../types";

vi.mock("../ipc");

const batch: DeleteBatch = {
  manifest: {
    batch_id: "2025-08-06T14-22-01",
    created_at: 1_754_490_121,
    library_path: "/lib/master.db",
    reason: "Duplicate resolution",
    entries: [
      {
        track_id: "t1",
        original_path: "/music/a.mp3",
        stored_as: "a.mp3",
        bytes: 5_242_880,
      },
    ],
  },
  total_bytes: 5_242_880,
  file_count: 1,
};

function renderSection(libraryPath: string | null = "/lib/master.db") {
  return render(
    <WithProviders>
      <DeletedAudioSection libraryPath={libraryPath} />
    </WithProviders>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.musicRoots).mockResolvedValue([]);
  vi.mocked(ipc.listDeletedBatches).mockResolvedValue([]);
  vi.mocked(ipc.setMusicRoots).mockResolvedValue(undefined);
  vi.mocked(ipc.suggestMusicRoots).mockResolvedValue([]);
});

describe("DeletedAudioSection", () => {
  it("says the feature is off while no music folders are configured", async () => {
    renderSection();
    expect(await screen.findByTestId("no-music-roots")).toHaveTextContent(
      /Deleting from disk is off/i,
    );
  });

  it("lists configured folders and can drop one", async () => {
    vi.mocked(ipc.musicRoots).mockResolvedValue(["/Users/cole/Music"]);
    const user = userEvent.setup();
    renderSection();

    expect(await screen.findByText("/Users/cole/Music")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Remove /Users/cole/Music" }),
    );
    await waitFor(() => expect(ipc.setMusicRoots).toHaveBeenCalledWith([]));
  });

  it("suggests folders from the library and adds the one that is picked", async () => {
    vi.mocked(ipc.suggestMusicRoots).mockResolvedValue([
      { path: "/Users/cole", track_count: 4213 },
    ]);
    const user = userEvent.setup();
    renderSection();

    await user.click(
      await screen.findByRole("button", { name: "Suggest from library" }),
    );
    const suggestions = await screen.findByTestId("root-suggestions");
    expect(suggestions).toHaveTextContent("/Users/cole");
    expect(suggestions).toHaveTextContent("4213 tracks");

    await user.click(screen.getByRole("button", { name: "Add" }));
    await waitFor(() =>
      expect(ipc.setMusicRoots).toHaveBeenCalledWith(["/Users/cole"]),
    );
  });

  it("cannot suggest folders with no library open", async () => {
    renderSection(null);
    await screen.findByTestId("no-music-roots");
    expect(
      screen.queryByRole("button", { name: "Suggest from library" }),
    ).not.toBeInTheDocument();
  });

  it("shows a batch with what it holds and why it was deleted", async () => {
    vi.mocked(ipc.listDeletedBatches).mockResolvedValue([batch]);
    renderSection();
    expect(await screen.findByText(/1 file · 5\.0 MB/)).toBeInTheDocument();
    expect(screen.getByText(/Duplicate resolution/)).toBeInTheDocument();
  });

  it("restores a batch without asking — restoring is the safe direction", async () => {
    vi.mocked(ipc.listDeletedBatches).mockResolvedValue([batch]);
    vi.mocked(ipc.restoreDeletedBatch).mockResolvedValue({
      batch_id: batch.manifest.batch_id,
      results: [
        {
          track_id: "t1",
          original_path: "/music/a.mp3",
          outcome: { outcome: "restored", path: "/music/a.mp3" },
        },
      ],
      restored: 1,
      batch_emptied: true,
    });
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole("button", { name: /Restore/ }));
    await waitFor(() =>
      expect(ipc.restoreDeletedBatch).toHaveBeenCalledWith(
        "2025-08-06T14-22-01",
      ),
    );
  });

  it("reports the files a restore could not put back", async () => {
    vi.mocked(ipc.listDeletedBatches).mockResolvedValue([batch]);
    vi.mocked(ipc.restoreDeletedBatch).mockResolvedValue({
      batch_id: batch.manifest.batch_id,
      results: [
        {
          track_id: "t1",
          original_path: "/music/a.mp3",
          outcome: { outcome: "occupied", path: "/music/a.mp3" },
        },
      ],
      restored: 0,
      batch_emptied: false,
    });
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole("button", { name: /Restore/ }));
    expect(
      await screen.findByText(/1 could not be put back/i),
    ).toBeInTheDocument();
  });

  it("confirms before emptying, and says that step is the permanent one", async () => {
    vi.mocked(ipc.listDeletedBatches).mockResolvedValue([batch]);
    vi.mocked(ipc.purgeDeletedBatch).mockResolvedValue(5_242_880);
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole("button", { name: /Empty/ }));
    expect(
      await screen.findByText(/cannot be undone/i),
    ).toBeInTheDocument();
    expect(ipc.purgeDeletedBatch).not.toHaveBeenCalled();

    await user.click(
      screen.getByRole("button", { name: "Delete permanently" }),
    );
    await waitFor(() =>
      expect(ipc.purgeDeletedBatch).toHaveBeenCalledWith("2025-08-06T14-22-01"),
    );
  });

  it("leaves the batch alone when the confirmation is declined", async () => {
    vi.mocked(ipc.listDeletedBatches).mockResolvedValue([batch]);
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole("button", { name: /Empty/ }));
    await screen.findByText(/cannot be undone/i);
    await user.click(screen.getByRole("button", { name: /Cancel/ }));
    expect(ipc.purgeDeletedBatch).not.toHaveBeenCalled();
  });
});
