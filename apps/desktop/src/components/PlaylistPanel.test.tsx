import { render, screen, fireEvent, within } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PlaylistPanel } from "./PlaylistPanel";
import { getPlaylist, listPlaylists } from "../ipc";

vi.mock("../ipc", () => ({
  listPlaylists: vi.fn(),
  getPlaylist: vi.fn(),
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listPlaylists).mockResolvedValue([
    { id: "2", name: "Techno Set", kind: "Playlist", parent_id: "1", seq: 1 },
    { id: "3", name: "House Vibes", kind: "Playlist", parent_id: "1", seq: 2 },
  ]);
  vi.mocked(getPlaylist).mockResolvedValue({
    playlist: { id: "2", name: "Techno Set", kind: "Playlist", parent_id: "1", seq: 1 },
    tracks: [
      {
        id: "1",
        title: "Dark Matter",
        artist: "Surgeon",
        album: null,
        genre: "Techno",
        musical_key: "8A",
        bpm: 140,
        duration_secs: 360,
        rating: null,
        comment: null,
        folder_path: null,
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
      },
    ],
  });
});

describe("PlaylistPanel", () => {
  it("fills the available main panel height", async () => {
    render(<PlaylistPanel libraryPath="/db" />, { wrapper });
    const panel = await screen.findByTestId("playlist-panel");
    expect(panel).toHaveClass("flex-1");
    expect(panel).toHaveClass("min-h-0");
  });

  it("renders playlists and selected playlist tracks", async () => {
    render(<PlaylistPanel libraryPath="/db" />, { wrapper });
    expect(await screen.findByText("Techno Set")).toBeInTheDocument();
    expect(await screen.findByText("Dark Matter")).toBeInTheDocument();
  });

  it("filters playlists by name", async () => {
    render(<PlaylistPanel libraryPath="/db" />, { wrapper });
    await screen.findByText("Techno Set");
    fireEvent.change(screen.getByPlaceholderText("Filter playlists…"), {
      target: { value: "house" },
    });
    expect(screen.queryByText("Techno Set")).not.toBeInTheDocument();
    expect(screen.getByText("House Vibes")).toBeInTheDocument();
  });

  it("loads tracks when selecting a playlist", async () => {
    render(<PlaylistPanel libraryPath="/db" />, { wrapper });
    fireEvent.click(await screen.findByText("House Vibes"));
    expect(getPlaylist).toHaveBeenCalledWith("/db", "3");
  });

  it("selects a track from the playlist detail", async () => {
    const onSelectTrack = vi.fn();
    render(<PlaylistPanel libraryPath="/db" onSelectTrack={onSelectTrack} />, {
      wrapper,
    });
    // Scoped to the row list: the timeline above it also has a bar per track,
    // and its label starts with the title too.
    const list = await screen.findByTestId("playlist-track-list");
    fireEvent.click(within(list).getByRole("button", { name: /Dark Matter/ }));
    expect(onSelectTrack).toHaveBeenCalledWith(
      expect.objectContaining({ id: "1", title: "Dark Matter" }),
    );
  });

  it("renders folder hierarchy with collapsible folders", async () => {
    vi.mocked(listPlaylists).mockResolvedValue([
      { id: "1", name: "Gigs", kind: "Folder", parent_id: null, seq: 1 },
      { id: "2", name: "Friday Night", kind: "Playlist", parent_id: "1", seq: 1 },
      { id: "3", name: "Saturday Set", kind: "Playlist", parent_id: "1", seq: 2 },
      { id: "4", name: "Reference", kind: "Playlist", parent_id: null, seq: 2 },
    ]);
    render(<PlaylistPanel libraryPath="/db" />, { wrapper });

    // Folder + its children appear (auto-expanded on first load).
    //
    // Wait on a *child*, not on the folder: auto-expansion happens in an effect
    // after the first render with data, so "Gigs" is on screen one render
    // before its children are. Waiting on the folder and then asserting
    // synchronously catches that intermediate render whenever the machine is
    // slow enough.
    expect(await screen.findByText("Friday Night")).toBeInTheDocument();
    expect(screen.getByText("Gigs")).toBeInTheDocument();
    expect(screen.getByText("Saturday Set")).toBeInTheDocument();
    expect(screen.getByText("Reference")).toBeInTheDocument();

    // Collapse the folder — children disappear, folder stays.
    fireEvent.click(screen.getByRole("button", { name: /Gigs/ }));
    expect(screen.queryByText("Friday Night")).not.toBeInTheDocument();
    expect(screen.queryByText("Saturday Set")).not.toBeInTheDocument();
    expect(screen.getByText("Gigs")).toBeInTheDocument();
    expect(screen.getByText("Reference")).toBeInTheDocument();
  });

  it("shows a DUP badge and duplicate count for repeated tracks", async () => {
    const track = {
      id: "1",
      title: "Dark Matter",
      artist: "Surgeon",
      album: null,
      genre: "Techno",
      musical_key: "8A",
      bpm: 140,
      duration_secs: 360,
      rating: null,
      comment: null,
      folder_path: null,
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
    vi.mocked(getPlaylist).mockResolvedValue({
      playlist: { id: "2", name: "Techno Set", kind: "Playlist", parent_id: null, seq: 1 },
      tracks: [track, track], // same track twice — legitimate Rekordbox dupe
    });

    render(<PlaylistPanel libraryPath="/db" />, { wrapper });
    await screen.findByText("Techno Set");

    // Second occurrence gets a DUP badge.
    expect(await screen.findByText("DUP")).toBeInTheDocument();
    // Header reports the duplicate count.
    expect(screen.getByText(/1 duplicate/)).toBeInTheDocument();
  });

  it("flattens hierarchy while filtering", async () => {
    vi.mocked(listPlaylists).mockResolvedValue([
      { id: "1", name: "Gigs", kind: "Folder", parent_id: null, seq: 1 },
      { id: "2", name: "Techno Set", kind: "Playlist", parent_id: "1", seq: 1 },
      { id: "3", name: "House Vibes", kind: "Playlist", parent_id: "1", seq: 2 },
    ]);
    render(<PlaylistPanel libraryPath="/db" />, { wrapper });
    await screen.findByText("Techno Set");

    fireEvent.change(screen.getByPlaceholderText("Filter playlists…"), {
      target: { value: "house" },
    });

    // Folder hidden during filter; matching leaf shown.
    expect(screen.queryByText("Gigs")).not.toBeInTheDocument();
    expect(screen.queryByText("Techno Set")).not.toBeInTheDocument();
    expect(screen.getByText("House Vibes")).toBeInTheDocument();
  });
});
