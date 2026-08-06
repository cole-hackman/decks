import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CommonTextSection } from "./CommonTextSection";
import {
  commonTextBlocklistAdd,
  commonTextBlocklistList,
  commonTextBlocklistRemove,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";

vi.mock("../ipc", () => ({
  commonTextBlocklistList: vi.fn(),
  commonTextBlocklistAdd: vi.fn(),
  commonTextBlocklistRemove: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(commonTextBlocklistList).mockResolvedValue(["(Original Mix)"]);
  vi.mocked(commonTextBlocklistAdd).mockResolvedValue(undefined);
  vi.mocked(commonTextBlocklistRemove).mockResolvedValue(undefined);
});

function renderSection() {
  render(
    <WithProviders>
      <CommonTextSection />
    </WithProviders>,
  );
}

describe("CommonTextSection", () => {
  it("lists what the fix will strip", async () => {
    renderSection();
    expect(await screen.findByText("(Original Mix)")).toBeInTheDocument();
  });

  it("says what an empty list means, not just that it is empty", async () => {
    vi.mocked(commonTextBlocklistList).mockResolvedValue([]);
    renderSection();
    expect(await screen.findByTestId("no-common-text")).toHaveTextContent(
      /proposes nothing/,
    );
  });

  it("adds a pattern", async () => {
    const user = userEvent.setup();
    renderSection();
    await screen.findByText("(Original Mix)");
    await user.type(screen.getByLabelText("Text to remove"), "(Extended Mix)");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await waitFor(() => {
      expect(commonTextBlocklistAdd).toHaveBeenCalledWith("(Extended Mix)");
    });
  });

  it("adding one already on the list is a no-op, not a duplicate row", async () => {
    // A duplicate would have the fix strip it twice and give the user two
    // rows they cannot tell apart.
    const user = userEvent.setup();
    renderSection();
    await screen.findByText("(Original Mix)");
    await user.type(screen.getByLabelText("Text to remove"), "(original mix)");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await waitFor(() => {
      expect(commonTextBlocklistAdd).not.toHaveBeenCalled();
    });
  });

  it("the Camelot preset adds all 24 keys", async () => {
    const user = userEvent.setup();
    renderSection();
    await screen.findByText("(Original Mix)");
    await user.click(screen.getByRole("button", { name: "Add Camelot keys" }));
    await waitFor(() => {
      expect(commonTextBlocklistAdd).toHaveBeenCalledTimes(24);
    });
    expect(commonTextBlocklistAdd).toHaveBeenCalledWith("8A");
    expect(commonTextBlocklistAdd).toHaveBeenCalledWith("12B");
  });

  it("a preset skips what is already there", async () => {
    const user = userEvent.setup();
    renderSection();
    await screen.findByText("(Original Mix)");
    await user.click(screen.getByRole("button", { name: "Add (Original Mix)" }));
    await waitFor(() => {
      expect(commonTextBlocklistAdd).not.toHaveBeenCalled();
    });
  });

  it("removes a pattern", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(await screen.findByLabelText("Remove (Original Mix)"));
    await waitFor(() => {
      expect(commonTextBlocklistRemove).toHaveBeenCalledWith("(Original Mix)");
    });
  });

  it("surfaces a backend error instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(commonTextBlocklistAdd).mockRejectedValue(new Error("cache locked"));
    renderSection();
    await screen.findByText("(Original Mix)");
    await user.type(screen.getByLabelText("Text to remove"), "x");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByText(/cache locked/)).toBeInTheDocument();
  });
});
