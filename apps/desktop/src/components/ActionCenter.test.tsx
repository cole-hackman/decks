import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ActionCenter } from "./ActionCenter";
import { ActionProvider } from "../hooks/useActions";
import type { ActionDef } from "../lib/actions";

function renderPalette(actions: ActionDef[], open = true) {
  const onClose = vi.fn();
  render(
    <ActionProvider actions={actions}>
      <ActionCenter open={open} onClose={onClose} />
    </ActionProvider>,
  );
  return { onClose };
}

const PLAY = vi.fn();
const JUMP = vi.fn();

function actions(): ActionDef[] {
  return [
    {
      id: "player.playPause",
      label: "Play / pause",
      group: "Player",
      defaultBinding: { key: " " },
      run: PLAY,
    },
    {
      id: "player.beatJump",
      label: "Beat jump forward",
      group: "Player",
      defaultBinding: { key: "arrowright", meta: true },
      run: JUMP,
    },
    {
      id: "app.secret",
      label: "Hidden thing",
      group: "App",
      hidden: true,
      run: vi.fn(),
    },
    {
      id: "app.off",
      label: "Disabled thing",
      group: "App",
      enabled: false,
      run: vi.fn(),
    },
  ];
}

describe("ActionCenter", () => {
  it("renders nothing when closed", () => {
    renderPalette(actions(), false);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("lists enabled, non-hidden actions with their bindings", () => {
    renderPalette(actions());
    expect(screen.getByText("Play / pause")).toBeInTheDocument();
    expect(screen.getByText("Beat jump forward")).toBeInTheDocument();
    // Hidden and disabled actions stay out of the palette.
    expect(screen.queryByText("Hidden thing")).toBeNull();
    expect(screen.queryByText("Disabled thing")).toBeNull();
    expect(screen.getByText("Space")).toBeInTheDocument();
  });

  it("filters by subsequence, not just prefix", async () => {
    const user = userEvent.setup();
    renderPalette(actions());
    await user.type(screen.getByLabelText("Run a command"), "bjf");
    expect(screen.getByText("Beat jump forward")).toBeInTheDocument();
    expect(screen.queryByText("Play / pause")).toBeNull();
  });

  it("runs the highlighted action on Enter and closes", async () => {
    const user = userEvent.setup();
    const { onClose } = renderPalette(actions());
    const input = screen.getByLabelText("Run a command");
    await user.type(input, "beat");
    await user.keyboard("{Enter}");
    expect(JUMP).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("runs the first result when Enter is pressed with nothing moved", async () => {
    const user = userEvent.setup();
    PLAY.mockClear();
    renderPalette(actions());
    await user.keyboard("{Enter}");
    expect(PLAY).toHaveBeenCalled();
  });

  it("moves the highlight with the arrow keys", async () => {
    const user = userEvent.setup();
    renderPalette(actions());
    const options = screen.getAllByRole("option");
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}");
    expect(screen.getAllByRole("option")[1]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowUp}");
    expect(screen.getAllByRole("option")[0]).toHaveAttribute("aria-selected", "true");
  });

  it("shows an empty state for a query that matches nothing", async () => {
    const user = userEvent.setup();
    renderPalette(actions());
    await user.type(screen.getByLabelText("Run a command"), "zzzzz");
    expect(screen.getByText("No matching command.")).toBeInTheDocument();
  });

  it("closes on Escape and on backdrop click", async () => {
    const user = userEvent.setup();
    const { onClose } = renderPalette(actions());
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();

    onClose.mockClear();
    await user.click(screen.getByTestId("action-center-backdrop"));
    expect(onClose).toHaveBeenCalled();
  });
});
