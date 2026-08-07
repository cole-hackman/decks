import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OtherRecipesSection } from "./OtherRecipesSection";
import { otherRecipeApply } from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({ otherRecipeApply: vi.fn() }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(otherRecipeApply).mockResolvedValue({
    changed: ["t1", "t2"],
    staged: [],
    skipped: [],
  });
});

function renderSection(trackIds = ["t1", "t2"]) {
  render(
    <WithProviders>
      <OtherRecipesSection libraryPath="/lib.db" trackIds={trackIds} />
    </WithProviders>,
  );
}

describe("OtherRecipesSection", () => {
  it("runs the chosen recipe over the selection", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: /Run on 2 track/ }));
    await waitFor(() => {
      expect(otherRecipeApply).toHaveBeenCalledWith(
        "/lib.db",
        ["t1", "t2"],
        "mark_as_incoming",
      );
    });
  });

  it("explains what each recipe does before it is run", async () => {
    const user = userEvent.setup();
    renderSection();
    expect(screen.getByTestId("other-recipe-detail")).toHaveTextContent(/to-do list/);

    await user.selectOptions(
      screen.getByLabelText("Other recipe"),
      "remove_from_all_playlists",
    );
    // The smartlist caveat matters — it is why the recipe looks like it missed some.
    expect(screen.getByTestId("other-recipe-detail")).toHaveTextContent(
      /Smartlists are untouched/,
    );
  });

  it("says why modification time is used rather than creation time", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.selectOptions(
      screen.getByLabelText("Other recipe"),
      "import_date_from_filesystem",
    );
    expect(screen.getByTestId("other-recipe-detail")).toHaveTextContent(
      /not portable/,
    );
  });

  it("reports staged changes separately from tracks touched", async () => {
    const user = userEvent.setup();
    vi.mocked(otherRecipeApply).mockResolvedValue({
      changed: ["t1"],
      staged: ["c1", "c2", "c3"],
      skipped: [],
    });
    renderSection();
    await user.click(screen.getByRole("button", { name: /Run on 2 track/ }));
    expect(
      await screen.findByText(/3 change\(s\) staged for review/),
    ).toBeInTheDocument();
  });

  it("surfaces skipped tracks with their reason", async () => {
    const user = userEvent.setup();
    vi.mocked(otherRecipeApply).mockResolvedValue({
      changed: [],
      staged: [],
      skipped: [["t1", "track has no file path"]],
    });
    renderSection();
    await user.click(screen.getByRole("button", { name: /Run on 2 track/ }));
    expect(await screen.findByText(/track has no file path/)).toBeInTheDocument();
  });

  it("is unavailable with nothing selected", () => {
    renderSection([]);
    expect(screen.getByRole("button", { name: /Run on 0 track/ })).toBeDisabled();
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(otherRecipeApply).mockRejectedValue(new Error("library locked"));
    renderSection();
    await user.click(screen.getByRole("button", { name: /Run on 2 track/ }));
    expect(await screen.findByText(/library locked/)).toBeInTheDocument();
  });
});
