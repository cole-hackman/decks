import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PathRewriteSection } from "./PathRewriteSection";
import { applyPathRewrite, previewPathRewrite } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { RewritePreview } from "../types";

vi.mock("../ipc", () => ({
  previewPathRewrite: vi.fn(),
  applyPathRewrite: vi.fn(),
}));

const PREVIEW: RewritePreview = {
  considered: 3,
  plan: {
    rewrites: [
      { track_id: "t1", from: "D:/Music/a.mp3", to: "/Volumes/Music/a.mp3" },
    ],
    skipped: [
      ["t2", "/elsewhere/b.mp3", { kind: "no_match" }],
      [
        "t3",
        "D:/Music/c.mp3",
        { kind: "taken", detail: "/Volumes/Music/c.mp3" },
      ],
    ],
  },
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(previewPathRewrite).mockResolvedValue(PREVIEW);
  vi.mocked(applyPathRewrite).mockResolvedValue(["c1"]);
});

function renderSection() {
  render(
    <WithProviders>
      <PathRewriteSection libraryPath="/lib.db" />
    </WithProviders>,
  );
}

async function fill(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText("Source prefix"), "D:/Music");
  await user.type(screen.getByLabelText("Target prefix"), "/Volumes/Music");
}

describe("PathRewriteSection", () => {
  it("cannot preview without a source prefix", () => {
    // An empty prefix matches every path in the library — never what anyone
    // meant.
    renderSection();
    expect(screen.getByRole("button", { name: "Preview rewrite" })).toBeDisabled();
  });

  it("sends the prefixes the user typed", async () => {
    const user = userEvent.setup();
    renderSection();
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Preview rewrite" }));
    await waitFor(() => {
      expect(previewPathRewrite).toHaveBeenCalledWith("/lib.db", {
        from_prefix: "D:/Music",
        to_prefix: "/Volumes/Music",
        new_extension: null,
        all_tracks: false,
      });
    });
  });

  it("passes an extension substitution through", async () => {
    const user = userEvent.setup();
    renderSection();
    await fill(user);
    await user.type(screen.getByLabelText("New extension"), "mp3");
    await user.click(screen.getByRole("button", { name: "Preview rewrite" }));
    await waitFor(() => {
      expect(previewPathRewrite).toHaveBeenCalledWith(
        "/lib.db",
        expect.objectContaining({ new_extension: "mp3" }),
      );
    });
  });

  it("warns before rewriting paths that currently work", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(
      screen.getByLabelText("Include tracks that are not missing"),
    );
    expect(screen.getByTestId("all-tracks-warning")).toHaveTextContent(
      /whole folder moved/,
    );
  });

  it("reports how many of how many would be rewritten", async () => {
    // "1 rewritten" alone reads as though the library has one track.
    const user = userEvent.setup();
    renderSection();
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Preview rewrite" }));
    expect(
      await screen.findByText(/1 of 3 track\(s\) would be rewritten/),
    ).toBeInTheDocument();
  });

  it("lists collisions but not every non-matching path", async () => {
    // "does not start with that prefix" over 4,000 tracks is noise; a
    // collision is something the user has to decide about.
    const user = userEvent.setup();
    renderSection();
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Preview rewrite" }));
    const collisions = await screen.findByTestId("rewrite-collisions");
    expect(collisions).toHaveTextContent(/already at \/Volumes\/Music\/c\.mp3/);
    expect(screen.queryByText(/\/elsewhere\/b\.mp3/)).not.toBeInTheDocument();
  });

  it("says so when nothing matched", async () => {
    const user = userEvent.setup();
    vi.mocked(previewPathRewrite).mockResolvedValue({
      considered: 3,
      plan: { rewrites: [], skipped: [] },
    });
    renderSection();
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Preview rewrite" }));
    expect(
      await screen.findByText(/No path starts with that prefix/),
    ).toBeInTheDocument();
  });

  it("stages exactly what the preview showed", async () => {
    const user = userEvent.setup();
    renderSection();
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Preview rewrite" }));
    await user.click(
      await screen.findByRole("button", { name: /Stage 1 relocation/ }),
    );
    await waitFor(() => {
      expect(applyPathRewrite).toHaveBeenCalledWith(
        "/lib.db",
        PREVIEW.plan.rewrites,
      );
    });
  });

  it("drops a stale preview when the prefix changes", async () => {
    const user = userEvent.setup();
    renderSection();
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Preview rewrite" }));
    await screen.findByTestId("rewrite-preview");
    await user.type(screen.getByLabelText("Target prefix"), "/more");
    expect(screen.queryByTestId("rewrite-preview")).not.toBeInTheDocument();
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(previewPathRewrite).mockRejectedValue(new Error("library locked"));
    renderSection();
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Preview rewrite" }));
    expect(await screen.findByText(/library locked/)).toBeInTheDocument();
  });
});
