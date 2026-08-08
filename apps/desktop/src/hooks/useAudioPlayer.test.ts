import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { useAudioPlayer } from "./useAudioPlayer";
import type { Track } from "../types";

vi.mock("../ipc", () => ({
  playTrack: vi.fn().mockResolvedValue(undefined),
  pauseAudio: vi.fn().mockResolvedValue(undefined),
  resumeAudio: vi.fn().mockResolvedValue(undefined),
  stopAudio: vi.fn().mockResolvedValue(undefined),
}));

import { playTrack, pauseAudio, resumeAudio } from "../ipc";

const TRACK: Track = {
  id: "1",
  title: "Test",
  artist: null,
  album: null,
  genre: null,
  musical_key: null,
  bpm: null,
  duration_secs: null,
  rating: null,
  comment: null,
  folder_path: "/music/test.mp3",
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

beforeEach(() => {
  vi.clearAllMocks();
});

describe("useAudioPlayer", () => {
  it("starts with isPlaying=false and currentPath=null", () => {
    const { result } = renderHook(() => useAudioPlayer(null));
    expect(result.current.isPlaying).toBe(false);
    expect(result.current.currentPath).toBeNull();
  });

  it("play sets isPlaying=true and currentPath", async () => {
    const { result } = renderHook(() => useAudioPlayer(TRACK));
    await act(() => result.current.play(TRACK));
    expect(vi.mocked(playTrack)).toHaveBeenCalledWith(TRACK.folder_path);
    expect(result.current.isPlaying).toBe(true);
    expect(result.current.currentPath).toBe(TRACK.folder_path);
  });

  it("pause sets isPlaying=false", async () => {
    const { result } = renderHook(() => useAudioPlayer(TRACK));
    await act(() => result.current.play(TRACK));
    await act(() => result.current.pause());
    expect(vi.mocked(pauseAudio)).toHaveBeenCalled();
    expect(result.current.isPlaying).toBe(false);
  });

  it("resume sets isPlaying=true", async () => {
    const { result } = renderHook(() => useAudioPlayer(TRACK));
    await act(() => result.current.play(TRACK));
    await act(() => result.current.pause());
    await act(() => result.current.resume());
    expect(vi.mocked(resumeAudio)).toHaveBeenCalled();
    expect(result.current.isPlaying).toBe(true);
  });

  it("toggleCurrent plays if different track is selected", async () => {
    const { result } = renderHook(() => useAudioPlayer(TRACK));
    await act(() => result.current.toggleCurrent());
    expect(vi.mocked(playTrack)).toHaveBeenCalledWith(TRACK.folder_path);
  });

  it("toggleCurrent pauses if current track is playing", async () => {
    const { result } = renderHook(() => useAudioPlayer(TRACK));
    await act(() => result.current.play(TRACK));
    await act(() => result.current.toggleCurrent());
    expect(vi.mocked(pauseAudio)).toHaveBeenCalled();
  });

  it("toggleCurrent resumes if current track is paused", async () => {
    const { result } = renderHook(() => useAudioPlayer(TRACK));
    await act(() => result.current.play(TRACK));
    await act(() => result.current.pause());
    await act(() => result.current.toggleCurrent());
    expect(vi.mocked(resumeAudio)).toHaveBeenCalled();
  });

  it("toggleCurrent does nothing when no track selected", async () => {
    const { result } = renderHook(() => useAudioPlayer(null));
    await act(() => result.current.toggleCurrent());
    expect(vi.mocked(playTrack)).not.toHaveBeenCalled();
  });

  it("isCurrentTrack returns true for matching path", async () => {
    const { result } = renderHook(() => useAudioPlayer(TRACK));
    await act(() => result.current.play(TRACK));
    expect(result.current.isCurrentTrack(TRACK)).toBe(true);
  });

  it("isCurrentTrack returns false for different path", async () => {
    const { result } = renderHook(() => useAudioPlayer(TRACK));
    await act(() => result.current.play(TRACK));
    const otherTrack = { ...TRACK, folder_path: "/music/other.mp3" };
    expect(result.current.isCurrentTrack(otherTrack)).toBe(false);
  });

  it("play does nothing for track with no folder_path", async () => {
    const noPathTrack = { ...TRACK, folder_path: null };
    const { result } = renderHook(() => useAudioPlayer(noPathTrack));
    await act(() => result.current.play(noPathTrack));
    expect(vi.mocked(playTrack)).not.toHaveBeenCalled();
    expect(result.current.isPlaying).toBe(false);
  });

  // Global space-key binding lives in App.tsx via the shared
  // useKeyboardShortcuts hook (which also excludes buttons/links). The
  // hook itself no longer registers a keydown listener — tested via
  // useKeyboardShortcuts.test.ts instead.
});
