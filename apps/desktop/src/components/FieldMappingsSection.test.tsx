import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FieldMappingsSection } from "./FieldMappingsSection";
import {
  createFieldMapping,
  deleteFieldMapping,
  listFieldMappings,
  mappableTagTargets,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { FieldMappingRow } from "../types";

vi.mock("../ipc", () => ({
  listFieldMappings: vi.fn(),
  createFieldMapping: vi.fn(),
  deleteFieldMapping: vi.fn(),
  mappableTagTargets: vi.fn(),
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
    expect(await screen.findByText("Energy")).toBeInTheDocument();
    expect(screen.getByText("replaces")).toBeInTheDocument();
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
});
