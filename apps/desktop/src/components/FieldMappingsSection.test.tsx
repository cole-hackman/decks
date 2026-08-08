import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FieldMappingsSection } from "./FieldMappingsSection";
import {
  createFieldMapping,
  deleteFieldMapping,
  listFieldMappings,
  listTagCategories,
  mappableTagTargets,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { FieldMappingRow } from "../types";

vi.mock("../ipc", () => ({
  listFieldMappings: vi.fn(),
  createFieldMapping: vi.fn(),
  deleteFieldMapping: vi.fn(),
  mappableTagTargets: vi.fn(),
  listTagCategories: vi.fn(),
}));

const ROWS: FieldMappingRow[] = [
  { id: "r1", source: { kind: "energy" }, target: "Comment", overwrite: true },
  {
    id: "r2",
    source: { kind: "all_custom_tags" },
    target: "Comment",
    overwrite: false,
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listFieldMappings).mockResolvedValue(ROWS);
  vi.mocked(mappableTagTargets).mockResolvedValue(["Comment", "Genre"]);
  vi.mocked(createFieldMapping).mockResolvedValue("r3");
  vi.mocked(listTagCategories).mockResolvedValue([
    { id: "genre", name: "Genre", seq: 0, color: null },
    { id: "mood", name: "Mood", seq: 1, color: null },
  ]);
  vi.mocked(deleteFieldMapping).mockResolvedValue(true);
});

function renderSection() {
  render(
    <WithProviders>
      <FieldMappingsSection />
    </WithProviders>,
  );
}

describe("FieldMappingsSection", () => {
  it("says what no mappings means, not just that there are none", async () => {
    vi.mocked(listFieldMappings).mockResolvedValue([]);
    renderSection();
    expect(await screen.findByTestId("no-field-mappings")).toHaveTextContent(
      /not written to files/,
    );
  });

  it("lists mappings with whether they replace or append", async () => {
    renderSection();
    // Wait on something only the rule list renders. "Energy" is *also* a
    // static <option> in the source picker, so awaiting that resolves on the
    // first render and never waits for `listFieldMappings` at all — which is
    // green locally and red on a slower runner.
    expect(await screen.findByText("replaces")).toBeInTheDocument();
    expect(screen.getByText("appends")).toBeInTheDocument();
  });

  it("only offers targets audio files actually have", async () => {
    renderSection();
    // Wait for an option to exist, not for the call: the targets arrive in an
    // effect, so the select is on screen one render before it has any options.
    const comment = await screen.findByRole("option", { name: "Comment" });
    const select = screen.getByLabelText("Mapping target");
    expect(select).toHaveValue("Comment");
    expect(comment).toBeInTheDocument();
    expect(select.querySelectorAll("option")).toHaveLength(2);
  });

  it("adds a mapping", async () => {
    const user = userEvent.setup();
    renderSection();
    // The target options and the rule list arrive from two different promises;
    // wait for the one this test drives.
    await screen.findByRole("option", { name: "Genre" });
    await user.selectOptions(screen.getByLabelText("Mapping source"), "all_custom_tags");
    await user.selectOptions(screen.getByLabelText("Mapping target"), "Genre");
    await user.click(screen.getByLabelText("Replace existing value"));
    await user.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect(createFieldMapping).toHaveBeenCalledWith(
        { kind: "all_custom_tags" },
        "Genre",
        true,
      );
    });
  });

  it("removes a mapping", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(await screen.findByLabelText("Remove mapping Energy"));
    await waitFor(() => {
      expect(deleteFieldMapping).toHaveBeenCalledWith("r1");
    });
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(createFieldMapping).mockRejectedValue(
      new Error("a target field is required"),
    );
    renderSection();
    await screen.findByRole("option", { name: "Comment" });
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(
      await screen.findByText(/a target field is required/),
    ).toBeInTheDocument();
  });

  it("offers each tag category as a source in its own right", async () => {
    // Per the spec, "a single category can be the source instead" of all tags.
    // The engine has always supported it; it was never offered.
    renderSection();
    const select = await screen.findByLabelText("Mapping source");
    await waitFor(() =>
      expect(
        screen.getByRole("option", { name: "Tag category: Genre" }),
      ).toBeInTheDocument(),
    );

    await userEvent.selectOptions(select, "category:mood");
    await userEvent.click(screen.getByRole("button", { name: /^Add/ }));

    // Stored by *name*, not id: a renamed category then stops matching rather
    // than silently exporting a different set under the old label.
    expect(vi.mocked(createFieldMapping)).toHaveBeenCalledWith(
      { kind: "tag_category", name: "Mood" },
      "Comment",
      false,
    );
  });

  it("still works when the tag tree cannot be read", async () => {
    // No categories just means no per-category sources, not a broken panel.
    vi.mocked(listTagCategories).mockRejectedValue(new Error("no cache"));
    renderSection();
    expect(await screen.findByLabelText("Mapping source")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Energy" })).toBeInTheDocument();
  });
});
