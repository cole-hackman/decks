import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FavouritePlaylistsBar } from "./FavouritePlaylistsBar";
import {
  addTracksToPlaylist,
  listFavouritePlaylists,
  toggleFavouritePlaylist,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { FavouritePlaylist } from "../types";
import { TRACK_IDS_MIME } from "../lib/track-drag";

vi.mock("../ipc", () => ({
  listFavouritePlaylists: vi.fn(),
  toggleFavouritePlaylist: vi.fn(),
  addTracksToPlaylist: vi.fn(),
}));

const FAVOURITES: FavouritePlaylist[] = [
  { playlist_id: "p1", name: "Warmup", seq: 1, track_count: 12 },
  { playlist_id: "p2", name: "Peak", seq: 2, track_count: 40 },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listFavouritePlaylists).mockResolvedValue(FAVOURITES);
  vi.mocked(toggleFavouritePlaylist).mockResolvedValue(false);
  vi.mocked(addTracksToPlaylist).mockResolvedValue(["c1", "c2"]);
});

function renderBar(selected: string[] = ["t1", "t2"]) {
  const onOpenPlaylist = vi.fn();
  render(
    <WithProviders>
      <FavouritePlaylistsBar
        libraryPath="/lib.db"
        selectedTrackIds={new Set(selected)}
        onOpenPlaylist={onOpenPlaylist}
      />
    </WithProviders>,
  );
  return { onOpenPlaylist };
}

