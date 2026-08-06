import { renderHook } from "@testing-library/react";
import { WithProviders } from "../test-utils/providers";
import { describe, expect, it, vi } from "vitest";
import { useTrackContextActions } from "./useTrackContextActions";
import type { Track } from "../types";

vi.mock("../ipc", () => ({
  playTrack: vi.fn(),
  analyzeTrack: vi.fn(),
  stageChange: vi.fn(),
  revealInFinder: vi.fn(),
  removeTrackFromPlaylist: vi.fn(),
}));

const TRACK = { id: "t1", title: "A", folder_path: "/a.mp3" } as Track;

function ids(actions: { id: string }[]): string[] {
  return actions.map((a) => a.id);
}

describe("useTrackContextActions — Send to → Move files", () => {
  it("is absent when no handler is supplied", () => {
    const { result } = renderHook(
      () =>
        useTrackContextActions({
          libraryPath: "/db",
          onShowDetails: vi.fn(),
        }),
      { wrapper: WithProviders },
    );
    expect(ids(result.current)).not.toContain("send-to-files");
  });

  it("appears when a handler is supplied, and says it moves files on disk", () => {
    const onSendToFiles = vi.fn();
    const { result } = renderHook(
      () =>
        useTrackContextActions({
          libraryPath: "/db",
          onShowDetails: vi.fn(),
          onSendToFiles,
        }),
      { wrapper: WithProviders },
    );
    const action = result.current.find((a) => a.id === "send-to-files");
    expect(action).toBeDefined();
    // The hint matters: this is the one context-menu entry that touches disk.
    expect(action?.hint).toMatch(/disk/i);
  });

  it("passes the right-clicked track to the handler", () => {
    const onSendToFiles = vi.fn();
    const { result } = renderHook(
      () =>
        useTrackContextActions({
          libraryPath: "/db",
          onShowDetails: vi.fn(),
          onSendToFiles,
        }),
      { wrapper: WithProviders },
    );
    result.current.find((a) => a.id === "send-to-files")?.onSelect(TRACK);
    expect(onSendToFiles).toHaveBeenCalledWith(TRACK);
  });
});
