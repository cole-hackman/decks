import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UndoHistoryPanel } from "./UndoHistoryPanel";
import { listUndoRuns, undoRun, undoRunEntries } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { UndoEntry, UndoRun } from "../types";

vi.mock("../ipc", () => ({
  listUndoRuns: vi.fn(),
  undoRunEntries: vi.fn(),
  undoRun: vi.fn(),
}));

const RUN: UndoRun = {
  id: "r1",
  library_path: "/lib.db",
  applied_at: 1_700_000_000,
  undone_at: null,
  reversible: 3,
  blocked: 1,
};

const ENTRIES: UndoEntry[] = [
  {
    id: "e1",
    source_change_id: "c1",
    kind: "TrackMetadataEdit",
    target_id: "t1",
    field: "Title",
    old_value: "Get Lucky",
    new_value: "get lucky",
    description: 'Title: "Get Lucky" → "get lucky"',
    blocked_reason: null,
  },
  {
    id: "e2",
    source_change_id: "c2",
    kind: null,
    target_id: null,
    field: null,
    old_value: null,
    new_value: null,
    description: "TrackAddCue on t1",
    blocked_reason: "the new row's id is generated when the change is applied",
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listUndoRuns).mockResolvedValue([RUN]);
  vi.mocked(undoRunEntries).mockResolvedValue(ENTRIES);
  vi.mocked(undoRun).mockResolvedValue({
    staged: ["s1", "s2", "s3"],
    blocked: [["TrackAddCue on t1", "generated id"]],
  });
});

function renderPanel(onStaged?: () => void) {
  render(
    <WithProviders>
      <UndoHistoryPanel libraryPath="/lib.db" onStaged={onStaged} />
    </WithProviders>,
  );
}

describe("UndoHistoryPanel", () => {
  it("lists runs with how much of each can be put back", async () => {
    renderPanel();
    // The split is shown before the user commits to anything.
    expect(await screen.findByText("3 reversible, 1 not")).toBeInTheDocument();
  });

  it("says there is nothing to undo when no sync has run", async () => {
    vi.mocked(listUndoRuns).mockResolvedValue([]);
    renderPanel();
    expect(await screen.findByTestId("no-undo-runs")).toBeInTheDocument();
  });

  it("expands a run to show what it did", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByRole("button", { name: /Sync of/ }));
    const list = await screen.findByTestId("undo-entries");
    expect(list).toHaveTextContent('Title: "Get Lucky" → "get lucky"');
  });

  it("shows the reason next to an entry that cannot be reversed", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByRole("button", { name: /Sync of/ }));
    expect(
      await screen.findByText(/id is generated when the change is applied/),
    ).toBeInTheDocument();
  });

  it("collapses again on a second click", async () => {
    const user = userEvent.setup();
    renderPanel();
    const header = await screen.findByRole("button", { name: /Sync of/ });
    await user.click(header);
    await screen.findByTestId("undo-entries");
    await user.click(header);
    await waitFor(() => {
      expect(screen.queryByTestId("undo-entries")).not.toBeInTheDocument();
    });
  });

  it("stages the inverses and reports both halves of the result", async () => {
    const user = userEvent.setup();
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "Undo 3" }));
    await waitFor(() => {
      expect(undoRun).toHaveBeenCalledWith("/lib.db", "r1");
    });
    // The count that could not be reversed is reported, not swallowed.
    expect(
      await screen.findByText(/1 could not be reversed/),
    ).toBeInTheDocument();
  });

  it("tells the change list to refresh once inverses are staged", async () => {
    const user = userEvent.setup();
    const onStaged = vi.fn();
    renderPanel(onStaged);
    await user.click(await screen.findByRole("button", { name: "Undo 3" }));
    await waitFor(() => expect(onStaged).toHaveBeenCalled());
  });

  it("marks an already-undone run rather than offering it again", async () => {
    vi.mocked(listUndoRuns).mockResolvedValue([
      { ...RUN, undone_at: 1_700_000_100 },
    ]);
    renderPanel();
    expect(await screen.findByText("Undone")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Undo/ })).toBeNull();
  });

  it("does not offer to undo a run where nothing can be reversed", async () => {
    vi.mocked(listUndoRuns).mockResolvedValue([
      { ...RUN, reversible: 0, blocked: 2 },
    ]);
    renderPanel();
    expect(await screen.findByRole("button", { name: "Undo 0" })).toBeDisabled();
  });

  it("surfaces a backend refusal instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(undoRun).mockRejectedValue(
      new Error("that sync run has already been undone"),
    );
    renderPanel();
    await user.click(await screen.findByRole("button", { name: "Undo 3" }));
    expect(await screen.findByText(/already been undone/)).toBeInTheDocument();
  });
});
