import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TrackMatcherView } from "./TrackMatcherView";
import {
  createPlaylistFromTracks,
  matchTracks,
  parseCsvForMatcher,
  parseCsvHeadersForMatcher,
  parseTracklistForMatcher,
  storeLinksForTracks,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({
  matchTracks: vi.fn(),
  createPlaylistFromTracks: vi.fn(),
  parseCsvForMatcher: vi.fn(),
  parseCsvHeadersForMatcher: vi.fn(),
  parseTracklistForMatcher: vi.fn(),
  storeLinksForTracks: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
});

function render_() {
  return render(
    <WithProviders>
      <TrackMatcherView libraryPath="/db" />
    </WithProviders>,
  );
}

describe("TrackMatcherView", () => {
  it("parses pasted lines and calls matchTracks", async () => {
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "Title",
        input_artist: "Artist",
        track: { id: "t1", title: "Title", artist: "Artist" },
        score: 1.0,
        status: "Exact",
      },
    ]);
    render_();
    const textarea = screen.getByPlaceholderText(/Artist - Title/);
    await userEvent.type(textarea, "Artist - Title");
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    expect(matchTracks).toHaveBeenCalledWith("/db", [
      { title: "Title", artist: "Artist" },
    ]);
    expect(await screen.findByText(/1 \/ 1 tracks matched/)).toBeInTheDocument();
  });

  it("treats a line without ' - ' as just a title", async () => {
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "Lone Title",
        input_artist: null,
        track: null,
        score: 0,
        status: "Unmatched",
      },
    ]);
    render_();
    await userEvent.type(
      screen.getByPlaceholderText(/Artist - Title/),
      "Lone Title",
    );
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    expect(matchTracks).toHaveBeenCalledWith("/db", [{ title: "Lone Title" }]);
  });

  it("CSV upload shows column mapping UI and delegates parse to backend", async () => {
    vi.mocked(parseCsvHeadersForMatcher).mockResolvedValue(["title", "artist"]);
    vi.mocked(parseCsvForMatcher).mockResolvedValue([
      { title: "Strobe", artist: "Deadmau5" },
    ]);
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "Strobe",
        input_artist: "Deadmau5",
        track: { id: "t1", title: "Strobe", artist: "Deadmau5" },
        score: 1.0,
        status: "Exact",
      },
    ]);
    render_();
    // Switch source to CSV via the source dropdown.
    const sourceSelect = screen.getAllByRole("combobox")[0];
    await userEvent.selectOptions(sourceSelect, "csv");

    // Upload a CSV file.
    const csv = "title,artist\nStrobe,Deadmau5\n";
    const file = new File([csv], "list.csv", { type: "text/csv" });
    // jsdom's File doesn't implement .text() in this version — stub it.
    Object.defineProperty(file, "text", {
      value: () => Promise.resolve(csv),
    });
    const input = document.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    await userEvent.upload(input, file);

    // Column-mapping UI surfaces headers.
    expect(await screen.findByText(/1 rows · 2 columns/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Match" }));

    expect(parseCsvForMatcher).toHaveBeenCalledWith(csv, "title", "artist");
    expect(matchTracks).toHaveBeenCalledWith("/db", [
      { title: "Strobe", artist: "Deadmau5" },
    ]);
    expect(await screen.findByText(/1 \/ 1 tracks matched/)).toBeInTheDocument();
  });

  it("Create playlist prompts for name and stages", async () => {
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "A",
        input_artist: null,
        track: { id: "t1", title: "A", artist: null },
        score: 1.0,
        status: "Exact",
      },
    ]);
    vi.mocked(createPlaylistFromTracks).mockResolvedValue("pl-new");
    render_();
    await userEvent.type(screen.getByPlaceholderText(/Artist - Title/), "A");
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    await screen.findByText(/1 \/ 1 tracks matched/);
    await userEvent.click(
      screen.getByRole("button", { name: /Create playlist/ }),
    );
    // Dialog prompt input has a default value; click OK.
    await userEvent.click(await screen.findByRole("button", { name: "OK" }));
    expect(createPlaylistFromTracks).toHaveBeenCalledWith(
      "/db",
      "Imported (paste)",
      ["t1"],
    );
  });

  it("passes the chosen separator to parseTracklistForMatcher, which is what turns a raw tracklist into query candidates", async () => {
    vi.mocked(parseTracklistForMatcher).mockResolvedValue([
      { title: "Title", artist: "Artist" },
    ]);
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "Title",
        input_artist: "Artist",
        track: { id: "t1", title: "Title", artist: "Artist" },
        score: 1.0,
        status: "Exact",
      },
    ]);
    render_();
    const sourceSelect = screen.getAllByRole("combobox")[0];
    await userEvent.selectOptions(sourceSelect, "txt");
    const separatorSelect = screen.getAllByRole("combobox")[1];
    await userEvent.selectOptions(separatorSelect, "em_dash");
    await userEvent.type(
      screen.getByPlaceholderText(/split by the separator above/),
      "Artist — Title",
    );
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    expect(parseTracklistForMatcher).toHaveBeenCalledWith(
      "Artist — Title",
      "em_dash",
    );
  });

  it("sends the typed delimiter as a Custom separator, because free-text splitting must not silently reinterpret it as one of the presets", async () => {
    vi.mocked(parseTracklistForMatcher).mockResolvedValue([
      { title: "Title", artist: "Artist" },
    ]);
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "Title",
        input_artist: "Artist",
        track: { id: "t1", title: "Title", artist: "Artist" },
        score: 1.0,
        status: "Exact",
      },
    ]);
    render_();
    const sourceSelect = screen.getAllByRole("combobox")[0];
    await userEvent.selectOptions(sourceSelect, "txt");
    const separatorSelect = screen.getAllByRole("combobox")[1];
    await userEvent.selectOptions(separatorSelect, "custom");
    await userEvent.type(
      screen.getByPlaceholderText(/Custom separator/),
      "::",
    );
    await userEvent.type(
      screen.getByPlaceholderText(/split by the separator above/),
      "Artist::Title",
    );
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    expect(parseTracklistForMatcher).toHaveBeenCalledWith("Artist::Title", {
      custom: "::",
    });
  });

  it("stages a playlist from only the exact and fuzzy hits, keeping unmatched rows out of a playlist that would otherwise misrepresent what was found", async () => {
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "A",
        input_artist: null,
        track: { id: "t1", title: "A", artist: null },
        score: 1.0,
        status: "Exact",
      },
      {
        input_title: "B",
        input_artist: null,
        track: { id: "t2", title: "B", artist: null },
        score: 0.8,
        status: "Fuzzy",
      },
      {
        input_title: "C",
        input_artist: null,
        track: null,
        score: 0,
        status: "Unmatched",
      },
    ]);
    vi.mocked(createPlaylistFromTracks).mockResolvedValue("pl-new");
    render_();
    await userEvent.type(
      screen.getByPlaceholderText(/Artist - Title/),
      "A{enter}B{enter}C",
    );
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    await screen.findByText(/2 \/ 3 tracks matched/);
    await userEvent.click(
      screen.getByRole("button", { name: /Create playlist/ }),
    );
    await userEvent.click(await screen.findByRole("button", { name: "OK" }));
    expect(createPlaylistFromTracks).toHaveBeenCalledWith(
      "/db",
      "Imported (paste)",
      ["t1", "t2"],
    );
  });

  it("disables Create playlist when nothing matched, so there is nothing for the button to stage", async () => {
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "Lone Title",
        input_artist: null,
        track: null,
        score: 0,
        status: "Unmatched",
      },
    ]);
    render_();
    await userEvent.type(
      screen.getByPlaceholderText(/Artist - Title/),
      "Lone Title",
    );
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    await screen.findByText(/0 \/ 1 tracks matched/);
    expect(
      screen.getByRole("button", { name: /Create playlist/ }),
    ).toBeDisabled();
  });

  it("opens store search links in a new tab without granting the target page a window.opener reference back into the app", async () => {
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "Ghost Track",
        input_artist: "Nobody",
        track: null,
        score: 0,
        status: "Unmatched",
      },
    ]);
    vi.mocked(storeLinksForTracks).mockResolvedValue([
      {
        title: "Ghost Track",
        artist: "Nobody",
        links: [["Beatport", "https://www.beatport.com/search?q=Ghost+Track"]],
      },
    ]);
    render_();
    await userEvent.type(
      screen.getByPlaceholderText(/Artist - Title/),
      "Nobody - Ghost Track",
    );
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    await screen.findByText(/0 \/ 1 tracks matched/);
    await userEvent.click(
      screen.getByRole("button", { name: "Find store links" }),
    );
    const link = await screen.findByRole("link", { name: "Beatport" });
    expect(link).toHaveAttribute("target", "_blank");
    expect(link.getAttribute("rel")).toContain("noreferrer");
    expect(storeLinksForTracks).toHaveBeenCalledWith(
      [{ title: "Ghost Track", artist: "Nobody" }],
      expect.arrayContaining(["beatport", "spotify"]),
    );
  });

  it("tells the user these are search links only, since decks does not compare prices or push playlists without a registered per-user token", async () => {
    vi.mocked(matchTracks).mockResolvedValue([
      {
        input_title: "Ghost Track",
        input_artist: null,
        track: null,
        score: 0,
        status: "Unmatched",
      },
    ]);
    render_();
    await userEvent.type(
      screen.getByPlaceholderText(/Artist - Title/),
      "Ghost Track",
    );
    await userEvent.click(screen.getByRole("button", { name: "Match" }));
    expect(await screen.findByText(/Search links only/)).toBeInTheDocument();
  });
});
