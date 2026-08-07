import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CueEditor } from "./CueEditor";
import {
  beatJumpPosition,
  stageCueAdd,
  stageCueDelete,
  stageCueEdit,
  stageGridShift,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { HotCue, Track } from "../types";

vi.mock("../ipc", () => ({
  stageCueAdd: vi.fn(),
  stageCueDelete: vi.fn(),
  stageCueEdit: vi.fn(),
  stageGridShift: vi.fn(),
  beatJumpPosition: vi.fn(),
}));

const TRACK: Track = {
  id: "t1",
  title: "Test",
  artist: "A",
  album: null,
  genre: null,
  musical_key: "8A",
  bpm: 120,
  duration_secs: 300,
  rating: null,
  comment: null,
  folder_path: "/music/a.mp3",
  analysis_data_path: null,
  file_type: 1,
  sample_rate: null,
  bit_rate: null,
  release_year: null,
  dj_play_count: null,
  energy: null,
};

function cue(id: string, slot: number, inMsec: number, out?: number): HotCue {
  return {
    id,
    content_id: "t1",
    in_msec: inMsec,
    out_msec: out ?? null,
    kind: slot === 0 ? "MemoryCue" : { HotCue: slot },
    color: -1,
    comment: null,
  };
}

function renderEditor(cues: HotCue[] = [], positionMs = 5000) {
  const onSeek = vi.fn();
  const onChanged = vi.fn();
  render(
    <WithProviders>
      <CueEditor
        libraryPath="/lib.db"
        track={TRACK}
        cues={cues}
        positionMs={positionMs}
        onSeek={onSeek}
        onChanged={onChanged}
      />
    </WithProviders>,
  );
  return { onSeek, onChanged };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(stageCueAdd).mockResolvedValue("ch1");
  vi.mocked(stageCueDelete).mockResolvedValue("ch2");
  vi.mocked(stageCueEdit).mockResolvedValue("ch3");
  vi.mocked(stageGridShift).mockResolvedValue(["ch4", "ch5"]);
  vi.mocked(beatJumpPosition).mockResolvedValue(7500);
});

