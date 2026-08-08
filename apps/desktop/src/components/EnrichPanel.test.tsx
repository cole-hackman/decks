import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EnrichPanel } from "./EnrichPanel";
import { enrichPreview, enrichStage } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { EnrichPreview } from "../types";

vi.mock("../ipc", () => ({
  enrichPreview: vi.fn(),
  enrichStage: vi.fn(),
}));

const PREVIEW: EnrichPreview = {
  tracks: [
    {
      track_id: "t1",
      proposals: [
        {
          field: "Genre",
          before: "House",
          after: "Deep House",
          source: "Discogs",
          confidence: 0.9,
        },
        {
          field: "Album",
          before: null,
          after: "Some Album",
          source: "MusicBrainz",
          confidence: 0.7,
        },
      ],
      tags: ["Tech House", "Minimal"],
      no_match: false,
    },
    {
      track_id: "t2",
      proposals: [],
      tags: [],
      no_match: true,
    },
    {
      track_id: "t4",
      proposals: [
        {
          field: "Label",
          before: null,
          after: "XL Recordings",
          source: "Discogs",
          confidence: 0.8,
        },
      ],
      tags: [],
      no_match: false,
    },
  ],
  unsearchable: ["t3"],
  errors: ["Discogs rate-limited"],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(enrichPreview).mockResolvedValue(PREVIEW);
  vi.mocked(enrichStage).mockResolvedValue(["t1"]);
});

function renderPanel(onClose = vi.fn()) {
  render(
    <WithProviders>
      <EnrichPanel
        libraryPath="/db"
        trackIds={["t1", "t2", "t3", "t4"]}
        onClose={onClose}
      />
    </WithProviders>,
  );
  return onClose;
}

/** Runs the lookup and waits for the results list to land. */
async function findTags() {
  await userEvent.click(screen.getByRole("button", { name: "Find tags" }));
  await screen.findByText("Deep House");
}

describe("EnrichPanel", () => {
  it("shows the claiming source on every proposal, since ADR-0008 forbids eliding it", async () => {
    renderPanel();
    await findTags();

    // Two different providers claimed two different fields on the same
    // track — both source names must be visible, not just one of them.
    expect(screen.getAllByText("Discogs").length).toBeGreaterThan(0);
    expect(screen.getByText("MusicBrainz")).toBeInTheDocument();
  });

  it("unchecking a proposal excludes it from the enrichStage call", async () => {
    renderPanel();
    await findTags();

    // Clicking the proposed value toggles the checkbox its <label> wraps.
    await userEvent.click(screen.getByText("Deep House"));
    await userEvent.click(
      screen.getByRole("button", { name: /^Accept selected/ }),
    );

    const [, accepted] = vi.mocked(enrichStage).mock.calls[0];
    const t1 = accepted.find((t) => t.track_id === "t1");
    expect(t1?.proposals.map((p) => p.field)).toEqual(["Album"]);
  });

  it("a track whose proposals are all unchecked is not sent at all", async () => {
    renderPanel();
    await findTags();

    // Uncheck every proposal and every tag chip on t1, leaving it with
    // nothing selected, while t4 stays fully checked.
    await userEvent.click(screen.getByText("Deep House"));
    await userEvent.click(screen.getByText("Some Album"));
    await userEvent.click(screen.getByText("Tech House"));
    await userEvent.click(screen.getByText("Minimal"));
    await userEvent.click(
      screen.getByRole("button", { name: /^Accept selected/ }),
    );

    const [, accepted] = vi.mocked(enrichStage).mock.calls[0];
    expect(accepted.find((t) => t.track_id === "t1")).toBeUndefined();
    expect(accepted.find((t) => t.track_id === "t4")).toBeDefined();
  });

  it("Cancel stages nothing", async () => {
    const onClose = renderPanel();
    await findTags();

    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(vi.mocked(enrichStage)).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("no_match tracks are surfaced with their own message pointing at Smart Fixes", async () => {
    renderPanel();
    await findTags();

    expect(screen.getByText(/Nothing found/)).toBeInTheDocument();
    expect(screen.getByText(/Smart Fixes/)).toBeInTheDocument();
    expect(screen.getByText("t2")).toBeInTheDocument();
  });

  it("unsearchable tracks are listed separately from tracks with no match", async () => {
    renderPanel();
    await findTags();

    expect(screen.getByText(/No usable title/)).toBeInTheDocument();
    expect(screen.getByText("t3")).toBeInTheDocument();
  });

  it("provider errors are displayed as a warning, not swallowed", async () => {
    renderPanel();
    await findTags();

    expect(screen.getByText("Discogs rate-limited")).toBeInTheDocument();
    // The run still produced usable proposals — the error strip must not
    // replace them or read as the whole run having failed.
    expect(screen.getByText("Deep House")).toBeInTheDocument();
  });

  it("sends the option checkboxes through to enrichPreview's request", async () => {
    renderPanel();

    await userEvent.click(screen.getByText("Original release"));
    await userEvent.click(screen.getByText("Use Discogs"));
    await userEvent.click(screen.getByRole("button", { name: "Find tags" }));

    expect(vi.mocked(enrichPreview)).toHaveBeenCalledWith({
      library_path: "/db",
      track_ids: ["t1", "t2", "t3", "t4"],
      original_release: true,
      use_discogs: true,
    });
  });

  it("offers no album-art option, because nothing can embed one yet", async () => {
    // `enrichment::cover_art` can fetch and identify a cover, but
    // `crates/audio-tags` has no picture support, so a checkbox here would
    // download an image and discard it — the stub logic CLAUDE.md forbids in
    // production paths. See ADR-0016.
    renderPanel();
    expect(screen.queryByText(/album art/i)).toBeNull();
  });
});
