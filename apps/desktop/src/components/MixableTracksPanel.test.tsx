import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MixableTracksPanel } from "./MixableTracksPanel";
import {
  deleteMixableTemplate,
  findMixableTracks,
  getKeyMixingMode,
  listMixableTemplates,
  mixableDefaultOptions,
  saveMixableTemplate,
  setKeyMixingMode,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { MixableOptions, MixableResult, Track } from "../types";

/** What `mixable_default_options` serves — the Rust side owns the real one. */
const BASIC_OPTIONS: MixableOptions = {
  bpm_tolerance_pct: 6,
  match_key: true,
  key_mixing_mode: "harmonically_compatible",
  include_half_double: false,
  must_have_cues: false,
  genres: [],
  year: { kind: "off" },
  energy: { kind: "off" },
  rating: { kind: "off" },
  must_have_tags: [],
  must_not_have_tags: [],
  limit: 25,
};

vi.mock("../ipc", () => ({
  findMixableTracks: vi.fn(),
  mixableDefaultOptions: vi.fn(),
  getKeyMixingMode: vi.fn(),
  setKeyMixingMode: vi.fn(),
  listMixableTemplates: vi.fn(),
  saveMixableTemplate: vi.fn(),
  deleteMixableTemplate: vi.fn(),
}));

function track(id: string, over: Partial<Track> = {}): Track {
  return {
    id,
    title: `Track ${id}`,
    artist: "Someone",
    album: null,
    genre: null,
    musical_key: "8A",
    bpm: 128,
    duration_secs: null,
    rating: null,
    comment: null,
    folder_path: null,
    analysis_data_path: null,
    file_type: null,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    energy: null,
    ...over,
  } as Track;
}

const SOURCE = track("s", { title: "Seed", bpm: 128 });

const RESULT: MixableResult = {
  source: SOURCE,
  considered: 4213,
  compatible_keys: ["8A", "8B", "7A", "9A"],
  matches: [
    {
      track: track("a", { title: "Perfect", bpm: 128.4 }),
      score: 100,
      reasons: ["Perfect Harmonic Match", "Perfect BPM Match (128.0 vs 128.4)"],
      bpm_relation: "direct",
      key_relation: "same",
    },
    {
      track: track("b", { title: "Halftime", bpm: 64 }),
      score: 82,
      reasons: ["Perfect Harmonic Match", "Half-time match"],
      bpm_relation: "half",
      key_relation: "same",
    },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(findMixableTracks).mockResolvedValue(RESULT);
  vi.mocked(mixableDefaultOptions).mockResolvedValue(BASIC_OPTIONS);
  vi.mocked(getKeyMixingMode).mockResolvedValue("harmonically_compatible");
  vi.mocked(setKeyMixingMode).mockResolvedValue(undefined);
  vi.mocked(listMixableTemplates).mockResolvedValue([]);
  vi.mocked(saveMixableTemplate).mockResolvedValue("t1");
  vi.mocked(deleteMixableTemplate).mockResolvedValue(true);
});

function renderPanel(over: Partial<Parameters<typeof MixableTracksPanel>[0]> = {}) {
  const onUseAsNextTrack = vi.fn();
  render(
    <WithProviders>
      <MixableTracksPanel
        libraryPath="/lib.db"
        track={SOURCE}
        onUseAsNextTrack={onUseAsNextTrack}
        onClose={vi.fn()}
        {...over}
      />
    </WithProviders>,
  );
  return { onUseAsNextTrack };
}

describe("MixableTracksPanel", () => {
  it("reports how much of the library survived the rules, not just the count", async () => {
    renderPanel();
    // "2" alone reads as "there are only two"; "2 of 4213" reads as
    // "the rules are tight", which is the actionable version.
    expect(await screen.findByTestId("mixable-count")).toHaveTextContent(
      "2 of 4213 track(s) mix out of this one.",
    );
  });

  it("opens in basic mode with the advanced rules hidden", async () => {
    renderPanel();
    await screen.findByTestId("mixable-results");
    expect(screen.queryByTestId("advanced-rules")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Advanced rules" }));
    expect(screen.getByTestId("advanced-rules")).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "Must have cue points" }),
    ).toBeInTheDocument();
  });

  it("names the fields it does not offer rather than showing dead controls", async () => {
    renderPanel();
    await screen.findByTestId("mixable-results");
    await userEvent.click(screen.getByRole("button", { name: "Advanced rules" }));
    expect(screen.getByTestId("advanced-rules")).toHaveTextContent(
      /Colour, date added, Popularity, Danceability and Happiness are not offered/,
    );
  });

  it("shows the compatible key set for the seed track", async () => {
    renderPanel();
    expect(await screen.findByTestId("compatible-keys")).toHaveTextContent(
      "Mixes into 8A, 8B, 7A, 9A",
    );
  });

  it("labels a half-time candidate as one", async () => {
    renderPanel();
    const results = await screen.findByTestId("mixable-results");
    expect(results).toHaveTextContent(/half-time/);
  });

  it("re-seeds the search from Use as next track", async () => {
    const { onUseAsNextTrack } = renderPanel();
    await screen.findByTestId("mixable-results");
    const buttons = screen.getAllByRole("button", { name: "Use as next track" });
    await userEvent.click(buttons[0]);
    expect(onUseAsNextTrack).toHaveBeenCalledWith(
      expect.objectContaining({ id: "a" }),
    );
  });

  it("re-runs the search when a rule changes", async () => {
    renderPanel();
    await screen.findByTestId("mixable-results");
    const before = vi.mocked(findMixableTracks).mock.calls.length;

    await userEvent.click(screen.getByRole("checkbox", { name: "Match key" }));

    await waitFor(() => {
      expect(vi.mocked(findMixableTracks).mock.calls.length).toBeGreaterThan(
        before,
      );
    });
    const last = vi.mocked(findMixableTracks).mock.calls.at(-1);
    expect(last?.[2]).toMatchObject({ match_key: false });
  });

  it("persists the key mixing mode globally, not per search", async () => {
    renderPanel();
    await screen.findByTestId("mixable-results");
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Key mixing mode" }),
      "fuzzy",
    );
    expect(setKeyMixingMode).toHaveBeenCalledWith("fuzzy");
  });

  it("saves and reloads a template", async () => {
    renderPanel();
    await screen.findByTestId("mixable-results");
    await userEvent.click(screen.getByRole("button", { name: "Advanced rules" }));
    await userEvent.click(
      screen.getByRole("checkbox", { name: "Must have cue points" }),
    );
    await userEvent.type(
      screen.getByRole("textbox", { name: "Template name" }),
      "Peak time",
    );

    vi.mocked(listMixableTemplates).mockResolvedValue([
      {
        id: "t1",
        name: "Peak time",
        options: { ...BASIC_OPTIONS, must_have_cues: true },
        created_at: 1770000000,
      },
    ]);
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(saveMixableTemplate).toHaveBeenCalledWith(
      "Peak time",
      expect.objectContaining({ must_have_cues: true }),
    );
    expect(await screen.findByTestId("mixable-templates")).toHaveTextContent(
      "Peak time",
    );
  });

  it("says what to loosen when nothing matched", async () => {
    vi.mocked(findMixableTracks).mockResolvedValue({
      ...RESULT,
      matches: [],
    });
    renderPanel();
    expect(
      await screen.findByText(/Widen the BPM range, or turn off Match key/),
    ).toBeInTheDocument();
  });

  it("does not search before the backend has served the default rules", async () => {
    // The renderer has no copy of basic mode, so searching early would search
    // with rules nobody chose.
    let resolve: (o: MixableOptions) => void = () => {};
    vi.mocked(mixableDefaultOptions).mockReturnValue(
      new Promise<MixableOptions>((r) => {
        resolve = r;
      }),
    );
    renderPanel();
    expect(await screen.findByText("Loading rules…")).toBeInTheDocument();
    expect(findMixableTracks).not.toHaveBeenCalled();
    resolve(BASIC_OPTIONS);
    await screen.findByTestId("mixable-results");
    expect(findMixableTracks).toHaveBeenCalledTimes(1);
  });

  it("asks for a track rather than searching with none", async () => {
    renderPanel({ track: null });
    expect(
      await screen.findByText("Select a track to see what mixes out of it."),
    ).toBeInTheDocument();
    expect(findMixableTracks).not.toHaveBeenCalled();
  });

  it("survives a template list that comes back empty or broken", async () => {
    // One unparseable stored template must not take the panel down; the
    // backend already drops those, so the panel only has to tolerate null.
    vi.mocked(listMixableTemplates).mockResolvedValue(
      null as unknown as never,
    );
    renderPanel();
    await screen.findByTestId("mixable-results");
    expect(screen.queryByTestId("mixable-templates")).not.toBeInTheDocument();
  });
});
