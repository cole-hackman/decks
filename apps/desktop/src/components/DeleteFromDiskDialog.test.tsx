import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DeleteFromDiskDialog } from "./DeleteFromDiskDialog";
import * as ipc from "../ipc";
import type { DeletePlanView } from "../types";

vi.mock("../ipc");

const basePlan: DeletePlanView = {
  deletable: [{ track_id: "t1", source: "/music/a.mp3", bytes: 5_242_880 }],
  refused: [],
  total_bytes: 5_242_880,
  labels: { t1: "Daft Punk — One More Time", t2: "Justice — D.A.N.C.E." },
  no_roots_configured: false,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.planDeleteFromDisk).mockResolvedValue(basePlan);
  vi.mocked(ipc.deleteFromDisk).mockResolvedValue({
    manifest: {
      batch_id: "2025-08-06T14-22-01",
      created_at: 1_754_490_121,
      library_path: "/lib/master.db",
      reason: "Duplicates",
      entries: [
        {
          track_id: "t1",
          original_path: "/music/a.mp3",
          stored_as: "a.mp3",
          bytes: 5_242_880,
        },
      ],
    },
    failed: [],
  });
});

function renderDialog(overrides: Partial<Parameters<typeof DeleteFromDiskDialog>[0]> = {}) {
  return render(
    <DeleteFromDiskDialog
      libraryPath="/lib/master.db"
      trackIds={["t1"]}
      reason="Duplicates"
      onClose={vi.fn()}
      {...overrides}
    />,
  );
}

describe("DeleteFromDiskDialog", () => {
  it("plans before it asks, and names the files it will move", async () => {
    renderDialog();
    expect(
      await screen.findByText("Daft Punk — One More Time"),
    ).toBeInTheDocument();
    expect(screen.getByText("/music/a.mp3")).toBeInTheDocument();
    expect(ipc.deleteFromDisk).not.toHaveBeenCalled();
  });

  it("says the audio is restorable rather than gone", async () => {
    renderDialog();
    await screen.findByText("Daft Punk — One More Time");
    expect(
      screen.getByText(/Restorable from Settings/i),
    ).toBeInTheDocument();
  });

  it("needs two clicks — the first only arms the confirmation", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(await screen.findByRole("button", { name: /Delete 1 from disk/ }));
    expect(ipc.deleteFromDisk).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /Yes, move 1 file/ }));
    await waitFor(() => expect(ipc.deleteFromDisk).toHaveBeenCalledOnce());
  });

  it("shows every refusal with its reason", async () => {
    vi.mocked(ipc.planDeleteFromDisk).mockResolvedValue({
      ...basePlan,
      refused: [
        {
          track_id: "t2",
          path: "/music/b.mp3",
          reason: { kind: "symlink" },
          message: "That path is a symbolic link.",
        },
      ],
    });
    renderDialog({ trackIds: ["t1", "t2"] });
    const refused = await screen.findByTestId("refused-list");
    expect(refused).toHaveTextContent("Justice — D.A.N.C.E.");
    expect(refused).toHaveTextContent("That path is a symbolic link.");
  });

  it("explains the fail-closed state instead of just refusing everything", async () => {
    vi.mocked(ipc.planDeleteFromDisk).mockResolvedValue({
      ...basePlan,
      deletable: [],
      total_bytes: 0,
      no_roots_configured: true,
    });
    renderDialog();
    expect(
      await screen.findByText(/No music folders are set up yet/i),
    ).toBeInTheDocument();
    // Nothing to delete, so the action is not offered at all.
    expect(
      screen.getByRole("button", { name: /Delete 0 from disk/ }),
    ).toBeDisabled();
  });

  it("offers the playlist override only when a playlist is what blocked something", async () => {
    vi.mocked(ipc.planDeleteFromDisk).mockResolvedValue({
      ...basePlan,
      refused: [
        {
          track_id: "t2",
          path: "/music/b.mp3",
          reason: { kind: "still_in_playlists", playlists: ["Warmup"] },
          message: "Still in a playlist: Warmup.",
        },
      ],
    });
    const user = userEvent.setup();
    renderDialog({ trackIds: ["t1", "t2"] });

    const toggle = await screen.findByRole("checkbox", {
      name: /Also delete tracks that playlists still use/,
    });
    await user.click(toggle);

    // Turning it on re-plans rather than reusing the stale answer.
    await waitFor(() =>
      expect(ipc.planDeleteFromDisk).toHaveBeenLastCalledWith(
        "/lib/master.db",
        ["t1", "t2"],
        "Duplicates",
        true,
      ),
    );
  });

  it("does not offer the override when nothing was blocked by a playlist", async () => {
    renderDialog();
    await screen.findByText("Daft Punk — One More Time");
    expect(
      screen.queryByRole("checkbox", {
        name: /Also delete tracks that playlists still use/,
      }),
    ).not.toBeInTheDocument();
  });

  it("un-arms the confirmation when the override changes the list", async () => {
    vi.mocked(ipc.planDeleteFromDisk).mockResolvedValue({
      ...basePlan,
      refused: [
        {
          track_id: "t2",
          path: "/music/b.mp3",
          reason: { kind: "still_in_playlists", playlists: ["Warmup"] },
          message: "Still in a playlist: Warmup.",
        },
      ],
    });
    const user = userEvent.setup();
    renderDialog({ trackIds: ["t1", "t2"] });

    await user.click(await screen.findByRole("button", { name: /Delete 1 from disk/ }));
    expect(screen.getByRole("button", { name: /Yes, move/ })).toBeInTheDocument();

    await user.click(
      screen.getByRole("checkbox", {
        name: /Also delete tracks that playlists still use/,
      }),
    );
    // The user agreed to one list; the list changed, so the agreement lapses.
    expect(screen.queryByRole("button", { name: /Yes, move/ })).not.toBeInTheDocument();
  });

  it("surfaces a failure and stays open", async () => {
    vi.mocked(ipc.deleteFromDisk).mockRejectedValue("Permission denied");
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderDialog({ onClose });

    await user.click(await screen.findByRole("button", { name: /Delete 1 from disk/ }));
    await user.click(screen.getByRole("button", { name: /Yes, move/ }));

    expect(await screen.findByText(/Permission denied/)).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
