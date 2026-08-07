import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WriteTagsPanel } from "./WriteTagsPanel";
import { writeTagsBulk } from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({ writeTagsBulk: vi.fn() }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(writeTagsBulk).mockResolvedValue({
    written: ["t1", "t2"],
    failed: [],
    skipped: [],
  });
});

function renderPanel(trackIds = ["t1", "t2"]) {
  render(
    <WithProviders>
      <WriteTagsPanel libraryPath="/lib.db" trackIds={trackIds} />
    </WithProviders>,
  );
}

describe("WriteTagsPanel", () => {
  it("selects no fields by default, so writing is unavailable", () => {
    renderPanel();
    expect(
      screen.getByRole("button", { name: /Write tags to 2 file\(s\)/ }),
    ).toBeDisabled();
  });

  it("writes only the ticked fields", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(screen.getByLabelText("Title"));
    await user.click(screen.getByLabelText("BPM"));
    await user.click(
      screen.getByRole("button", { name: /Write tags to 2 file\(s\)/ }),
    );

    await waitFor(() => {
      expect(writeTagsBulk).toHaveBeenCalledWith("/lib.db", ["t1", "t2"], {
        title: true,
        artist: false,
        album: false,
        genre: false,
        bpm: true,
        musical_key: false,
        comment: false,
        year: false,
      });
    });
  });

  it("does nothing when there are no tracks to write to", async () => {
    const user = userEvent.setup();
    renderPanel([]);
    await user.click(screen.getByLabelText("Title"));
    expect(
      screen.getByRole("button", { name: /Write tags to 0 file\(s\)/ }),
    ).toBeDisabled();
  });

  it("clearing the selection disables writing again", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(screen.getByLabelText("Genre"));
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(
      screen.getByRole("button", { name: /Write tags to 2 file\(s\)/ }),
    ).toBeDisabled();
  });

  it("reports how many files had nothing to write", async () => {
    const user = userEvent.setup();
    vi.mocked(writeTagsBulk).mockResolvedValue({
      written: ["t1"],
      failed: [],
      skipped: ["t2"],
    });
    renderPanel();
    await user.click(screen.getByLabelText("Genre"));
    await user.click(
      screen.getByRole("button", { name: /Write tags to 2 file\(s\)/ }),
    );
    expect(
      await screen.findByText(/1 had nothing to write/),
    ).toBeInTheDocument();
  });

  it("reports failures rather than claiming success", async () => {
    const user = userEvent.setup();
    vi.mocked(writeTagsBulk).mockResolvedValue({
      written: [],
      failed: [["t1", "read-only file system"]],
      skipped: [],
    });
    renderPanel();
    await user.click(screen.getByLabelText("Title"));
    await user.click(
      screen.getByRole("button", { name: /Write tags to 2 file\(s\)/ }),
    );
    expect(
      await screen.findByText(/read-only file system/),
    ).toBeInTheDocument();
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(writeTagsBulk).mockRejectedValue(new Error("library locked"));
    renderPanel();
    await user.click(screen.getByLabelText("Title"));
    await user.click(
      screen.getByRole("button", { name: /Write tags to 2 file\(s\)/ }),
    );
    expect(await screen.findByText(/library locked/)).toBeInTheDocument();
  });
});
