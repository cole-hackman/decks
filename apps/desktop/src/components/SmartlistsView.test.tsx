import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SmartlistsView } from "./SmartlistsView";
import {
  createSmartlist,
  deleteSmartlist,
  evaluateSmartlist,
  generateSmartlists,
  listSmartlists,
  previewSmartlist,
  smartlistCompatibility,
  smartlistCounts,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { Smartlist, Track } from "../types";

vi.mock("../ipc", () => ({
  listSmartlists: vi.fn(),
  createSmartlist: vi.fn(),
  updateSmartlist: vi.fn(),
  deleteSmartlist: vi.fn(),
  evaluateSmartlist: vi.fn(),
  previewSmartlist: vi.fn(),
  smartlistCounts: vi.fn(),
  smartlistCompatibility: vi.fn(),
  generateSmartlists: vi.fn(),
}));

function track(id: string, title: string): Track {
  return {
    id,
    title,
    artist: "Artist",
    album: null,
    genre: "House",
    musical_key: "8A",
    bpm: 128,
    duration_secs: 300,
    rating: null,
    comment: null,
    folder_path: `/music/${id}.mp3`,
    analysis_data_path: null,
    file_type: 1,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    energy: null,
  };
}

const HOUSE: Smartlist = {
  id: "s1",
  name: "House",
  parent_folder_id: null,
  combinator: "all",
  clauses: [
    { rules: [{ field: "genre", op: "equals", value: { type: "text", value: "House" } }] },
  ],
  created_at: 0,
  updated_at: 0,
};

const GENERATED: Smartlist = {
  ...HOUSE,
  id: "s2",
  name: "Techno",
  parent_folder_id: "Lexicon",
};

beforeEach(() => {
  vi.mocked(listSmartlists).mockResolvedValue([HOUSE, GENERATED]);
  vi.mocked(smartlistCounts).mockResolvedValue({ s1: 42, s2: 7 });
  vi.mocked(smartlistCompatibility).mockResolvedValue({
    s1: { materialised: { reason: "Rekordbox only expresses tag (MyTag) rules" } },
    s2: "native",
  });
  vi.mocked(previewSmartlist).mockResolvedValue([track("t1", "A")]);
  vi.mocked(evaluateSmartlist).mockResolvedValue([
    track("t1", "Alpha"),
    track("t2", "Beta"),
  ]);
  vi.mocked(createSmartlist).mockResolvedValue(HOUSE);
  vi.mocked(deleteSmartlist).mockResolvedValue(undefined);
  vi.mocked(generateSmartlists).mockResolvedValue([GENERATED]);
});

function renderView() {
  return render(
    <WithProviders>
      <SmartlistsView libraryPath="/lib.db" />
    </WithProviders>,
  );
}

describe("SmartlistsView", () => {
  it("lists smartlists with counts and generated marker", async () => {
    renderView();
    expect(await screen.findByText("House")).toBeInTheDocument();
    expect(screen.getByText(/42 tracks/)).toBeInTheDocument();
    expect(screen.getByText(/7 tracks · All rules · generated/)).toBeInTheDocument();
  });

  it("surfaces Rekordbox compatibility per smartlist", async () => {
    renderView();
    await screen.findByText("House");
    expect(
      screen.getByText(/flattened to a playlist — Rekordbox only expresses tag/),
    ).toBeInTheDocument();
    expect(screen.getByText("Rekordbox: native MyTag smartlist")).toBeInTheDocument();
  });

  it("shows matching tracks when a smartlist is selected", async () => {
    const user = userEvent.setup();
    renderView();
    await screen.findByText("House");
    await user.click(screen.getAllByRole("button", { name: "Show" })[0]);

    await waitFor(() => {
      expect(evaluateSmartlist).toHaveBeenCalledWith("/lib.db", "s1");
    });
    expect(await screen.findByText("Alpha")).toBeInTheDocument();
    expect(screen.getByText("2 matching track(s)")).toBeInTheDocument();
  });

  it("hides OR grouping in Any-rule mode", async () => {
    const user = userEvent.setup();
    renderView();
    await screen.findByText("House");
    await user.click(screen.getByRole("button", { name: "New smartlist" }));

    // All-rules is the default, so OR grouping is offered.
    expect(screen.getByRole("button", { name: "+ OR condition" })).toBeInTheDocument();

    await user.click(screen.getByLabelText("Any rule"));
    expect(screen.queryByRole("button", { name: "+ OR condition" })).toBeNull();
    expect(
      screen.getByText(/OR grouping is only available in “All rules” mode/),
    ).toBeInTheDocument();
  });

  it("shows a live match count while editing", async () => {
    const user = userEvent.setup();
    renderView();
    await screen.findByText("House");
    await user.click(screen.getByRole("button", { name: "New smartlist" }));

    await waitFor(() => {
      expect(screen.getByTestId("preview-count")).toHaveTextContent("1 track(s) match");
    });
    expect(previewSmartlist).toHaveBeenCalled();
  });

  it("resets the operator when a field change invalidates it", async () => {
    const user = userEvent.setup();
    renderView();
    await screen.findByText("House");
    await user.click(screen.getByRole("button", { name: "New smartlist" }));

    // Default rule is Genre (text) with "is". Switch to BPM (number) — the
    // text-only operators must disappear rather than producing an invalid rule.
    await user.selectOptions(screen.getByLabelText("Field"), "bpm");
    const opSelect = screen.getByLabelText("Operator") as HTMLSelectElement;
    const options = Array.from(opSelect.options).map((o) => o.value);
    expect(options).not.toContain("contains");
    expect(options).toContain("between");
  });

  it("disables save until the smartlist is named", async () => {
    const user = userEvent.setup();
    renderView();
    await screen.findByText("House");
    await user.click(screen.getByRole("button", { name: "New smartlist" }));

    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();

    await user.type(screen.getByLabelText("Name"), "Peak time");
    expect(save).toBeEnabled();

    await user.click(save);
    await waitFor(() => {
      expect(createSmartlist).toHaveBeenCalledWith(
        "/lib.db",
        "Peak time",
        "all",
        expect.any(Array),
      );
    });
  });

  it("runs the generator and reports when nothing is new", async () => {
    const user = userEvent.setup();
    vi.mocked(generateSmartlists).mockResolvedValueOnce([]);
    renderView();
    await screen.findByText("House");

    await user.click(screen.getByRole("button", { name: "By decade" }));
    await waitFor(() => {
      expect(generateSmartlists).toHaveBeenCalledWith("/lib.db", { kind: "by_decade" });
    });
    expect(
      await screen.findByText(/Nothing new to generate/),
    ).toBeInTheDocument();
  });

  it("deletes a smartlist", async () => {
    const user = userEvent.setup();
    renderView();
    await screen.findByText("House");
    await user.click(screen.getAllByRole("button", { name: "Delete" })[0]);
    await waitFor(() => {
      expect(deleteSmartlist).toHaveBeenCalledWith("/lib.db", "s1");
    });
  });

  it("renders an empty state when there are no smartlists", async () => {
    vi.mocked(listSmartlists).mockResolvedValue([]);
    vi.mocked(smartlistCounts).mockResolvedValue({});
    vi.mocked(smartlistCompatibility).mockResolvedValue({});
    renderView();
    expect(await screen.findByText("No smartlists yet.")).toBeInTheDocument();
  });
});
