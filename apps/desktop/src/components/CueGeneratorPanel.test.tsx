import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CueGeneratorPanel } from "./CueGeneratorPanel";
import { applyGeneratedCues, previewGeneratedCues, suggestAnchorRules } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { GeneratePreview, Track } from "../types";

vi.mock("../ipc", () => ({
  previewGeneratedCues: vi.fn(),
  applyGeneratedCues: vi.fn(),
  suggestAnchorRules: vi.fn(),
}));

const TRACK: Track = {
  id: "t1",
  title: "Test",
  artist: "A",
  album: null,
  genre: null,
  musical_key: null,
  bpm: 128,
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

const PREVIEW: GeneratePreview = {
  anchors: [
    { anchor: { kind: "drop", ordinal: 1 }, position_ms: 60000, confidence: "certain" },
  ],
  cues: [
    {
      position_ms: 0,
      name: "Start",
      color: 5,
      slot: 1,
      memory_cue: false,
      loop_end_ms: null,
      confidence: "certain",
      template_index: 0,
    },
    {
      position_ms: 60000,
      name: "Drop",
      color: 1,
      slot: 2,
      memory_cue: false,
      loop_end_ms: null,
      confidence: { detected: 0.35 },
      template_index: 2,
    },
  ],
  skipped: [
    { reason: "anchor_missing", name: "Breakdown", anchor: { kind: "breakdown", ordinal: 1 } },
    { reason: "duplicate_memory_cue", name: "Mix out", position_ms: 200000 },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(suggestAnchorRules).mockResolvedValue([
    { anchor: { kind: "drop", ordinal: 1 }, name: "Drop", color: null },
  ]);
  vi.mocked(previewGeneratedCues).mockResolvedValue(PREVIEW);
  vi.mocked(applyGeneratedCues).mockResolvedValue(["ch1", "ch2"]);
});

function renderPanel() {
  const onChanged = vi.fn();
  render(
    <WithProviders>
      <CueGeneratorPanel libraryPath="/lib.db" track={TRACK} onChanged={onChanged} />
    </WithProviders>,
  );
  return { onChanged };
}

describe("CueGeneratorPanel", () => {
  it("suggests anchors from the track's existing cues", async () => {
    renderPanel();
    await waitFor(() => {
      expect(suggestAnchorRules).toHaveBeenCalledWith("/lib.db", "t1");
    });
    expect(await screen.findByLabelText("Remove Drop anchor")).toBeInTheDocument();
  });

  it("says so when no anchors could be suggested, and disables staging", async () => {
    vi.mocked(suggestAnchorRules).mockResolvedValue([]);
    renderPanel();
    expect(await screen.findByTestId("no-anchors")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stage cues" })).toBeDisabled();
  });

  it("is explicit that detection is not implemented", () => {
    renderPanel();
    expect(
      screen.getByText(/Automatic drop detection is not implemented yet/),
    ).toBeInTheDocument();
  });

  it("previews the cues a template would create", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByLabelText("Remove Drop anchor");
    await user.click(screen.getByRole("button", { name: "Preview" }));

    expect(await screen.findByTestId("generator-preview")).toBeInTheDocument();
    expect(screen.getByText("Would create 2 cue(s)")).toBeInTheDocument();
    expect(screen.getByText("Start")).toBeInTheDocument();
    expect(screen.getByText("1:00")).toBeInTheDocument();
  });

  it("marks low-confidence cues as provisional rather than presenting them as fact", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByLabelText("Remove Drop anchor");
    await user.click(screen.getByRole("button", { name: "Preview" }));
    expect(await screen.findByText("provisional 35%")).toBeInTheDocument();
  });

  it("explains every skipped cue", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByLabelText("Remove Drop anchor");
    await user.click(screen.getByRole("button", { name: "Preview" }));

    expect(await screen.findByText(/no breakdown found/)).toBeInTheDocument();
    expect(
      screen.getByText(/Rekordbox rejects two memory cues at the same position/),
    ).toBeInTheDocument();
  });

  it("stages the generated cues", async () => {
    const user = userEvent.setup();
    const { onChanged } = renderPanel();
    await screen.findByLabelText("Remove Drop anchor");
    await user.click(screen.getByRole("button", { name: "Stage cues" }));

    await waitFor(() => {
      expect(applyGeneratedCues).toHaveBeenCalledWith(
        "/lib.db",
        "t1",
        expect.objectContaining({ name: "Default" }),
        [{ anchor: { kind: "drop", ordinal: 1 }, name: "Drop", color: null }],
      );
    });
    expect(await screen.findByText("Staged 2 cue(s) for review.")).toBeInTheDocument();
    expect(onChanged).toHaveBeenCalled();
  });

  it("reports when nothing was staged", async () => {
    const user = userEvent.setup();
    vi.mocked(applyGeneratedCues).mockResolvedValue([]);
    renderPanel();
    await screen.findByLabelText("Remove Drop anchor");
    await user.click(screen.getByRole("button", { name: "Stage cues" }));
    expect(
      await screen.findByText("Nothing to stage — no anchors resolved."),
    ).toBeInTheDocument();
  });

  it("lets an anchor be removed", async () => {
    const user = userEvent.setup();
    renderPanel();
    await screen.findByLabelText("Remove Drop anchor");
    await user.click(screen.getByLabelText("Remove Drop anchor"));
    expect(await screen.findByTestId("no-anchors")).toBeInTheDocument();
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(previewGeneratedCues).mockRejectedValue(new Error("no beat grid"));
    renderPanel();
    await screen.findByLabelText("Remove Drop anchor");
    await user.click(screen.getByRole("button", { name: "Preview" }));
    expect(await screen.findByText(/no beat grid/)).toBeInTheDocument();
  });
});
