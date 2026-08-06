import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { TrackTimeline } from "./TrackTimeline";
import { LARGE_PLAYLIST_THRESHOLD, type TimelineTrack } from "../lib/timeline";

function track(over: Partial<TimelineTrack> & { id: string }): TimelineTrack {
  return {
    title: `Track ${over.id}`,
    artist: "Someone",
    musical_key: null,
    bpm: null,
    rating: null,
    energy: null,
    ...over,
  };
}

const SET: TimelineTrack[] = [
  track({ id: "a", bpm: 124, musical_key: "8A" }),
  track({ id: "b", bpm: 128, musical_key: "9A" }),
  track({ id: "c", bpm: 126, musical_key: "2B" }),
];

describe("TrackTimeline", () => {
  it("renders nothing for an empty set", () => {
    const { container } = render(<TrackTimeline tracks={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("draws a bar per track", () => {
    render(<TrackTimeline tracks={SET} />);
    expect(
      screen.getByTestId("timeline-bars").querySelectorAll("button"),
    ).toHaveLength(3);
  });

  it("puts the value and direction in the label, so colour is not the only cue", () => {
    render(<TrackTimeline tracks={SET} />);
    expect(
      screen.getByRole("button", { name: "Track b — 128.0 BPM ↑" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Track c — 126.0 BPM ↓" }),
    ).toBeInTheDocument();
  });

  it("explains what the colours mean", () => {
    render(<TrackTimeline tracks={SET} />);
    expect(screen.getByTestId("timeline-legend")).toHaveTextContent(
      "Green: tempo rose · Red: fell · Grey: held",
    );
  });

  it("switches to key colouring and says so", async () => {
    render(<TrackTimeline tracks={SET} />);
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Timeline colour mode" }),
      "key",
    );
    expect(screen.getByTestId("timeline-legend")).toHaveTextContent(
      "Coloured by key",
    );
  });

  it("switches the metric and relabels the bars", async () => {
    render(
      <TrackTimeline
        tracks={[
          track({ id: "a", rating: 3, bpm: 120 }),
          track({ id: "b", rating: 5, bpm: 128 }),
        ]}
      />,
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Timeline metric" }),
      "rating",
    );
    expect(
      screen.getByRole("button", { name: "Track b — 5★" }),
    ).toBeInTheDocument();
  });

  it("counts key changes that leave the wheel", () => {
    // 8A → 9A mixes; 9A → 2B does not.
    render(<TrackTimeline tracks={SET} />);
    expect(screen.getByTestId("timeline-clashes")).toHaveTextContent(
      "1 key change(s) outside the wheel",
    );
  });

  it("says nothing about clashes when the set is harmonically clean", () => {
    render(
      <TrackTimeline
        tracks={[
          track({ id: "a", musical_key: "8A", bpm: 124 }),
          track({ id: "b", musical_key: "9A", bpm: 126 }),
        ]}
      />,
    );
    expect(screen.queryByTestId("timeline-clashes")).not.toBeInTheDocument();
  });

  it("hides itself on a large set, and says why", () => {
    // It is a set-building tool, not a collection tool.
    const many = Array.from({ length: LARGE_PLAYLIST_THRESHOLD + 1 }, (_, i) =>
      track({ id: `t${i}`, bpm: 120 + (i % 8) }),
    );
    render(<TrackTimeline tracks={many} />);
    expect(screen.getByTestId("timeline-hidden")).toHaveTextContent(
      "for building a set, not browsing a collection",
    );
    expect(screen.queryByTestId("timeline-bars")).not.toBeInTheDocument();
  });

  it("can still be asked for on a large set", async () => {
    const many = Array.from({ length: LARGE_PLAYLIST_THRESHOLD + 1 }, (_, i) =>
      track({ id: `t${i}`, bpm: 120 }),
    );
    render(<TrackTimeline tracks={many} />);
    await userEvent.click(screen.getByRole("button", { name: "Show anyway" }));
    expect(screen.getByTestId("timeline-bars")).toBeInTheDocument();
  });

  it("shows by default at exactly the threshold", () => {
    const many = Array.from({ length: LARGE_PLAYLIST_THRESHOLD }, (_, i) =>
      track({ id: `t${i}`, bpm: 120 }),
    );
    render(<TrackTimeline tracks={many} />);
    expect(screen.getByTestId("timeline-bars")).toBeInTheDocument();
  });

  it("selects the track behind a bar", async () => {
    const onSelectTrack = vi.fn();
    render(<TrackTimeline tracks={SET} onSelectTrack={onSelectTrack} />);
    await userEvent.click(
      screen.getByRole("button", { name: "Track b — 128.0 BPM ↑" }),
    );
    expect(onSelectTrack).toHaveBeenCalledWith("b");
  });

  it("names a track with no value rather than showing a silent gap", () => {
    render(
      <TrackTimeline
        tracks={[track({ id: "a", bpm: 124 }), track({ id: "b" })]}
      />,
    );
    expect(
      screen.getByRole("button", { name: "Track b — no bpm" }),
    ).toBeInTheDocument();
  });
});
