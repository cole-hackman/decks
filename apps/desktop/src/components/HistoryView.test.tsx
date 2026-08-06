import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { HistoryView } from "./HistoryView";
import {
  deleteHistorySet,
  historySetTracks,
  importHistory,
  listHistorySets,
  previewHistoryAsPlaylist,
  removeHistoryTrack,
  saveHistoryAsPlaylist,
  setHistoryMetadata,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { HistorySet, HistoryTrack } from "../types";

vi.mock("../ipc", () => ({
  importHistory: vi.fn(),
  listHistorySets: vi.fn(),
  historySetTracks: vi.fn(),
  setHistoryMetadata: vi.fn(),
  deleteHistorySet: vi.fn(),
  removeHistoryTrack: vi.fn(),
  previewHistoryAsPlaylist: vi.fn(),
  saveHistoryAsPlaylist: vi.fn(),
}));

const SETS: HistorySet[] = [
  {
    id: "s1",
    source_id: "h1",
    name: "2026-05-01 Basement",
    played_at: "2026-05-01T22:00:00Z",
    rating: null,
    location: null,
    track_count: 2,
  },
];

const TRACKS: HistoryTrack[] = [
  {
    id: "t1",
    seq: 1,
    content_id: "1",
    title: "Original Title",
    artist: "Someone",
    album: null,
    genre: null,
    musical_key: "8A",
    bpm: 128,
    duration_secs: 365,
    folder_path: "/music/a.mp3",
  },
  {
    id: "t2",
    seq: 2,
    content_id: "gone",
    title: "Deleted Since",
    artist: "Nobody",
    album: null,
    genre: null,
    musical_key: null,
    bpm: null,
    duration_secs: null,
    folder_path: "/music/b.mp3",
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listHistorySets).mockResolvedValue(SETS);
  vi.mocked(historySetTracks).mockResolvedValue(TRACKS);
  vi.mocked(importHistory).mockResolvedValue({
    imported: 2,
    already_known: 0,
    previously_deleted: 0,
  });
  vi.mocked(setHistoryMetadata).mockResolvedValue(undefined);
  vi.mocked(deleteHistorySet).mockResolvedValue(true);
  vi.mocked(removeHistoryTrack).mockResolvedValue(true);
  vi.mocked(previewHistoryAsPlaylist).mockResolvedValue({
    matches: [
      {
        history_track_id: "t1",
        title: "Original Title",
        artist: "Someone",
        track_id: "1",
        kind: "content_id",
      },
      {
        history_track_id: "t2",
        title: "Deleted Since",
        artist: "Nobody",
        track_id: null,
        kind: "none",
      },
    ],
    matched: 1,
    unmatched: 1,
  });
  vi.mocked(saveHistoryAsPlaylist).mockResolvedValue(["c1", "c2"]);
});

function renderView() {
  render(
    <WithProviders>
      <HistoryView libraryPath="/lib.db" />
    </WithProviders>,
  );
}

async function openSet() {
  renderView();
  await userEvent.click(await screen.findByText("2026-05-01 Basement"));
  await screen.findByTestId("history-tracks");
}

