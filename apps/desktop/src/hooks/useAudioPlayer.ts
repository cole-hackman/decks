import { useState, useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  playTrack,
  pauseAudio,
  resumeAudio,
  getPlaybackStatus,
  seekAudio,
} from "../ipc";
import type { Track } from "../types";

const POLL_MS = 250;

export function useAudioPlayer(selectedTrack: Track | null) {
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentPath, setCurrentPath] = useState<string | null>(null);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  /**
   * Timestamp of the last natural end-of-track, or `null`.
   *
   * A monotonically increasing value rather than a boolean, so a consumer
   * (the play queue) can tell two consecutive ends apart — a boolean would
   * stay `true` and the second track would never advance.
   */
  const [endedAt, setEndedAt] = useState<number | null>(null);
  const stoppedRef = useRef(false);

  const play = useCallback(async (track: Track) => {
    if (!track.folder_path) return;
    try {
      await playTrack(track.folder_path);
      setIsPlaying(true);
      setCurrentPath(track.folder_path);
      setCurrentTime(0);
      stoppedRef.current = false;
    } catch (e) {
      console.error("play error", e);
    }
  }, []);

  const pause = useCallback(async () => {
    try {
      await pauseAudio();
      setIsPlaying(false);
    } catch (e) {
      console.error("pause error", e);
    }
  }, []);

  const resume = useCallback(async () => {
    try {
      await resumeAudio();
      setIsPlaying(true);
    } catch (e) {
      console.error("resume error", e);
    }
  }, []);

  const toggleCurrent = useCallback(async () => {
    if (!selectedTrack) return;
    if (currentPath !== selectedTrack.folder_path) {
      await play(selectedTrack);
    } else if (isPlaying) {
      await pause();
    } else {
      await resume();
    }
  }, [selectedTrack, currentPath, isPlaying, play, pause, resume]);

  const seek = useCallback(async (targetSecs: number) => {
    try {
      await seekAudio(targetSecs);
      setCurrentTime(targetSecs);
    } catch (e) {
      console.error("seek error", e);
    }
  }, []);

  // Poll backend for playback time/duration while a track is loaded.
  useEffect(() => {
    if (!currentPath) return;
    let cancelled = false;
    const tick = async () => {
      try {
        const status = await getPlaybackStatus();
        if (cancelled) return;
        setIsPlaying(status.is_playing);
        setCurrentTime(status.time);
        setDuration(status.duration);
      } catch {
        // Tauri may not be ready or command may fail in tests — ignore.
      }
    };
    void tick();
    const id = window.setInterval(() => void tick(), POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [currentPath]);

  // Backend emits `playback-ended` when the source drains naturally.
  useEffect(() => {
    const unlistenPromise = listen("playback-ended", () => {
      setIsPlaying(false);
      setEndedAt(Date.now());
    });
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, []);

  const isCurrentTrack = useCallback(
    (track: Track) =>
      track.folder_path !== null && track.folder_path === currentPath,
    [currentPath],
  );

  return {
    isPlaying,
    currentPath,
    currentTime,
    duration,
    endedAt,
    play,
    pause,
    resume,
    toggleCurrent,
    seek,
    isCurrentTrack,
  };
}
