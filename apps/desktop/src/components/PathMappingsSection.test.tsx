import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PathMappingsSection } from "./PathMappingsSection";
import {
  createPathMapping,
  deletePathMapping,
  listPathMappings,
  previewPathMapping,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({
  listPathMappings: vi.fn(),
  createPathMapping: vi.fn(),
  deletePathMapping: vi.fn(),
  previewPathMapping: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listPathMappings).mockResolvedValue([]);
  vi.mocked(createPathMapping).mockResolvedValue("m1");
  vi.mocked(deletePathMapping).mockResolvedValue(true);
  vi.mocked(previewPathMapping).mockResolvedValue([
    "/Users/me/Music/House/track.mp3",
    true,
  ]);
});

function renderSection() {
  render(
    <WithProviders>
      <PathMappingsSection />
    </WithProviders>,
  );
}

describe("PathMappingsSection", () => {
  it("says plainly that no mappings means paths are used verbatim", async () => {
    renderSection();
    expect(await screen.findByTestId("no-mappings")).toHaveTextContent(
      /exactly as the library stores them/,
    );
  });

  it("will not add a mapping with either prefix missing", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.type(screen.getByLabelText("Stored prefix"), "D:\\Music");
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
  });

  it("adds a mapping and reloads the list", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.type(screen.getByLabelText("Stored prefix"), "D:\\Music");
    await user.type(screen.getByLabelText("On this computer"), "/Users/me/Music");
    await user.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect(createPathMapping).toHaveBeenCalledWith("D:\\Music", "/Users/me/Music");
    });
    expect(listPathMappings).toHaveBeenCalledTimes(2);
  });

  it("lists existing mappings and removes one", async () => {
    const user = userEvent.setup();
    vi.mocked(listPathMappings).mockResolvedValue([
      { id: "m1", from: "D:\\Music", to: "/Users/me/Music" },
    ]);
    renderSection();
    await user.click(await screen.findByLabelText("Remove mapping D:\\Music"));
    await waitFor(() => {
      expect(deletePathMapping).toHaveBeenCalledWith("m1");
    });
  });

  it("tests a stored path and says whether the file is actually there", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.type(
      screen.getByLabelText("Test a stored path"),
      "D:\\Music\\House\\track.mp3",
    );
    await user.click(screen.getByRole("button", { name: "Test" }));

    const result = await screen.findByTestId("mapping-test-result");
    expect(result).toHaveTextContent("/Users/me/Music/House/track.mp3");
    expect(result).toHaveTextContent("file found");
  });

  it("says so when the mapped path leads nowhere", async () => {
    const user = userEvent.setup();
    vi.mocked(previewPathMapping).mockResolvedValue(["/nowhere/track.mp3", false]);
    renderSection();
    await user.type(screen.getByLabelText("Test a stored path"), "D:\\x.mp3");
    await user.click(screen.getByRole("button", { name: "Test" }));
    expect(await screen.findByTestId("mapping-test-result")).toHaveTextContent(
      "no file there",
    );
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(createPathMapping).mockRejectedValue(
      new Error("both prefixes are required"),
    );
    renderSection();
    await user.type(screen.getByLabelText("Stored prefix"), "D:\\Music");
    await user.type(screen.getByLabelText("On this computer"), "/m");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(
      await screen.findByText(/both prefixes are required/),
    ).toBeInTheDocument();
  });
});