describe("HistoryView", () => {
  it("says re-importing is safe rather than leaving the user to guess", async () => {
    vi.mocked(listHistorySets).mockResolvedValue([]);
    renderView();
    expect(await screen.findByTestId("history-empty")).toHaveTextContent(
      "running it again never duplicates them",
    );
  });

  it("reports what an import skipped, not just what it took", async () => {
    // Otherwise "why is my deleted set not back?" is a mystery.
    vi.mocked(importHistory).mockResolvedValue({
      imported: 1,
      already_known: 3,
      previously_deleted: 2,
    });
    renderView();
    await userEvent.click(screen.getByRole("button", { name: "Import" }));
    expect(
      await screen.findByText(
        "1 imported · 3 already known · 2 skipped (deleted before)",
      ),
    ).toBeInTheDocument();
  });

  it("states the snapshot rule, which is why a row can differ from the library", async () => {
    await openSet();
    expect(
      screen.getByText(/Editing them since has not changed this record/),
    ).toBeInTheDocument();
  });

  it("shows the set in play order, including a track that is gone", async () => {
    await openSet();
    const list = screen.getByTestId("history-tracks");
    expect(list).toHaveTextContent("Original Title");
    // The snapshot survives the track leaving the library — the point of it.
    expect(list).toHaveTextContent("Deleted Since");
  });

  it("warns that deleting a set sticks", async () => {
    await openSet();
    await userEvent.click(screen.getByRole("button", { name: "Delete set" }));
    expect(
      await screen.findByText(/importing again will not bring it back/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/audio files and library are untouched/),
    ).toBeInTheDocument();
  });

  it("does not delete when the confirmation is declined", async () => {
    await openSet();
    await userEvent.click(screen.getByRole("button", { name: "Delete set" }));
    await screen.findByText(/importing again will not bring it back/);
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(deleteHistorySet).not.toHaveBeenCalled();
  });

  it("saves rating and location against the set", async () => {
    await openSet();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Set rating" }),
      "4",
    );
    await waitFor(() =>
      expect(setHistoryMetadata).toHaveBeenCalledWith("s1", 4, null),
    );

    const location = screen.getByRole("textbox", { name: "Set location" });
    await userEvent.type(location, "The Basement");
    await userEvent.tab();
    await waitFor(() =>
      expect(setHistoryMetadata).toHaveBeenLastCalledWith(
        "s1",
        null,
        "The Basement",
      ),
    );
  });

  it("names how each track was re-matched rather than implying they all were", async () => {
    // "Same filename" is a materially weaker claim than "same track" — ADR-0008.
    vi.mocked(previewHistoryAsPlaylist).mockResolvedValue({
      matches: [
        {
          history_track_id: "t1",
          title: "Moved File",
          artist: "A",
          track_id: "1",
          kind: "filename",
        },
        {
          history_track_id: "t2",
          title: "Deleted Since",
          artist: "B",
          track_id: null,
          kind: "none",
        },
      ],
      matched: 1,
      unmatched: 1,
    });
    await openSet();
    await userEvent.click(
      screen.getByRole("button", { name: "Save as playlist" }),
    );
    const report = await screen.findByTestId("history-match-report");
    expect(report).toHaveTextContent("1 of 2 track(s) are still in the library");
    expect(report).toHaveTextContent("Moved File — same filename — the file moved");
    expect(report).toHaveTextContent("Deleted Since — not in the library any more");
  });

  it("stages only the tracks that matched", async () => {
    await openSet();
    await userEvent.click(
      screen.getByRole("button", { name: "Save as playlist" }),
    );
    await screen.findByTestId("history-match-report");
    await userEvent.click(await screen.findByRole("button", { name: "Stage playlist" }));
    expect(saveHistoryAsPlaylist).toHaveBeenCalledWith(
      "/lib.db",
      "2026-05-01 Basement",
      ["1"],
    );
  });

  it("says so when nothing matched rather than offering an empty playlist", async () => {
    vi.mocked(previewHistoryAsPlaylist).mockResolvedValue({
      matches: [],
      matched: 0,
      unmatched: 0,
    });
    await openSet();
    await userEvent.click(
      screen.getByRole("button", { name: "Save as playlist" }),
    );
    expect(
      await screen.findByText(
        "None of these tracks are in the library any more.",
      ),
    ).toBeInTheDocument();
    expect(saveHistoryAsPlaylist).not.toHaveBeenCalled();
  });

  it("removes one track from a set without touching the rest", async () => {
    await openSet();
    await userEvent.click(
      screen.getByRole("button", {
        name: "Remove Deleted Since from this set",
      }),
    );
    await waitFor(() => expect(removeHistoryTrack).toHaveBeenCalledWith("t2"));
    await waitFor(() =>
      expect(screen.getByTestId("history-tracks")).not.toHaveTextContent(
        "Deleted Since",
      ),
    );
    expect(screen.getByTestId("history-tracks")).toHaveTextContent(
      "Original Title",
    );
  });

  it("survives a set list that comes back null", async () => {
    vi.mocked(listHistorySets).mockResolvedValue(null as unknown as never);
    renderView();
    await waitFor(() => expect(listHistorySets).toHaveBeenCalled());
    expect(await screen.findByTestId("history-empty")).toBeInTheDocument();
  });
});