describe("CueEditor", () => {
  it("stages a cue at the playhead when an empty slot is clicked", async () => {
    const user = userEvent.setup();
    const { onChanged } = renderEditor([], 5000);
    await user.click(screen.getByLabelText("Set cue 3"));
    await waitFor(() => {
      expect(stageCueAdd).toHaveBeenCalledWith(
        "/lib.db",
        "t1",
        { in_msec: 5000, kind: 3 },
        "beat",
      );
    });
    expect(onChanged).toHaveBeenCalled();
  });

  it("seeks instead of staging when the slot is already occupied", async () => {
    const user = userEvent.setup();
    const { onSeek } = renderEditor([cue("c1", 3, 1234)]);
    await user.click(screen.getByLabelText("Play cue 3"));
    expect(onSeek).toHaveBeenCalledWith(1234);
    expect(stageCueAdd).not.toHaveBeenCalled();
  });

  it("passes null for quantize when the toggle is off", async () => {
    const user = userEvent.setup();
    renderEditor([], 5000);
    await user.click(screen.getByLabelText("Quantize"));
    await user.click(screen.getByLabelText("Set cue 1"));
    await waitFor(() => {
      expect(stageCueAdd).toHaveBeenCalledWith(
        "/lib.db",
        "t1",
        { in_msec: 5000, kind: 1 },
        null,
      );
    });
  });

  it("sends the chosen resolution", async () => {
    const user = userEvent.setup();
    renderEditor([], 5000);
    await user.selectOptions(screen.getByLabelText("Quantize resolution"), "bar");
    await user.click(screen.getByLabelText("Set cue 1"));
    await waitFor(() => {
      expect(stageCueAdd).toHaveBeenCalledWith(
        "/lib.db",
        "t1",
        expect.anything(),
        "bar",
      );
    });
  });

  it("sets a cue with the number keys and deletes with the modifier", async () => {
    const user = userEvent.setup();
    renderEditor([cue("c1", 2, 1000)], 5000);

    await user.keyboard("5");
    await waitFor(() => expect(stageCueAdd).toHaveBeenCalled());

    await user.keyboard("{Control>}2{/Control}");
    await waitFor(() => expect(stageCueDelete).toHaveBeenCalledWith("/lib.db", "c1"));
  });

  it("toggles quantize with Q", async () => {
    const user = userEvent.setup();
    renderEditor();
    expect(screen.getByLabelText("Quantize")).toBeChecked();
    await user.keyboard("q");
    expect(screen.getByLabelText("Quantize")).not.toBeChecked();
  });

  it("computes loop length from the track BPM", async () => {
    const user = userEvent.setup();
    // 120 BPM → 500ms per beat, so 8 beats = 4000ms after the in-point.
    renderEditor([cue("c1", 1, 1000)]);
    await user.selectOptions(screen.getByLabelText("Loop length for cue 1"), "8");
    await waitFor(() => {
      expect(stageCueEdit).toHaveBeenCalledWith("/lib.db", "c1", "OutMsec", 5000, null);
    });
  });

  it("moves a cue to the playhead", async () => {
    const user = userEvent.setup();
    renderEditor([cue("c1", 1, 1000)], 8200);
    await user.click(screen.getByLabelText("Move cue 1 to playhead"));
    await waitFor(() => {
      expect(stageCueEdit).toHaveBeenCalledWith("/lib.db", "c1", "InMsec", 8200, 1000);
    });
  });

  it("stages a colour change", async () => {
    const user = userEvent.setup();
    renderEditor([cue("c1", 1, 1000)]);
    await user.selectOptions(screen.getByLabelText("Colour for cue 1"), "4");
    await waitFor(() => {
      expect(stageCueEdit).toHaveBeenCalledWith("/lib.db", "c1", "Color", 4, -1);
    });
  });

  it("nudges the grid and reports how many cues followed", async () => {
    const user = userEvent.setup();
    renderEditor([cue("c1", 1, 1000)]);
    await user.click(screen.getByLabelText("Nudge grid 10ms"));
    await waitFor(() => {
      expect(stageGridShift).toHaveBeenCalledWith("/lib.db", "t1", 10);
    });
    expect(
      await screen.findByText(/Staged 2 cue move\(s\) following the grid/),
    ).toBeInTheDocument();
  });

  it("says so when no cues sit on the grid", async () => {
    const user = userEvent.setup();
    vi.mocked(stageGridShift).mockResolvedValueOnce([]);
    renderEditor([cue("c1", 1, 1000)]);
    await user.click(screen.getByLabelText("Nudge grid 10ms"));
    expect(await screen.findByText("No on-grid cues to move.")).toBeInTheDocument();
  });

  it("beat jumps through the backend and seeks to the result", async () => {
    const user = userEvent.setup();
    const { onSeek } = renderEditor([], 5000);
    await user.click(screen.getByLabelText("Beat jump 16"));
    await waitFor(() => {
      expect(beatJumpPosition).toHaveBeenCalledWith("/lib.db", "t1", 5000, 16);
    });
    expect(onSeek).toHaveBeenCalledWith(7500);
  });

  it("marks loops in the list", () => {
    renderEditor([cue("c1", 1, 1000, 5000)]);
    expect(screen.getByText("loop")).toBeInTheDocument();
  });

  it("shows an empty state", () => {
    renderEditor([]);
    expect(
      screen.getByText(/No cues yet. Press 1–8 to set one at the playhead./),
    ).toBeInTheDocument();
  });

  it("refuses to build a loop on a track with no BPM", async () => {
    const user = userEvent.setup();
    render(
      <WithProviders>
        <CueEditor
          libraryPath="/lib.db"
          track={{ ...TRACK, bpm: null }}
          cues={[cue("c1", 1, 1000)]}
          positionMs={0}
        />
      </WithProviders>,
    );
    await user.selectOptions(screen.getByLabelText("Loop length for cue 1"), "8");
    expect(
      await screen.findByText(/Track has no BPM — analyse it before making loops./),
    ).toBeInTheDocument();
    expect(stageCueEdit).not.toHaveBeenCalled();
  });
});
