import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CustomTagsPanel } from "./CustomTagsPanel";
import { listTagCategories, listTags } from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", async () => {
  const actual = await vi.importActual<typeof import("../ipc")>("../ipc");
  return {
    ...actual,
    listTagCategories: vi.fn(),
    listTags: vi.fn(),
    createTagCategory: vi.fn(),
    createTag: vi.fn(),
    deleteTag: vi.fn(),
  };
});

beforeEach(() => {
  vi.clearAllMocks();
});

function render_(props: Partial<React.ComponentProps<typeof CustomTagsPanel>> = {}) {
  return render(
    <WithProviders>
      <CustomTagsPanel {...props} />
    </WithProviders>,
  );
}

describe("CustomTagsPanel", () => {
  it("renders a usage count badge when a tag has tracks", async () => {
    vi.mocked(listTagCategories).mockResolvedValue([
      { id: "c1", name: "Mood", seq: 0 },
    ]);
    vi.mocked(listTags).mockResolvedValue([
      { id: "t1", category_id: "c1", name: "Chill", seq: 0, usage_count: 7 },
      { id: "t2", category_id: "c1", name: "Hype", seq: 1, usage_count: 0 },
    ]);

    render_();

    // Category collapsed by default; expand it.
    await userEvent.click(await screen.findByText("Mood"));
    expect(await screen.findByText("Chill")).toBeInTheDocument();
    expect(screen.getByText("(7)")).toBeInTheDocument();
    // Tag with no bindings should not render a "(0)" badge.
    expect(screen.queryByText("(0)")).toBeNull();
  });

  it("renders a 'Show tracks' button after selecting tags and calls onShowTracks", async () => {
    vi.mocked(listTagCategories).mockResolvedValue([
      { id: "c1", name: "Mood", seq: 0 },
    ]);
    vi.mocked(listTags).mockResolvedValue([
      { id: "t1", category_id: "c1", name: "Chill", seq: 0, usage_count: 3 },
    ]);

    const onShowTracks = vi.fn();
    render_({ onShowTracks });

    await userEvent.click(await screen.findByText("Mood"));
    const chip = await screen.findByRole("button", { name: /^Chill/ });
    await userEvent.click(chip);

    const showBtn = await screen.findByRole("button", { name: /show 1 tag/i });
    await userEvent.click(showBtn);
    // The flat list, plus the same ids grouped by the category they came from.
    expect(onShowTracks).toHaveBeenCalledWith(["t1"], [["t1"]]);
  });

  it("groups the selection by category, which is what makes the semantics work", async () => {
    // Per the spec this page means "any within a category, all across". Two
    // genres and one mood is (House OR Techno) AND Peak, and a flat list of
    // three ids cannot say that.
    vi.mocked(listTagCategories).mockResolvedValue([
      { id: "genre", name: "Genre", seq: 0 },
      { id: "mood", name: "Mood", seq: 1 },
    ]);
    // The panel fetches every tag at once and groups them itself.
    vi.mocked(listTags).mockResolvedValue([
      { id: "house", category_id: "genre", name: "House", seq: 0, usage_count: 1 },
      { id: "techno", category_id: "genre", name: "Techno", seq: 1, usage_count: 1 },
      { id: "peak", category_id: "mood", name: "Peak", seq: 0, usage_count: 1 },
    ]);

    const onShowTracks = vi.fn();
    render_({ onShowTracks });

    await userEvent.click(await screen.findByText("Genre"));
    await userEvent.click(await screen.findByRole("button", { name: /^House/ }));
    await userEvent.click(await screen.findByRole("button", { name: /^Techno/ }));
    await userEvent.click(await screen.findByText("Mood"));
    await userEvent.click(await screen.findByRole("button", { name: /^Peak/ }));

    // And the rule is stated, not hidden.
    expect(screen.getByTestId("tag-selection-rule")).toHaveTextContent(
      "any within a category, all across",
    );

    await userEvent.click(
      await screen.findByRole("button", { name: /show 3 tags/i }),
    );
    const [, groups] = vi.mocked(onShowTracks).mock.calls[0];
    expect(groups).toEqual([["house", "techno"], ["peak"]]);
  });
});
