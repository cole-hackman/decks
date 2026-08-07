import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UnusedFilesPanel } from "./UnusedFilesPanel";
import { deleteUnusedFiles, scanUnusedFiles } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { UnusedScan } from "../types";

vi.mock("../ipc", () => ({
  scanUnusedFiles: vi.fn(),
  deleteUnusedFiles: vi.fn(),
}));

const SCAN: UnusedScan = {
  files: [
    { path: "/Music/cover.png", size_bytes: 2048 },
    { path: "/Music/notes.txt", size_bytes: 10 },
  ],
  total_bytes: 2058,
  skipped_directories: ["PioneerDJ", "_Serato_"],
  errors: [],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(scanUnusedFiles).mockResolvedValue(SCAN);
  vi.mocked(deleteUnusedFiles).mockResolvedValue({
    deleted: ["/Music/cover.png"],
    failed: [],
    report_path: "/data/reports/deleted-1.txt",
  });
});

function renderPanel() {
  render(
    <WithProviders>
      <UnusedFilesPanel libraryPath="/lib.db" />
    </WithProviders>,
  );
}

async function scanFor(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText(/Folder to scan/i), "/Music");
  await user.click(screen.getByRole("button", { name: "Scan" }));
  return screen.findByTestId("unused-scan");
}

describe("UnusedFilesPanel", () => {
  it("will not scan without a folder", () => {
    renderPanel();
    expect(screen.getByRole("button", { name: "Scan" })).toBeDisabled();
  });

  it("normalises the extension list before sending it", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.type(screen.getByLabelText(/Folder to scan/i), "/Music");
    await user.selectOptions(screen.getByLabelText("Extension mode"), "include");
    await user.type(screen.getByLabelText("Extension list"), "PNG, .JPG");
    await user.click(screen.getByRole("button", { name: "Scan" }));
    await waitFor(() => {
      expect(scanUnusedFiles).toHaveBeenCalledWith("/lib.db", ["/Music"], {
        mode: "include",
        extensions: ["png", "jpg"],
      });
    });
  });

  it("reports what the scan skipped rather than implying it was exhaustive", async () => {
    const user = userEvent.setup();
    renderPanel();
    await scanFor(user);
    expect(screen.getByText(/Skipped: PioneerDJ, _Serato_/)).toBeInTheDocument();
  });

  it("selects nothing by default", async () => {
    const user = userEvent.setup();
    renderPanel();
    await scanFor(user);
    expect(
      screen.getByRole("button", { name: /Delete 0 file\(s\)/ }),
    ).toBeDisabled();
    expect(screen.getByLabelText("Select /Music/cover.png")).not.toBeChecked();
  });

  it("requires an explicit confirmation before deleting", async () => {
    const user = userEvent.setup();
    renderPanel();
    await scanFor(user);
    await user.click(screen.getByLabelText("Select /Music/cover.png"));
    await user.click(screen.getByRole("button", { name: /Delete 1 file\(s\)…/ }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      /cannot be undone/,
    );
    expect(deleteUnusedFiles).not.toHaveBeenCalled();

    await user.click(
      screen.getByRole("button", { name: /Permanently delete 1 file\(s\)/ }),
    );
    await waitFor(() => {
      expect(deleteUnusedFiles).toHaveBeenCalledWith("/lib.db", [
        "/Music/cover.png",
      ]);
    });
  });

  it("cancelling the confirmation deletes nothing", async () => {
    const user = userEvent.setup();
    renderPanel();
    await scanFor(user);
    await user.click(screen.getByLabelText("Select /Music/cover.png"));
    await user.click(screen.getByRole("button", { name: /Delete 1 file\(s\)…/ }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(deleteUnusedFiles).not.toHaveBeenCalled();
  });

  it("says where the record of the deletion went", async () => {
    const user = userEvent.setup();
    renderPanel();
    await scanFor(user);
    await user.click(screen.getByRole("button", { name: "Select all" }));
    await user.click(screen.getByRole("button", { name: /Delete 2 file\(s\)…/ }));
    await user.click(
      screen.getByRole("button", { name: /Permanently delete 2 file\(s\)/ }),
    );
    expect(
      await screen.findByText(/deleted-1\.txt/),
    ).toBeInTheDocument();
  });

  it("reports files that could not be deleted", async () => {
    const user = userEvent.setup();
    vi.mocked(deleteUnusedFiles).mockResolvedValue({
      deleted: [],
      failed: [["/Music/cover.png", "now referenced by the library"]],
      report_path: null,
    });
    renderPanel();
    await scanFor(user);
    await user.click(screen.getByLabelText("Select /Music/cover.png"));
    await user.click(screen.getByRole("button", { name: /Delete 1 file\(s\)…/ }));
    await user.click(
      screen.getByRole("button", { name: /Permanently delete 1 file\(s\)/ }),
    );
    expect(
      await screen.findByText(/now referenced by the library/),
    ).toBeInTheDocument();
  });

  it("surfaces a scan error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(scanUnusedFiles).mockRejectedValue(
      new Error("refusing to scan with an empty library"),
    );
    renderPanel();
    await user.type(screen.getByLabelText(/Folder to scan/i), "/Music");
    await user.click(screen.getByRole("button", { name: "Scan" }));
    expect(await screen.findByText(/empty library/)).toBeInTheDocument();
  });
});
