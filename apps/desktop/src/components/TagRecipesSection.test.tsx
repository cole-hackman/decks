import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TagRecipesSection } from "./TagRecipesSection";
import { tagRecipeApply, tagRecipePreview } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { TagProposal } from "../types";

vi.mock("../ipc", () => ({
  tagRecipePreview: vi.fn(),
  tagRecipeApply: vi.fn(),
}));

const PROPOSALS: TagProposal[] = [
  {
    track_id: "t1",
    track_title: "Get Lucky",
    added: ["Techno", "Vocals"],
    removed: [],
  },
  { track_id: "t2", track_title: "One More Time", added: [], removed: ["Old"] },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(tagRecipePreview).mockResolvedValue(PROPOSALS);
  vi.mocked(tagRecipeApply).mockResolvedValue({
    tracks_changed: 2,
    tags_added: 2,
    tags_removed: 1,
    tags_created: ["Techno"],
  });
});

function renderSection(trackIds = ["t1", "t2"]) {
  render(
    <WithProviders>
      <TagRecipesSection
        libraryPath="/lib.db"
        trackIds={trackIds}
        fields={["title", "comment"]}
      />
    </WithProviders>,
  );
}

describe("TagRecipesSection", () => {
  it("defaults to importing from the comment field with a # marker", async () => {
    const user = userEvent.setup();
    renderSection();
    expect(screen.getByLabelText("Import source field")).toHaveValue("comment");
    expect(screen.getByLabelText("Tag marker")).toHaveValue("#");

    await user.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => {
      expect(tagRecipePreview).toHaveBeenCalledWith("/lib.db", ["t1", "t2"], {
        op: "import_from_text",
        field: "comment",
        separator: "#",
      });
    });
  });

  it("shows added and removed tags per track", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    expect(await screen.findByTestId("tag-recipe-preview")).toBeInTheDocument();
    expect(screen.getByText("+Techno")).toBeInTheDocument();
    expect(screen.getByText("−Old")).toBeInTheDocument();
  });

  it("splits a comma-separated tag list into a real list", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.selectOptions(screen.getByLabelText("Tag operation"), "add_tags");
    await user.type(screen.getByLabelText("Tags"), "Techno, , Vocals");
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => {
      expect(tagRecipePreview).toHaveBeenCalledWith("/lib.db", ["t1", "t2"], {
        op: "add_tags",
        tags: ["Techno", "Vocals"],
      });
    });
  });

  it("clear takes no parameters", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.selectOptions(screen.getByLabelText("Tag operation"), "clear_tags");
    expect(screen.queryByLabelText("Tags")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => {
      expect(tagRecipePreview).toHaveBeenCalledWith("/lib.db", ["t1", "t2"], {
        op: "clear_tags",
      });
    });
  });

  it("changing the operation clears a stale preview", async () => {
    // Otherwise the Apply button would act on results from a different recipe.
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await screen.findByTestId("tag-recipe-preview");
    await user.selectOptions(screen.getByLabelText("Tag operation"), "clear_tags");
    expect(screen.queryByTestId("tag-recipe-preview")).toBeNull();
  });

  it("applying reports what was created, since importing may invent tags", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await user.click(await screen.findByRole("button", { name: /Apply to 2 track/ }));
    expect(await screen.findByText(/created Techno/)).toBeInTheDocument();
  });

  it("says so when a recipe would change nothing", async () => {
    const user = userEvent.setup();
    vi.mocked(tagRecipePreview).mockResolvedValue([]);
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    expect(await screen.findByText(/No tag changes/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Apply to 0 track/ })).toBeDisabled();
  });

  it("surfaces a backend refusal instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(tagRecipeApply).mockRejectedValue(
      new Error("create a tag category before importing tags"),
    );
    renderSection();
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await user.click(await screen.findByRole("button", { name: /Apply to 2 track/ }));
    expect(
      await screen.findByText(/create a tag category before importing tags/),
    ).toBeInTheDocument();
  });
});
