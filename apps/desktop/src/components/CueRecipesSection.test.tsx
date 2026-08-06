import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CueRecipesSection } from "./CueRecipesSection";
import { cueRecipeApply, cueRecipePreview } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { CueRecipeTrack } from "../types";

vi.mock("../ipc", () => ({
  cueRecipePreview: vi.fn(),
  cueRecipeApply: vi.fn(),
}));

const TRACK: CueRecipeTrack = {
  track_id: "t1",
  track_title: "get lucky",
  edits: [
    {
      cue_id: "c1",
      cue_label: "1:05 Drop",
      field: "InMsec",
      before: 65000,
      after: 65500,
    },
  ],
  deletions: [],
  skipped: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(cueRecipePreview).mockResolvedValue([TRACK]);
  vi.mocked(cueRecipeApply).mockResolvedValue(["s1"]);
});

function renderSection(trackIds = ["t1", "t2"]) {
  render(
    <WithProviders>
      <CueRecipesSection libraryPath="/lib.db" trackIds={trackIds} />
    </WithProviders>,
  );
}

describe("CueRecipesSection", () => {
  it("previews the chosen operation over the selection", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    await waitFor(() => {
      expect(cueRecipePreview).toHaveBeenCalledWith("/lib.db", ["t1", "t2"], {
        op: "delete_cues",
        mode: "without_text",
      });
    });
  });

  it("shows only the parameters the chosen operation takes", async () => {
    const user = userEvent.setup();
    renderSection();
    expect(screen.getByLabelText("Which cues")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Cue operation"), "shift_cues");
    expect(screen.queryByLabelText("Which cues")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Shift offset")).toBeInTheDocument();
  });

  it("sends the offset as a number, not the text the input holds", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.selectOptions(screen.getByLabelText("Cue operation"), "shift_cues");
    await user.clear(screen.getByLabelText("Shift offset"));
    await user.type(screen.getByLabelText("Shift offset"), "250");
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    await waitFor(() => {
      expect(cueRecipePreview).toHaveBeenCalledWith("/lib.db", ["t1", "t2"], {
        op: "shift_cues",
        offset_ms: 250,
      });
    });
  });

  it("summarises edits and deletions per track", async () => {
    const user = userEvent.setup();
    vi.mocked(cueRecipePreview).mockResolvedValue([
      {
        ...TRACK,
        deletions: [{ cue_id: "c2", cue_label: "2:00" }],
      },
    ]);
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    const list = await screen.findByTestId("cue-recipe-preview");
    expect(list).toHaveTextContent("1 edit(s)");
    expect(list).toHaveTextContent("−1 cue(s)");
  });

  it("shows why a track was skipped rather than dropping it", async () => {
    const user = userEvent.setup();
    vi.mocked(cueRecipePreview).mockResolvedValue([
      {
        ...TRACK,
        edits: [],
        skipped: "this track has no beat grid",
      },
    ]);
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    expect(
      await screen.findByText("this track has no beat grid"),
    ).toBeInTheDocument();
  });

  it("does not offer to stage a track that only carries a skip reason", async () => {
    const user = userEvent.setup();
    vi.mocked(cueRecipePreview).mockResolvedValue([
      { ...TRACK, edits: [], skipped: "this track has no beat grid" },
    ]);
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    expect(
      await screen.findByRole("button", { name: /Stage 0 track/ }),
    ).toBeDisabled();
  });

  it("stages exactly what the preview showed", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    await user.click(await screen.findByRole("button", { name: /Stage 1 track/ }));
    await waitFor(() => {
      expect(cueRecipeApply).toHaveBeenCalledWith("/lib.db", [TRACK]);
    });
  });

  it("clears the preview once it has been staged", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    await user.click(await screen.findByRole("button", { name: /Stage 1 track/ }));
    await waitFor(() => {
      expect(screen.queryByTestId("cue-recipe-preview")).not.toBeInTheDocument();
    });
  });

  it("drops a stale preview when the operation changes", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    await screen.findByTestId("cue-recipe-preview");
    await user.selectOptions(screen.getByLabelText("Cue operation"), "sort_cues");
    expect(screen.queryByTestId("cue-recipe-preview")).not.toBeInTheDocument();
  });

  it("is unavailable with nothing selected", () => {
    renderSection([]);
    expect(screen.getByRole("button", { name: "Preview cues" })).toBeDisabled();
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(cueRecipePreview).mockRejectedValue(new Error("library locked"));
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview cues" }));
    expect(await screen.findByText(/library locked/)).toBeInTheDocument();
  });
});
