import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderHook, act } from "@testing-library/react";
import { PlayQueuePanel } from "./PlayQueuePanel";
import { usePlayQueue } from "../hooks/usePlayQueue";
import type { Track } from "../types";

function track(id: string, title: string, artist: string | null): Track {
  return {
    id,
    title,
    artist,
    album: null,
    genre: null,
    musical_key: null,
    bpm: 128,
    duration_secs: 200,
    rating: null,
    comment: null,
    folder_path: `/music/${id}.mp3`,
    analysis_data_path: null,
    file_type: null,
    sample_rate: null,
    bit_rate: null,
    release_year: null,
    dj_play_count: null,
    label: null,
    remixer: null,
    mix: null,
    color: null,
    date_added: null,
    energy: null,
  };
}

const LIBRARY = [
  track("1", "Dark Matter", "Surgeon"),
  track("2", "Acid Rain", "Aphex Twin"),
  track("3", "Windowlicker", null),
];

/**
 * The hook under test, with `endedAt` as a rerender prop so a track draining
 * mid-set is expressible.
 *
 * Deliberately not named `use…`: the call sits inside `renderHook`'s callback,
 * which the rules-of-hooks lint reads as a hook called from a callback when the
 * enclosing helper looks like a hook itself.
 */
function queueUnderTest(endedAt: number | null = null) {
  const play = vi.fn();
  const hook = renderHook(
    ({ ended }: { ended: number | null }) =>
      usePlayQueue({ library: LIBRARY, play, endedAt: ended }),
    { initialProps: { ended: endedAt } },
  );
  return { hook, play };
}

describe("usePlayQueue", () => {
  it("starts the transport when the marker lands on a track", () => {
    const { hook, play } = queueUnderTest();
    act(() => hook.result.current.startPlaying([LIBRARY[0]]));
    expect(play).toHaveBeenCalledWith(LIBRARY[0]);
  });

  it("does not restart the same track when the library re-renders", () => {
    // A library refetch changes object identity; without a guard the transport
    // would restart from zero mid-track.
    const { hook, play } = queueUnderTest();
    act(() => hook.result.current.startPlaying([LIBRARY[0]]));
    hook.rerender({ ended: null });
    hook.rerender({ ended: null });
    expect(play).toHaveBeenCalledTimes(1);
  });

  it("advances when the track drains and autoplay is on", () => {
    const { hook, play } = queueUnderTest();
    act(() => hook.result.current.startPlaying([LIBRARY[0]]));
    act(() => hook.result.current.addToQueue([LIBRARY[1]]));
    hook.rerender({ ended: 1000 });
    expect(play).toHaveBeenLastCalledWith(LIBRARY[1]);
  });

  it("stays put when autoplay is off", () => {
    const { hook, play } = queueUnderTest();
    act(() => hook.result.current.startPlaying([LIBRARY[0]]));
    act(() => hook.result.current.addToQueue([LIBRARY[1]]));
    act(() => hook.result.current.setAutoplay(false));
    hook.rerender({ ended: 1000 });
    expect(play).toHaveBeenCalledTimes(1);
  });

  it("advances twice for two consecutive ends", () => {
    // The reason `endedAt` is a timestamp and not a boolean: a boolean would
    // stay true and the second track would never advance.
    const { hook, play } = queueUnderTest();
    act(() =>
      hook.result.current.startPlaying([LIBRARY[0], LIBRARY[1], LIBRARY[2]]),
    );
    hook.rerender({ ended: 1000 });
    expect(play).toHaveBeenLastCalledWith(LIBRARY[1]);
    hook.rerender({ ended: 2000 });
    expect(play).toHaveBeenLastCalledWith(LIBRARY[2]);
  });

  it("stops at the end of the queue rather than looping", () => {
    const { hook, play } = queueUnderTest();
    act(() => hook.result.current.startPlaying([LIBRARY[0]]));
    hook.rerender({ ended: 1000 });
    expect(play).toHaveBeenCalledTimes(1);
  });

  it("drops ids the library no longer has", () => {
    const { hook } = queueUnderTest();
    act(() => hook.result.current.addToQueue([track("gone", "Gone", null)]));
    expect(hook.result.current.tracks).toHaveLength(0);
  });
});

describe("PlayQueuePanel", () => {
  /**
   * Renders the panel over a live `usePlayQueue`, with a `seed` button that
   * loads the queue. Testing against the real hook rather than a hand-built
   * state object is the point — it is what catches the panel and the state
   * machine disagreeing.
   */
  function renderPanel(seed: Track[] = LIBRARY) {
    const play = vi.fn();
    function Harness() {
      const queue = usePlayQueue({ library: LIBRARY, play, endedAt: null });
      return (
        <>
          <button onClick={() => queue.startPlaying(seed)}>seed</button>
          <PlayQueuePanel queue={queue} onReveal={vi.fn()} />
        </>
      );
    }
    render(<Harness />);
    const user = userEvent.setup();
    return {
      play,
      user,
      seedQueue: () => user.click(screen.getByRole("button", { name: "seed" })),
    };
  }

  it("says so when nothing is queued, and how to fix that", () => {
    renderPanel();
    expect(screen.getByText(/Right-click a track/)).toBeInTheDocument();
  });

  it("lists queued tracks and marks the one playing", async () => {
    const { seedQueue } = renderPanel();
    await seedQueue();

    const list = screen.getByTestId("queue-list");
    expect(list).toHaveTextContent("Surgeon — Dark Matter");
    expect(list).toHaveTextContent("Aphex Twin — Acid Rain");
    // A track with no artist reads as its title rather than "null — ".
    expect(list).toHaveTextContent("Windowlicker");
    expect(list.querySelector('[aria-current="true"]')).toHaveTextContent(
      "Dark Matter",
    );
  });

  it("counts what is still to come, not the whole queue", async () => {
    const { seedQueue } = renderPanel();
    await seedQueue();
    expect(screen.getByText("2 up next of 3")).toBeInTheDocument();
  });

  it("removes an entry", async () => {
    const { user, seedQueue } = renderPanel();
    await seedQueue();

    await user.click(
      screen.getByRole("button", {
        name: "Remove Aphex Twin — Acid Rain from queue",
      }),
    );
    expect(screen.getByTestId("queue-list")).not.toHaveTextContent("Acid Rain");
  });

  it("reorders with the move buttons", async () => {
    const { user, seedQueue } = renderPanel();
    await seedQueue();

    await user.click(screen.getByRole("button", { name: "Move Windowlicker up" }));
    const rows = screen.getByTestId("queue-list").querySelectorAll("li");
    expect(rows[1]).toHaveTextContent("Windowlicker");
  });

  it("Clear keeps the playing track — it is not Stop", async () => {
    const { user, seedQueue } = renderPanel();
    await seedQueue();

    await user.click(screen.getByRole("button", { name: /Clear/ }));
    const list = screen.getByTestId("queue-list");
    expect(list).toHaveTextContent("Dark Matter");
    expect(list).not.toHaveTextContent("Acid Rain");
  });

  it("plays a queued entry when its play button is pressed", async () => {
    const { play, user, seedQueue } = renderPanel();
    await seedQueue();

    await user.click(
      screen.getByRole("button", { name: "Play Aphex Twin — Acid Rain" }),
    );
    expect(play).toHaveBeenLastCalledWith(LIBRARY[1]);
  });

  it("cannot shuffle fewer than two upcoming tracks", async () => {
    const { seedQueue } = renderPanel([LIBRARY[0]]);
    await seedQueue();
    expect(screen.getByRole("button", { name: /Shuffle/ })).toBeDisabled();
  });
});
