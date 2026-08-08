import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FindPopup } from "./FindPopup";
import type { Playlist, Smartlist, Track } from "../types";

function track(id: string, title: string, artist: string | null): Track {
  return {
    id,
    title,
    artist,
    album: null,
    genre: null,
    musical_key: null,
    bpm: 128,
    duration_secs: 200,
    rating: null,
    comment: null,
    folder_path: `/music/${id}.mp3`,
    analysis_data_path: null,
    file_type: null,
    sample_rate: null,
    bit_rate: null,
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

const TRACKS = [
  track("1", "Acid Rain", "Aphex Twin"),
  track("2", "Braindance", "Surgeon"),
];

const PLAYLISTS: Playlist[] = [
  { id: "p1", name: "Rainy Warmup", kind: "Playlist", parent_id: null, seq: 1 },
];

const SMARTLISTS = [
  {
    id: "s1",
    name: "Rain Selection",
    parent_folder_id: null,
    combinator: "All",
    clauses: [{ rules: [{ field: "genre", op: "contains", value: "House" }] }],
    created_at: 0,
    updated_at: 0,
  },
] as unknown as Smartlist[];

function renderPopup(overrides: Partial<Parameters<typeof FindPopup>[0]> = {}) {
  const props = {
    open: true,
    onClose: vi.fn(),
    tracks: TRACKS,
    playlists: PLAYLISTS,
    smartlists: SMARTLISTS,
    selectedTracks: [] as Track[],
    onPlayTrack: vi.fn(),
    onQueueTrack: vi.fn(),
    onOpenPlaylist: vi.fn(),
    onOpenSmartlist: vi.fn(),
    ...overrides,
  };
  render(<FindPopup {...props} />);
  return { ...props, user: userEvent.setup() };
}

describe("FindPopup", () => {
  it("renders nothing when closed", () => {
    renderPopup({ open: false });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("opens focused, showing a prompt rather than the whole library", () => {
    renderPopup();
    expect(screen.getByLabelText("Find in library")).toHaveFocus();
    expect(screen.getByText(/Start typing to search/)).toBeInTheDocument();
  });

  it("searches playlists, smartlists and tracks in one box", async () => {
    const { user } = renderPopup();
    await user.type(screen.getByLabelText("Find in library"), "rain");

    const results = screen.getByTestId("find-results");
    expect(results).toHaveTextContent("Rainy Warmup");
    expect(results).toHaveTextContent("Rain Selection");
    expect(results).toHaveTextContent("Acid Rain");
  });

  it("says so when nothing matches", async () => {
    const { user } = renderPopup();
    await user.type(screen.getByLabelText("Find in library"), "zzzz");
    expect(await screen.findByTestId("find-empty")).toHaveTextContent(
      /Nothing matches/,
    );
  });

  it("Enter opens the highlighted container", async () => {
    // Containers sort first, so the first result is the playlist.
    const { user, onOpenPlaylist, onClose } = renderPopup();
    await user.type(screen.getByLabelText("Find in library"), "rain");
    await user.keyboard("{Enter}");
    expect(onOpenPlaylist).toHaveBeenCalledWith("p1");
    expect(onClose).toHaveBeenCalled();
  });

  it("Enter plays a highlighted track", async () => {
    const { user, onPlayTrack } = renderPopup();
    await user.type(screen.getByLabelText("Find in library"), "acid");
    await user.keyboard("{Enter}");
    expect(onPlayTrack).toHaveBeenCalledWith(
      expect.objectContaining({ id: "1" }),
    );
  });

  it("arrow keys walk the results across sections", async () => {
    const { user, onOpenSmartlist } = renderPopup();
    await user.type(screen.getByLabelText("Find in library"), "rain");
    // playlist → smartlist
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onOpenSmartlist).toHaveBeenCalledWith("s1");
  });

  it("does not walk off either end of the list", async () => {
    const { user, onOpenPlaylist } = renderPopup();
    await user.type(screen.getByLabelText("Find in library"), "rain");
    await user.keyboard("{ArrowUp}{ArrowUp}{Enter}");
    // Still on the first result rather than wrapping to the last.
    expect(onOpenPlaylist).toHaveBeenCalledWith("p1");
  });

  it("Escape closes", async () => {
    const { user, onClose } = renderPopup();
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });

  it("queues a track without playing it", async () => {
    const { user, onQueueTrack, onPlayTrack } = renderPopup();
    await user.type(screen.getByLabelText("Find in library"), "acid");
    await user.click(
      screen.getByRole("button", { name: "Add Acid Rain to queue" }),
    );
    expect(onQueueTrack).toHaveBeenCalledWith(
      expect.objectContaining({ id: "1" }),
    );
    expect(onPlayTrack).not.toHaveBeenCalled();
  });

  it("offers add-to-playlist only when something is selected", async () => {
    const { user } = renderPopup();
    await user.type(screen.getByLabelText("Find in library"), "rain");
    // No selection, so the action would add nothing — it is not offered.
    expect(
      screen.queryByRole("button", { name: /selected track/ }),
    ).not.toBeInTheDocument();
  });

  it("adds the selection to a playlist", async () => {
    const onAddSelectionToPlaylist = vi.fn();
    const { user } = renderPopup({
      selectedTracks: TRACKS,
      onAddSelectionToPlaylist,
    });
    await user.type(screen.getByLabelText("Find in library"), "rain");
    await user.click(
      screen.getByRole("button", {
        name: "Add 2 selected track(s) to Rainy Warmup",
      }),
    );
    expect(onAddSelectionToPlaylist).toHaveBeenCalledWith("p1", "Rainy Warmup");
  });

  it("resets its query between openings", async () => {
    // A popup that reopens showing the last search is answering a stale
    // question.
    const props = {
      onClose: vi.fn(),
      tracks: TRACKS,
      playlists: PLAYLISTS,
      smartlists: SMARTLISTS,
      selectedTracks: [] as Track[],
      onPlayTrack: vi.fn(),
      onQueueTrack: vi.fn(),
      onOpenPlaylist: vi.fn(),
      onOpenSmartlist: vi.fn(),
    };
    const user = userEvent.setup();
    const { rerender } = render(<FindPopup open {...props} />);
    await user.type(screen.getByLabelText("Find in library"), "rain");
    expect(screen.getByLabelText("Find in library")).toHaveValue("rain");

    rerender(<FindPopup open={false} {...props} />);
    rerender(<FindPopup open {...props} />);
    expect(screen.getByLabelText("Find in library")).toHaveValue("");
  });
});
