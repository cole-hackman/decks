import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SyncPanel } from "./SyncPanel";
import { syncCheck, syncExecute, syncPreview } from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({
  listPlaylists: vi.fn().mockResolvedValue([]),
  syncCheck: vi.fn(),
  syncExecute: vi.fn(),
  syncPreview: vi.fn(),
}));

const ROW = {
  change_id: "c1",
  kind: "TrackMetadataEdit",
  track_id: "t1",
  track_title: "Some Title",
  field: "Title",
  old_value: "Old",
  new_value: "New",
  reason: null,
  updated_at: 1,
};

beforeEach(() => {
  vi.clearAllMocks();
});

function render_() {
  return render(
    <WithProviders>
      <SyncPanel libraryPath="/db" />
    </WithProviders>,
  );
}

describe("SyncPanel", () => {
  it("renders empty state when nothing staged", async () => {
    vi.mocked(syncCheck).mockResolvedValue({ locked: false, pending_changes: 0 });
    vi.mocked(syncPreview).mockResolvedValue([]);
    render_();
    expect(await screen.findByText(/Nothing staged/)).toBeInTheDocument();
  });

  it("shows lock banner when master.db is locked", async () => {
    vi.mocked(syncCheck).mockResolvedValue({ locked: true, pending_changes: 1 });
    vi.mocked(syncPreview).mockResolvedValue([ROW]);
    render_();
    expect(
      await screen.findByText(/Rekordbox appears to be running/),
    ).toBeInTheDocument();
  });

  it("excludes a row when its checkbox is toggled off", async () => {
    vi.mocked(syncCheck).mockResolvedValue({ locked: false, pending_changes: 1 });
    vi.mocked(syncPreview).mockResolvedValue([ROW]);
    render_();
    await screen.findByText("Some Title");
    expect(screen.getByText(/1 of 1 included/)).toBeInTheDocument();
    // The row's include toggle sits inside the pending-changes table; the
    // keep-grids options checkbox is outside any <table>.
    const table = screen.getByRole("table");
    const rowCheckbox = table.querySelector(
      "input[type=checkbox]",
    ) as HTMLInputElement;
    expect(rowCheckbox).toBeTruthy();
    await userEvent.click(rowCheckbox);
    expect(screen.getByText(/0 of 1 included/)).toBeInTheDocument();
  });

  it("Apply calls syncExecute with included change ids", async () => {
    vi.mocked(syncCheck).mockResolvedValue({ locked: false, pending_changes: 1 });
    vi.mocked(syncPreview).mockResolvedValue([ROW]);
    vi.mocked(syncExecute).mockResolvedValue({ applied: ["c1"], failed: [] });
    render_();
    await screen.findByText("Some Title");
    await userEvent.click(screen.getByRole("button", { name: /Apply 1 change/ }));
    await userEvent.click(await screen.findByRole("button", { name: "Apply" }));
    expect(syncExecute).toHaveBeenCalledWith(
      "/db",
      "full",
      expect.any(Object),
      ["c1"],
    );
  });

  it("forwards non-default cue_destination, keep_grids, and convert_keys to syncExecute", async () => {
    vi.mocked(syncCheck).mockResolvedValue({ locked: false, pending_changes: 1 });
    vi.mocked(syncPreview).mockResolvedValue([ROW]);
    vi.mocked(syncExecute).mockResolvedValue({ applied: ["c1"], failed: [] });
    render_();
    await screen.findByText("Some Title");

    // Flip all three options off their defaults.
    const cueSelect = screen.getByRole("combobox", { name: /Cue destination/i });
    await userEvent.selectOptions(cueSelect, "both");
    const keysSelect = screen.getByRole("combobox", { name: /Convert keys/i });
    await userEvent.selectOptions(keysSelect, "camelot");
    const keepGridsBox = screen.getByRole("checkbox", { name: /grids/i });
    await userEvent.click(keepGridsBox);

    await userEvent.click(screen.getByRole("button", { name: /Apply 1 change/ }));
    await userEvent.click(await screen.findByRole("button", { name: "Apply" }));

    expect(syncExecute).toHaveBeenCalledWith(
      "/db",
      "full",
      expect.objectContaining({
        cue_destination: "both",
        keep_grids: true,
        convert_keys: "camelot",
      }),
      ["c1"],
    );
  });

  it("surfaces ApplyResult.warnings (e.g. key conversion failures) in the toast detail", async () => {
    vi.mocked(syncCheck).mockResolvedValue({ locked: false, pending_changes: 1 });
    vi.mocked(syncPreview).mockResolvedValue([ROW]);
    vi.mocked(syncExecute).mockResolvedValue({
      applied: ["c1"],
      failed: [],
      warnings: ["Failed to convert key for track t1: 'C♭ Major' could not be mapped to Camelot"],
    });
    render_();
    await screen.findByText("Some Title");
    await userEvent.click(screen.getByRole("button", { name: /Apply 1 change/ }));
    await userEvent.click(await screen.findByRole("button", { name: "Apply" }));

    // The toast message reflects the warning count and the detail line shows
    // the underlying message verbatim.
    expect(await screen.findByText(/Applied 1 change\(s\), 1 warning\./)).toBeInTheDocument();
    expect(
      screen.getByText(/could not be mapped to Camelot/),
    ).toBeInTheDocument();
  });
  it("forwards the leading-zero option, and says why it exists", async () => {
    // Sorted as text an unpadded library reads 1A, 10A, 11A, 2A. The label has
    // to carry that, or the option looks like cosmetics.
    render_();
    await screen.findByText("Some Title");
    const box = screen.getByRole("checkbox", { name: /leading zero/i });
    expect(box).not.toBeChecked();
    expect(screen.getByText(/so apps sort them correctly/)).toBeInTheDocument();

    await userEvent.click(box);
    await userEvent.click(screen.getByRole("button", { name: /Apply 1 change/ }));
    await userEvent.click(await screen.findByRole("button", { name: "Apply" }));

    expect(vi.mocked(syncExecute).mock.calls.at(-1)?.[2]).toMatchObject({
      add_leading_zero: true,
    });
  });

  it("forwards the nearest-colour option, off by default", async () => {
    // Default off is the load-bearing part: a colour Rekordbox cannot represent
    // is left alone rather than quietly approximated to something the user
    // never picked.
    render_();
    await screen.findByText("Some Title");
    const box = screen.getByRole("checkbox", { name: /nearest one/i });
    expect(box).not.toBeChecked();

    await userEvent.click(box);
    await userEvent.click(screen.getByRole("button", { name: /Apply 1 change/ }));
    await userEvent.click(await screen.findByRole("button", { name: "Apply" }));

    expect(vi.mocked(syncExecute).mock.calls.at(-1)?.[2]).toMatchObject({
      change_to_nearest_color: true,
    });
  });
});
