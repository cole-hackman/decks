import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BackupSection } from "./BackupSection";
import { createBackup, pickAndInspectBackup, restoreBackup } from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({
  createBackup: vi.fn(),
  pickAndInspectBackup: vi.fn(),
  restoreBackup: vi.fn(),
}));

const SUMMARY = {
  path: "/home/me/decks-backup.json",
  rows: 42,
  tables: [
    ["tags", 12],
    ["archived_tracks", 30],
  ] as [string, number][],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(createBackup).mockResolvedValue(SUMMARY);
  vi.mocked(pickAndInspectBackup).mockResolvedValue(SUMMARY);
  vi.mocked(restoreBackup).mockResolvedValue({
    restored: [
      ["tags", 12],
      ["archived_tracks", 30],
    ],
    unknown_tables: [],
    dropped_columns: [],
  });
});

function renderSection() {
  render(
    <WithProviders>
      <BackupSection />
    </WithProviders>,
  );
}

describe("BackupSection", () => {
  it("says what a backup does and does not cover", () => {
    renderSection();
    // The two things a user needs to know before trusting it.
    expect(screen.getByLabelText("Database backup")).toHaveTextContent(
      /Not your music files/,
    );
    expect(screen.getByLabelText("Database backup")).toHaveTextContent(
      /Restoring.*replaces/i,
    );
  });

  it("says backups are never auto-deleted, unlike Lexicon's", () => {
    renderSection();
    expect(screen.getByTestId("backup-retention-note")).toHaveTextContent(
      /never deleted automatically/,
    );
  });

  it("reports how much a backup holds", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Create backup…" }));
    expect(await screen.findByText(/Backed up 42 row\(s\)/)).toBeInTheDocument();
  });

  it("a cancelled save dialog is not an error", async () => {
    const user = userEvent.setup();
    vi.mocked(createBackup).mockResolvedValue(null);
    renderSection();
    await user.click(screen.getByRole("button", { name: "Create backup…" }));
    await waitFor(() => expect(createBackup).toHaveBeenCalled());
    expect(screen.queryByText(/Backed up/)).not.toBeInTheDocument();
  });

  it("shows what the backup holds before asking to replace", async () => {
    // The user should know what they are swapping *in*, not just that they are
    // about to lose something.
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Restore from backup…" }));
    const body = await screen.findByText(/Tags: 12/);
    expect(body).toHaveTextContent(/Archived tracks: 30/);
    expect(body).toHaveTextContent(/cannot be undone/);
  });

  it("cancelling the confirm restores nothing", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Restore from backup…" }));
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(restoreBackup).not.toHaveBeenCalled();
  });

  it("restores after the confirm and reports the row count", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(screen.getByRole("button", { name: "Restore from backup…" }));
    await user.click(await screen.findByRole("button", { name: "Replace" }));
    await waitFor(() => {
      expect(restoreBackup).toHaveBeenCalledWith("/home/me/decks-backup.json");
    });
    expect(await screen.findByText(/Restored 42 row\(s\)/)).toBeInTheDocument();
  });

  it("a file that is not a backup is rejected before anything is deleted", async () => {
    const user = userEvent.setup();
    vi.mocked(pickAndInspectBackup).mockRejectedValue(
      new Error("/tmp/notes.json is not a decks backup"),
    );
    renderSection();
    await user.click(screen.getByRole("button", { name: "Restore from backup…" }));
    expect(await screen.findByText(/is not a decks backup/)).toBeInTheDocument();
    expect(restoreBackup).not.toHaveBeenCalled();
  });

  it("reports what a restore could not carry", async () => {
    const user = userEvent.setup();
    vi.mocked(restoreBackup).mockResolvedValue({
      restored: [["tags", 12]],
      unknown_tables: ["future_table"],
      dropped_columns: [["tags", "future_column"]],
    });
    renderSection();
    await user.click(screen.getByRole("button", { name: "Restore from backup…" }));
    await user.click(await screen.findByRole("button", { name: "Replace" }));
    expect(
      await screen.findByText(/1 unknown table\(s\) skipped/),
    ).toBeInTheDocument();
  });
});