describe("FavouritePlaylistsBar", () => {
  it("renders nothing when nothing is starred", async () => {
    vi.mocked(listFavouritePlaylists).mockResolvedValue([]);
    renderBar();
    await waitFor(() => expect(listFavouritePlaylists).toHaveBeenCalled());
    expect(screen.queryByTestId("favourite-playlists")).not.toBeInTheDocument();
  });

  it("shows each favourite with its hotkey position and count", async () => {
    renderBar();
    const bar = await screen.findByTestId("favourite-playlists");
    expect(bar).toHaveTextContent("1Warmup12");
    expect(bar).toHaveTextContent("2Peak40");
  });

  it("says what the keys do rather than leaving numbers as decoration", async () => {
    renderBar();
    const bar = await screen.findByTestId("favourite-playlists");
    expect(bar).toHaveTextContent(
      "1–9 opens · Shift+1–9 or drag files the selection",
    );
  });

  it("a digit opens the favourite at that position", async () => {
    const { onOpenPlaylist } = renderBar();
    await screen.findByTestId("favourite-playlists");
    await userEvent.keyboard("2");
    expect(onOpenPlaylist).toHaveBeenCalledWith("p2");
    expect(addTracksToPlaylist).not.toHaveBeenCalled();
  });

  it("shift plus a digit files the selection instead", async () => {
    const { onOpenPlaylist } = renderBar(["t1", "t2"]);
    await screen.findByTestId("favourite-playlists");
    // `{Shift>}1{/Shift}` — with Shift held the key is "!", so the handler has
    // to read e.code, not e.key.
    await userEvent.keyboard("{Shift>}1{/Shift}");
    await waitFor(() =>
      expect(addTracksToPlaylist).toHaveBeenCalledWith("/lib.db", "p1", [
        "t1",
        "t2",
      ]),
    );
    expect(onOpenPlaylist).not.toHaveBeenCalled();
  });

  it("a digit with no favourite behind it does nothing", async () => {
    const { onOpenPlaylist } = renderBar();
    await screen.findByTestId("favourite-playlists");
    await userEvent.keyboard("7");
    expect(onOpenPlaylist).not.toHaveBeenCalled();
  });

  it("never steals a digit from a text field", async () => {
    const { onOpenPlaylist } = renderBar();
    await screen.findByTestId("favourite-playlists");
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();
    await userEvent.keyboard("1");
    expect(onOpenPlaylist).not.toHaveBeenCalled();
    expect(input.value).toBe("1");
    input.remove();
  });

  it("leaves modified chords to whatever owns them", async () => {
    const { onOpenPlaylist } = renderBar();
    await screen.findByTestId("favourite-playlists");
    await userEvent.keyboard("{Meta>}1{/Meta}");
    expect(onOpenPlaylist).not.toHaveBeenCalled();
  });

  it("says how many were already there rather than reporting a silent partial", async () => {
    // Two selected, one staged — the other was already in the playlist.
    vi.mocked(addTracksToPlaylist).mockResolvedValue(["c1"]);
    renderBar(["t1", "t2"]);
    await screen.findByTestId("favourite-playlists");
    await userEvent.click(
      screen.getByRole("button", { name: "File selection into Warmup" }),
    );
    expect(
      await screen.findByText("Staged 1 track(s) for Warmup."),
    ).toBeInTheDocument();
    expect(screen.getByText("1 already there.")).toBeInTheDocument();
  });

  it("says so when every selected track was already there", async () => {
    vi.mocked(addTracksToPlaylist).mockResolvedValue([]);
    renderBar(["t1"]);
    await screen.findByTestId("favourite-playlists");
    await userEvent.click(
      screen.getByRole("button", { name: "File selection into Peak" }),
    );
    expect(await screen.findByText("Already in Peak.")).toBeInTheDocument();
  });

  it("filing nothing says so rather than staging an empty change", async () => {
    renderBar([]);
    await screen.findByTestId("favourite-playlists");
    await userEvent.click(
      screen.getByRole("button", { name: "File selection into Warmup" }),
    );
    expect(await screen.findByText("Nothing selected to file.")).toBeInTheDocument();
    expect(addTracksToPlaylist).not.toHaveBeenCalled();
  });

  it("unstars from the bar and refetches", async () => {
    renderBar();
    await screen.findByTestId("favourite-playlists");
    vi.mocked(listFavouritePlaylists).mockResolvedValue([FAVOURITES[1]]);
    await userEvent.click(screen.getByRole("button", { name: "Unstar Warmup" }));
    await waitFor(() =>
      expect(toggleFavouritePlaylist).toHaveBeenCalledWith("/lib.db", "p1"),
    );
    await waitFor(() =>
      expect(screen.getByTestId("favourite-playlists")).not.toHaveTextContent(
        "Warmup",
      ),
    );
  });

  it("survives a favourites list that comes back null", async () => {
    // The bar sits above the browser; it must never take the view down.
    vi.mocked(listFavouritePlaylists).mockResolvedValue(
      null as unknown as never,
    );
    renderBar();
    await waitFor(() => expect(listFavouritePlaylists).toHaveBeenCalled());
    expect(screen.queryByTestId("favourite-playlists")).not.toBeInTheDocument();
  });

  /** A DataTransfer stand-in — jsdom does not construct one. */
  function transfer(payload: string | null) {
    return {
      types: payload === null ? [] : [TRACK_IDS_MIME],
      getData: () => payload ?? "",
      dropEffect: "",
      setData: () => {},
    };
  }

  it("files tracks dropped onto a favourite", async () => {
    renderBar();
    const chip = (await screen.findByText("Warmup")).closest("span")!;

    fireEvent.dragOver(chip, { dataTransfer: transfer("t1\nt2") });
    fireEvent.drop(chip, { dataTransfer: transfer("t1\nt2") });

    await waitFor(() =>
      expect(vi.mocked(addTracksToPlaylist)).toHaveBeenCalledWith(
        "/lib.db",
        "p1",
        ["t1", "t2"],
      ),
    );
  });

  it("files what was dragged, not what happens to be selected now", async () => {
    // The selection can change between the drag starting and the drop; the
    // payload is the record of what the user picked up.
    renderBar(["other"]);
    const chip = (await screen.findByText("Warmup")).closest("span")!;

    fireEvent.drop(chip, { dataTransfer: transfer("t1") });

    await waitFor(() =>
      expect(vi.mocked(addTracksToPlaylist)).toHaveBeenCalledWith(
        "/lib.db",
        "p1",
        ["t1"],
      ),
    );
  });

  it("ignores a drag that is not ours", async () => {
    // Without the type check the chip would light up for a dragged file and
    // then do nothing.
    renderBar();
    const chip = (await screen.findByText("Warmup")).closest("span")!;

    fireEvent.drop(chip, { dataTransfer: transfer(null) });
    expect(vi.mocked(addTracksToPlaylist)).not.toHaveBeenCalled();
  });
});
