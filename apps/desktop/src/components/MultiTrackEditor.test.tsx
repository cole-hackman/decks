import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MultiTrackEditor } from "./MultiTrackEditor";
import { multiEditApply, multiEditForm } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { MultiEditFormData } from "../types";

vi.mock("../ipc", () => ({
  multiEditForm: vi.fn(),
  multiEditApply: vi.fn(),
}));

const FORM: MultiEditFormData = {
  track_count: 2,
  fields: [
    // Both tracks agree.
    ["artist", { kind: "same", value: "Daft Punk" }],
    // They disagree — shows `<multiple values>` as a placeholder.
    ["genre", { kind: "multiple" }],
    // Both agree it is empty.
    ["album", { kind: "same", value: null }],
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(multiEditForm).mockResolvedValue(FORM);
  vi.mocked(multiEditApply).mockResolvedValue(["c1", "c2"]);
});

function renderEditor(onClose = vi.fn(), trackIds = ["t1", "t2"]) {
  render(
    <WithProviders>
      <MultiTrackEditor
        libraryPath="/lib.db"
        trackIds={trackIds}
        onClose={onClose}
      />
    </WithProviders>,
  );
  return onClose;
}

describe("MultiTrackEditor", () => {
  it("says how many tracks it covers", async () => {
    renderEditor();
    expect(await screen.findByText("Edit 2 tracks")).toBeInTheDocument();
  });

  it("shows the shared value where the selection agrees", async () => {
    renderEditor();
    expect(await screen.findByLabelText("artist")).toHaveValue("Daft Punk");
  });

  it("shows <multiple values> as a placeholder, not as text", async () => {
    // A placeholder carries no value, so there is nothing to save by accident.
    renderEditor();
    const genre = await screen.findByLabelText("genre");
    expect(genre).toHaveValue("");
    expect(genre).toHaveAttribute("placeholder", "<multiple values>");
  });

  it("saving an untouched form writes nothing", async () => {
    // The rule the whole feature turns on. Otherwise opening the editor on a
    // mixed selection and pressing Save flattens every field.
    const user = userEvent.setup();
    const onClose = renderEditor();
    await screen.findByLabelText("artist");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(multiEditApply).not.toHaveBeenCalled();
  });

  it("sends only the field that was touched", async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.type(await screen.findByLabelText("genre"), "Disco");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(multiEditApply).toHaveBeenCalledWith(
        "/lib.db",
        ["t1", "t2"],
        [{ field: "genre", value: "Disco" }],
      );
    });
  });

  it("typing a value back to what it already was is not a change", async () => {
    const user = userEvent.setup();
    renderEditor();
    const artist = await screen.findByLabelText("artist");
    await user.clear(artist);
    await user.type(artist, "Daft Punk");
    expect(screen.getByTestId("multi-edit-count")).toHaveTextContent(
      "0 field(s) changed",
    );
  });

  it("clearing a shared value is a real edit", async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.clear(await screen.findByLabelText("artist"));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(multiEditApply).toHaveBeenCalledWith(
        "/lib.db",
        ["t1", "t2"],
        [{ field: "artist", value: null }],
      );
    });
  });

  it("counts the fields that would change", async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.type(await screen.findByLabelText("genre"), "Disco");
    await user.type(screen.getByLabelText("album"), "Random Access Memories");
    expect(screen.getByTestId("multi-edit-count")).toHaveTextContent(
      "2 field(s) changed",
    );
  });

  it("Escape discards without writing", async () => {
    const user = userEvent.setup();
    const onClose = renderEditor();
    await user.type(await screen.findByLabelText("genre"), "Disco");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(multiEditApply).not.toHaveBeenCalled();
  });

  it("Enter saves", async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.type(await screen.findByLabelText("genre"), "Disco{Enter}");
    await waitFor(() => expect(multiEditApply).toHaveBeenCalled());
  });

  it("Cancel discards without writing", async () => {
    const user = userEvent.setup();
    const onClose = renderEditor();
    await user.type(await screen.findByLabelText("genre"), "Disco");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
    expect(multiEditApply).not.toHaveBeenCalled();
  });

  it("surfaces a load failure instead of showing an empty form", async () => {
    vi.mocked(multiEditForm).mockRejectedValue(new Error("library locked"));
    renderEditor();
    expect(await screen.findByText(/library locked/)).toBeInTheDocument();
  });

  it("surfaces a save failure and stays open", async () => {
    const user = userEvent.setup();
    vi.mocked(multiEditApply).mockRejectedValue(new Error("cache locked"));
    const onClose = renderEditor();
    await user.type(await screen.findByLabelText("genre"), "Disco");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText(/cache locked/)).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
