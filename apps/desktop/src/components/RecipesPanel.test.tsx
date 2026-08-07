import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RecipesPanel } from "./RecipesPanel";
import { recipeApply, recipeFields, recipePreview } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { RecipePreview } from "../types";

vi.mock("../ipc", () => ({
  recipeFields: vi.fn(),
  recipePreview: vi.fn(),
  recipeApply: vi.fn(),
  // Used by the tag section mounted below.
  tagRecipePreview: vi.fn(async () => []),
  tagRecipeApply: vi.fn(),
  otherRecipeApply: vi.fn(),
  // Used by the cue section mounted below.
  cueRecipePreview: vi.fn(async () => []),
  cueRecipeApply: vi.fn(),
  // Used by the CSV import section mounted below.
  csvImportFields: vi.fn(async () => ["title", "artist"]),
  csvImportHeaders: vi.fn(async () => []),
  csvImportPreview: vi.fn(),
  csvImportApply: vi.fn(),
}));

const PREVIEW: RecipePreview = {
  proposals: [
    {
      id: "t1:title",
      track_id: "t1",
      track_title: "get lucky",
      field: "title",
      before: "get lucky",
      after: "Get Lucky",
    },
    {
      id: "t2:title",
      track_id: "t2",
      track_title: "one more time",
      field: "title",
      before: "one more time",
      after: "One More Time",
    },
  ],
  skipped: [["t3", "remixer is empty"]],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(recipeFields).mockResolvedValue(["title", "artist", "genre"]);
  vi.mocked(recipePreview).mockResolvedValue(PREVIEW);
  vi.mocked(recipeApply).mockResolvedValue(["c1", "c2"]);
});

/** The field-recipe Preview button — the tag section has one of its own. */
function fieldPreviewButton() {
  return screen.getAllByRole("button", { name: "Preview" })[0];
}

/**
 * Wait for the field list to arrive.
 *
 * Waits on the *state* the fetch produces rather than on the fetch itself: the
 * selects render one pass before they have any options.
 */
async function fieldsLoaded() {
  await waitFor(() => {
    expect(screen.getByLabelText("Field")).toHaveValue("title");
  });
}

function renderPanel(trackIds = ["t1", "t2", "t3"]) {
  render(
    <WithProviders>
      <RecipesPanel libraryPath="/lib.db" trackIds={trackIds} />
    </WithProviders>,
  );
}

describe("RecipesPanel", () => {
  it("starts with no steps and says they run in order", async () => {
    renderPanel();
    expect(await screen.findByTestId("no-recipes")).toHaveTextContent(
      /run in the order listed/,
    );
  });

  it("cannot preview until a recipe has been added", async () => {
    renderPanel();
    await screen.findByTestId("no-recipes");
    expect(fieldPreviewButton()).toBeDisabled();
  });

  it("offers only fields the backend says are writable", async () => {
    renderPanel();
    await fieldsLoaded();
    const select = screen.getByLabelText("Field");
    expect(within(select).getAllByRole("option")).toHaveLength(3);
    expect(
      within(select).getByRole("option", { name: "genre" }),
    ).toBeInTheDocument();
  });

  it("adds a step and sends it to the preview", async () => {
    const user = userEvent.setup();
    renderPanel();
    await fieldsLoaded();
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(fieldPreviewButton());

    await waitFor(() => {
      expect(recipePreview).toHaveBeenCalledWith(
        "/lib.db",
        ["t1", "t2", "t3"],
        [{ op: "to_title_case", field: "title", ignore_words: [] }],
      );
    });
  });

  it("shows each proposed change as a before/after row", async () => {
    const user = userEvent.setup();
    renderPanel();
    await fieldsLoaded();
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(fieldPreviewButton());

    expect(await screen.findByTestId("recipe-preview")).toBeInTheDocument();
    expect(screen.getByText("Get Lucky")).toBeInTheDocument();
    expect(screen.getByText("One More Time")).toBeInTheDocument();
  });

  it("deselecting a row excludes it from what gets staged", async () => {
    const user = userEvent.setup();
    renderPanel();
    await fieldsLoaded();
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(fieldPreviewButton());
    await screen.findByTestId("recipe-preview");

    await user.click(screen.getByLabelText("Keep get lucky title"));
    await user.click(screen.getByRole("button", { name: /Stage 1 change/ }));

    await waitFor(() => {
      expect(recipeApply).toHaveBeenCalledWith("/lib.db", [PREVIEW.proposals[1]]);
    });
  });

  it("explains steps that did nothing rather than leaving a silent gap", async () => {
    const user = userEvent.setup();
    renderPanel();
    await fieldsLoaded();
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(fieldPreviewButton());
    expect(await screen.findByTestId("recipe-skipped")).toHaveTextContent(
      /remixer is empty/,
    );
  });

  it("says so when a preview would change nothing", async () => {
    const user = userEvent.setup();
    vi.mocked(recipePreview).mockResolvedValue({ proposals: [], skipped: [] });
    renderPanel();
    await fieldsLoaded();
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(fieldPreviewButton());
    expect(await screen.findByText(/Nothing would change/)).toBeInTheDocument();
  });

  it("a step can be removed from the list", async () => {
    const user = userEvent.setup();
    renderPanel();
    await fieldsLoaded();
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(await screen.findByLabelText("Remove step 1"));
    expect(await screen.findByTestId("no-recipes")).toBeInTheDocument();
  });

  it("changing the operation resets its parameters", async () => {
    // Carrying a delimiter value into an operation with no delimiter would
    // silently ship junk to the backend.
    const user = userEvent.setup();
    renderPanel();
    await fieldsLoaded();
    await user.selectOptions(screen.getByLabelText("Operation"), "shorten_text");
    expect(screen.getByLabelText("Characters per word")).toHaveValue(2);
    await user.selectOptions(screen.getByLabelText("Operation"), "adjust_number");
    expect(
      screen.getByLabelText("Amount (negative to decrease)"),
    ).toHaveValue(1);
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(recipePreview).mockRejectedValue(new Error("library locked"));
    renderPanel();
    await fieldsLoaded();
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(fieldPreviewButton());
    expect(await screen.findByText(/library locked/)).toBeInTheDocument();
  });
});
