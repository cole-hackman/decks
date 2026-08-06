import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AutomaticActionsSection } from "./AutomaticActionsSection";
import { listAutomaticActions, setAutomaticAction } from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { AutomaticAction } from "../types";

vi.mock("../ipc", () => ({
  listAutomaticActions: vi.fn(),
  setAutomaticAction: vi.fn(),
}));

const ACTIONS: AutomaticAction[] = [
  {
    key: "auto_analyze_new_tracks",
    label: "Auto-analyse new tracks",
    description: "Detect BPM and key when a file arrives.",
    enabled: false,
    unavailable: null,
  },
  {
    key: "auto_reencode_new_files",
    label: "Auto re-encode new MP3/M4A",
    description: "Run the Beatshift Fixer on arrival.",
    enabled: false,
    unavailable: "Needs the Beatshift Fixer, which is not built yet.",
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listAutomaticActions).mockResolvedValue(ACTIONS);
  vi.mocked(setAutomaticAction).mockResolvedValue(undefined);
});

function renderSection() {
  render(
    <WithProviders>
      <AutomaticActionsSection />
    </WithProviders>,
  );
}

describe("AutomaticActionsSection", () => {
  it("toggles an action that works", async () => {
    const user = userEvent.setup();
    renderSection();
    await user.click(await screen.findByLabelText("Auto-analyse new tracks"));
    await waitFor(() => {
      expect(setAutomaticAction).toHaveBeenCalledWith(
        "auto_analyze_new_tracks",
        true,
      );
    });
  });

  it("disables an action decks cannot honour, and says why", async () => {
    renderSection();
    expect(
      await screen.findByLabelText("Auto re-encode new MP3/M4A"),
    ).toBeDisabled();
    expect(
      screen.getByTestId("unavailable-auto_reencode_new_files"),
    ).toHaveTextContent(/Beatshift Fixer/);
  });

  it("lists unavailable actions rather than hiding them", async () => {
    // Hiding them would make the gap invisible.
    renderSection();
    expect(await screen.findByLabelText("Auto-analyse new tracks")).toBeInTheDocument();
    expect(screen.getByLabelText("Auto re-encode new MP3/M4A")).toBeInTheDocument();
  });

  it("surfaces a backend refusal instead of appearing to succeed", async () => {
    const user = userEvent.setup();
    vi.mocked(setAutomaticAction).mockRejectedValue(new Error("config is read-only"));
    renderSection();
    await user.click(await screen.findByLabelText("Auto-analyse new tracks"));
    expect(await screen.findByText(/config is read-only/)).toBeInTheDocument();
  });

  it("survives a host that returns nothing", async () => {
    vi.mocked(listAutomaticActions).mockRejectedValue(new Error("nope"));
    renderSection();
    await waitFor(() => {
      expect(listAutomaticActions).toHaveBeenCalled();
    });
    expect(screen.queryByLabelText("Auto-analyse new tracks")).toBeNull();
  });
});
