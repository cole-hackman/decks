import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CustomTagsPanel } from "./CustomTagsPanel";
import {
  listTagCategories,
  listTags,
  previewMyTagImport,
  importMyTags,
} from "../ipc";
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
    previewMyTagImport: vi.fn(),
    importMyTags: vi.fn(),
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

  // ── MyTag import ───────────────────────────────────────────────────────────

  it("does not offer the MyTag import with no library open", async () => {
    vi.mocked(listTagCategories).mockResolvedValue([]);
    vi.mocked(listTags).mockResolvedValue([]);
    render_();
    expect(
      screen.queryByRole("button", { name: "Check for MyTags" }),
    ).not.toBeInTheDocument();
  });

  it("previews before importing anything", async () => {
    vi.mocked(listTagCategories).mockResolvedValue([]);
    vi.mocked(listTags).mockResolvedValue([]);
    vi.mocked(previewMyTagImport).mockResolvedValue({
      new_categories: ["Genre"],
      new_tags: [["Genre", "Techno"]],
      existing_tags: 0,
      new_links: 3,
      unmatched_links: 0,
    });
    const user = userEvent.setup();
    render_({ libraryPath: "/lib.db" });

    await user.click(
      await screen.findByRole("button", { name: "Check for MyTags" }),
    );
    const preview = await screen.findByTestId("mytag-preview");
    expect(preview).toHaveTextContent("1 new category(ies)");
    expect(preview).toHaveTextContent("3 new track link(s)");
    expect(importMyTags).not.toHaveBeenCalled();
  });

  it("warns when the MyTag data points outside this library", async () => {
    vi.mocked(listTagCategories).mockResolvedValue([]);
    vi.mocked(listTags).mockResolvedValue([]);
    vi.mocked(previewMyTagImport).mockResolvedValue({
      new_categories: [],
      new_tags: [],
      existing_tags: 4,
      new_links: 0,
      unmatched_links: 812,
    });
    const user = userEvent.setup();
    render_({ libraryPath: "/lib.db" });

    await user.click(
      await screen.findByRole("button", { name: "Check for MyTags" }),
    );
    expect(
      await screen.findByText(/812 link\(s\) point at tracks that are not/),
    ).toBeInTheDocument();
  });

  it("imports on the second click and reports what it made", async () => {
    vi.mocked(listTagCategories).mockResolvedValue([]);
    vi.mocked(listTags).mockResolvedValue([]);
    vi.mocked(previewMyTagImport).mockResolvedValue({
      new_categories: ["Genre"],
      new_tags: [["Genre", "Techno"]],
      existing_tags: 0,
      new_links: 3,
      unmatched_links: 0,
    });
    vi.mocked(importMyTags).mockResolvedValue({
      categories_created: 1,
      tags_created: 1,
      links_created: 3,
    });
    const user = userEvent.setup();
    render_({ libraryPath: "/lib.db" });

    await user.click(
      await screen.findByRole("button", { name: "Check for MyTags" }),
    );
    await user.click(await screen.findByRole("button", { name: "Import" }));
    expect(await screen.findByTestId("mytag-done")).toHaveTextContent(
      "Imported 1 category(ies), 1 tag(s) and 3 track link(s).",
    );
  });

  it("says a re-import did nothing rather than claiming success", async () => {
    // The import is idempotent; reporting "imported 0" as a win would be a lie
    // about what happened.
    vi.mocked(listTagCategories).mockResolvedValue([]);
    vi.mocked(listTags).mockResolvedValue([]);
    vi.mocked(previewMyTagImport).mockResolvedValue({
      new_categories: [],
      new_tags: [],
      existing_tags: 2,
      new_links: 0,
      unmatched_links: 0,
    });
    vi.mocked(importMyTags).mockResolvedValue({
      categories_created: 0,
      tags_created: 0,
      links_created: 0,
    });
    const user = userEvent.setup();
    render_({ libraryPath: "/lib.db" });

    await user.click(
      await screen.findByRole("button", { name: "Check for MyTags" }),
    );
    await user.click(await screen.findByRole("button", { name: "Import" }));
    expect(await screen.findByTestId("mytag-done")).toHaveTextContent(
      /nothing to do/,
    );
  });

  it("surfaces a failed preview", async () => {
    vi.mocked(listTagCategories).mockResolvedValue([]);
    vi.mocked(listTags).mockResolvedValue([]);
    vi.mocked(previewMyTagImport).mockRejectedValue(new Error("no such table"));
    const user = userEvent.setup();
    render_({ libraryPath: "/lib.db" });

    await user.click(
      await screen.findByRole("button", { name: "Check for MyTags" }),
    );
    expect(await screen.findByTestId("mytag-error")).toHaveTextContent(
      "no such table",
    );
  });
});
