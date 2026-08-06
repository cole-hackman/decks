import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrokenTracksPanel } from "./BrokenTracksPanel";
import { brokenTracksReport, saveTextFile, scanBrokenTracks } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { BrokenScan } from "../types";

vi.mock("../ipc", () => ({
  scanBrokenTracks: vi.fn(),
  brokenTracksReport: vi.fn(),
  saveTextFile: vi.fn(),
}));

const SCAN: BrokenScan = {
  checked: 3,
  no_path: 1,
  broken: [
    {
      track_id: "t1",
      title: "Get Lucky",
      artist: "Daft Punk",
      path: "/music/a.mp3",
      status: { kind: "truncated", detail: "only 40% of the audio is present" },
      playlists: ["Techno Set", "Warmup"],
    },
    {
      track_id: "t2",
      title: "Nothing",
      artist: null,
      path: "/music/b.mp3",
      status: { kind: "missing" },
      playlists: [],
    },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(scanBrokenTracks).mockResolvedValue(SCAN);
  vi.mocked(brokenTracksReport).mockResolvedValue("Broken tracks\n\n…");
  vi.mocked(saveTextFile).mockResolvedValue("/home/me/broken-tracks.txt");
});

function renderPanel() {
  render(
    <WithProviders>
      <BrokenTracksPanel libraryPath="/lib.db" />
    </WithProviders>,
  );
}

describe("BrokenTracksPanel", () => {
  it("names the trade the two depths make", async () => {
    const user = userEvent.setup();
    renderPanel();
    // The quick check is the default, and the note says what it misses.
    expect(screen.getByTestId("broken-depth-note")).toHaveTextContent(
      /fine until the last ten seconds/,
    );
    await user.selectOptions(screen.getByLabelText("Check depth"), "full");
    expect(screen.getByTestId("broken-depth-note")).toHaveTextContent(
      /truncated download/,
    );
  });

  it("scans at the chosen depth", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.selectOptions(screen.getByLabelText("Check depth"), "full");
    await user.click(screen.getByRole("button", { name: "Scan" }));
    await waitFor(() => {
      expect(scanBrokenTracks).toHaveBeenCalledWith("/lib.db", [], "full");
    });
  });

  it("reports how many were checked, not only how many failed", async () => {
    // A bare "2 broken" cannot be told apart from a scan that did nothing.
    const user = userEvent.setup();
    renderPanel();
    await user.click(screen.getByRole("button", { name: "Scan" }));
    const result = await screen.findByTestId("broken-scan-result");
    expect(result).toHaveTextContent("Checked 3 track(s): 2 broken");
    expect(result).toHaveTextContent("1 with no file path to check");
  });

  it("says why each track failed, in a sentence", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(screen.getByRole("button", { name: "Scan" }));
    const result = await screen.findByTestId("broken-scan-result");
    expect(result).toHaveTextContent("incomplete: only 40% of the audio is present");
    expect(result).toHaveTextContent("the file is not there");
  });

  it("names the playlists a broken track was in", async () => {
    // The reason the report exists — sourcing a replacement means knowing
    // which set is now short a track.
    const user = userEvent.setup();
    renderPanel();
    await user.click(screen.getByRole("button", { name: "Scan" }));
    expect(await screen.findByText("in: Techno Set, Warmup")).toBeInTheDocument();
  });

  it("says so when everything plays", async () => {
    const user = userEvent.setup();
    vi.mocked(scanBrokenTracks).mockResolvedValue({
      checked: 40,
      no_path: 0,
      broken: [],
    });
    renderPanel();
    await user.click(screen.getByRole("button", { name: "Scan" }));
    expect(await screen.findByTestId("no-broken-tracks")).toBeInTheDocument();
  });

  it("cannot save a report before a scan has found anything", async () => {
    renderPanel();
    expect(screen.getByRole("button", { name: "Save report" })).toBeDisabled();
  });

  it("saves the report where the user chose", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(screen.getByRole("button", { name: "Scan" }));
    await user.click(await screen.findByRole("button", { name: "Save report" }));
    await waitFor(() => {
      expect(brokenTracksReport).toHaveBeenCalledWith(SCAN.broken);
    });
    expect(await screen.findByText(/broken-tracks\.txt/)).toBeInTheDocument();
  });

  it("a cancelled save dialog is not an error", async () => {
    const user = userEvent.setup();
    vi.mocked(saveTextFile).mockResolvedValue(null);
    renderPanel();
    await user.click(screen.getByRole("button", { name: "Scan" }));
    await user.click(await screen.findByRole("button", { name: "Save report" }));
    await waitFor(() => expect(saveTextFile).toHaveBeenCalled());
    expect(screen.queryByText(/Saved to/)).not.toBeInTheDocument();
  });

  it("drops a stale result when the depth changes", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(screen.getByRole("button", { name: "Scan" }));
    await screen.findByTestId("broken-scan-result");
    await user.selectOptions(screen.getByLabelText("Check depth"), "full");
    expect(screen.queryByTestId("broken-scan-result")).not.toBeInTheDocument();
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(scanBrokenTracks).mockRejectedValue(new Error("library locked"));
    renderPanel();
    await user.click(screen.getByRole("button", { name: "Scan" }));
    expect(await screen.findByText(/library locked/)).toBeInTheDocument();
  });
});
