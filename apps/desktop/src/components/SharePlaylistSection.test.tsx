import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SharePlaylistSection } from "./SharePlaylistSection";
import { saveShareFile, sharePlaylist } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { Playlist } from "../types";

vi.mock("../ipc", () => ({
  sharePlaylist: vi.fn(),
  saveShareFile: vi.fn(),
}));

const PLAYLISTS: Playlist[] = [
  { id: "f1", name: "Sets", parent_id: null, seq: 1, kind: "Folder" },
  { id: "p1", name: "Warmup", parent_id: "f1", seq: 1, kind: "Playlist" },
];

const writeText = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(sharePlaylist).mockResolvedValue({
    content: "Title,Artist\nOne,A",
    filename: "Warmup.csv",
    track_count: 1,
    skipped: [],
  });
  vi.mocked(saveShareFile).mockResolvedValue("/home/me/Warmup.csv");
  Object.assign(navigator, { clipboard: { writeText } });
  writeText.mockResolvedValue(undefined);
});

function renderSection() {
  render(
    <WithProviders>
      <SharePlaylistSection libraryPath="/lib.db" playlists={PLAYLISTS} />
    </WithProviders>,
  );
}

describe("SharePlaylistSection", () => {
  it("says what sharing is not", async () => {
    // The spec draws the line explicitly, and so should the UI: this produces
    // a file, it does not touch the library.
    renderSection();
    expect(
      screen.getByText(/Sharing produces a file\. Syncing updates Rekordbox/),
    ).toBeInTheDocument();
  });

  it("offers only playlists, not folders", () => {
    renderSection();
    const select = screen.getByRole("combobox", { name: "Playlist to share" });
    expect(select).toHaveTextContent("Warmup");
    expect(select).not.toHaveTextContent("Sets");
  });

  it("will not export until a playlist is chosen", async () => {
    renderSection();
    expect(screen.getByRole("button", { name: "Preview export" })).toBeDisabled();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Playlist to share" }),
      "p1",
    );
    expect(
      screen.getByRole("button", { name: "Preview export" }),
    ).toBeEnabled();
  });

  it("sends the columns in the order they were ticked", async () => {
    renderSection();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Playlist to share" }),
      "p1",
    );
    // Clear the defaults, then tick two in a deliberate order.
    for (const label of ["Title", "Artist", "BPM", "Key", "Duration"]) {
      await userEvent.click(screen.getByRole("button", { name: new RegExp(`^${label}`) }));
    }
    await userEvent.click(screen.getByRole("button", { name: /^BPM/ }));
    await userEvent.click(screen.getByRole("button", { name: /^Title/ }));

    await userEvent.click(screen.getByRole("button", { name: "Preview export" }));
    expect(sharePlaylist).toHaveBeenCalledWith("/lib.db", "p1", "csv", [
      "bpm",
      "title",
    ]);
  });

  it("hides the column picker for formats that have no columns", async () => {
    renderSection();
    expect(screen.getByTestId("share-columns")).toBeInTheDocument();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Export format" }),
      "m3u",
    );
    expect(screen.queryByTestId("share-columns")).not.toBeInTheDocument();
  });

  it("copies to the clipboard rather than offering a file for quick copy", async () => {
    vi.mocked(sharePlaylist).mockResolvedValue({
      content: "A - One",
      filename: "Warmup.txt",
      track_count: 1,
      skipped: [],
    });
    renderSection();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Playlist to share" }),
      "p1",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Export format" }),
      "quick_copy",
    );
    await userEvent.click(screen.getByRole("button", { name: "Preview export" }));
    await screen.findByTestId("share-preview");

    expect(screen.queryByRole("button", { name: "Save file" })).not.toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Copy to clipboard" }),
    );
    expect(writeText).toHaveBeenCalledWith("A - One");
  });

  it("saves a file for the formats that are files", async () => {
    renderSection();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Playlist to share" }),
      "p1",
    );
    await userEvent.click(screen.getByRole("button", { name: "Preview export" }));
    await screen.findByTestId("share-preview");
    await userEvent.click(screen.getByRole("button", { name: "Save file" }));
    expect(saveShareFile).toHaveBeenCalledWith(
      "csv",
      "Warmup.csv",
      "Title,Artist\nOne,A",
    );
  });

  it("names the tracks an M3U could not carry", async () => {
    // Handing back a quietly short playlist is how a set goes missing.
    vi.mocked(sharePlaylist).mockResolvedValue({
      content: "#EXTM3U\n",
      filename: "Warmup.m3u8",
      track_count: 3,
      skipped: ["Streaming Only", "No Path"],
    });
    renderSection();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Playlist to share" }),
      "p1",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Export format" }),
      "m3u",
    );
    await userEvent.click(screen.getByRole("button", { name: "Preview export" }));
    expect(await screen.findByTestId("share-skipped")).toHaveTextContent(
      "2 track(s) have no file path and are not in the M3U: Streaming Only, No Path",
    );
  });

  it("says the HTML route to a PDF rather than pretending to write one", async () => {
    renderSection();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Export format" }),
      "html",
    );
    expect(screen.getByTestId("share-format-blurb")).toHaveTextContent(
      "Printer-friendly. Use the browser's Save to PDF for a PDF.",
    );
  });

  it("changing the format drops a stale preview", async () => {
    renderSection();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Playlist to share" }),
      "p1",
    );
    await userEvent.click(screen.getByRole("button", { name: "Preview export" }));
    await screen.findByTestId("share-preview");

    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Export format" }),
      "m3u",
    );
    expect(screen.queryByTestId("share-preview")).not.toBeInTheDocument();
  });
});
