import { expect, test } from "@playwright/test";

/**
 * Find Popup, end to end.
 *
 * Per `docs/lexicon/00-overview.md §Find Popup`. What matters is that `Cmd+F`
 * reaches one box searching playlists, smartlists and tracks together, and that
 * the per-result actions do the right different things — `Enter` opens a
 * container but plays a track.
 */

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ libraryPath }) => {
    let savedPath: string | null = null;

    const tracks = [
      {
        id: "1",
        title: "Acid Rain",
        artist: "Aphex Twin",
        album: null,
        genre: "Ambient",
        musical_key: "11B",
        bpm: 130,
        duration_secs: 240,
        rating: null,
        comment: null,
        folder_path: "/music/a.mp3",
        analysis_data_path: null,
        file_type: 1,
        sample_rate: 44100,
        bit_rate: 320,
        release_year: null,
        dj_play_count: null,
      },
      {
        id: "2",
        title: "Braindance",
        artist: "Surgeon",
        album: null,
        genre: "Techno",
        musical_key: "8A",
        bpm: 140,
        duration_secs: 360,
        rating: null,
        comment: null,
        folder_path: "/music/b.mp3",
        analysis_data_path: null,
        file_type: 1,
        sample_rate: 44100,
        bit_rate: 320,
        release_year: null,
        dj_play_count: null,
      },
    ];

    const playlists = [
      {
        id: "p1",
        name: "Rainy Warmup",
        kind: "Playlist",
        parent_id: null,
        seq: 1,
      },
    ];

    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: Record<string, unknown>) => {
        switch (cmd) {
          case "plugin:dialog|open":
            return libraryPath;
          case "get_library_path":
            return savedPath;
          case "get_theme":
            return "dark";
          case "validate_library_path":
            return 2;
          case "set_library_path":
            savedPath = String(args.path);
            return null;
          case "list_tracks":
            return tracks;
          case "list_playlists":
            return playlists;
          case "list_smartlists":
            return [];
          case "search_has_operators":
            return false;
          case "play_track":
            return null;
          case "get_playback_status":
            return { is_playing: true, time: 0, duration: 240 };
          case "get_playlist":
            return { playlist: playlists[0], tracks };
          case "list_undo_runs":
          case "list_conversations":
          case "list_genres":
          case "list_artists":
          case "list_tracks_with_cues":
          case "list_tracks_in_any_playlist":
          case "list_tracks_with_missing_files":
          case "list_archived_track_ids":
          case "list_tracks_with_audio_features":
          case "list_smart_fix_proposals":
          case "list_changes":
          case "list_tags":
          case "get_track_cues":
            return [];
          case "list_track_tags_map":
            return {};
          default:
            return null;
        }
      },
      transformCallback: () => 1,
      unregisterCallback: () => {},
      convertFileSrc: (path: string) => path,
      metadata: { currentWindow: { label: "main" } },
    };
  }, { libraryPath: LIBRARY_PATH });
});

async function openLibrary(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await expect(page.getByText("Acid Rain")).toBeVisible();
}

test("Cmd+F opens one box over playlists and tracks", async ({ page }) => {
  await openLibrary(page);
  await page.keyboard.press("ControlOrMeta+f");

  const box = page.getByLabel("Find in library");
  await expect(box).toBeFocused();

  await box.fill("rain");
  const results = page.getByTestId("find-results");
  await expect(results).toContainText("Rainy Warmup");
  await expect(results).toContainText("Acid Rain");
});

test("Enter opens a playlist rather than playing it", async ({ page }) => {
  await openLibrary(page);
  await page.keyboard.press("ControlOrMeta+f");
  await page.getByLabel("Find in library").fill("rainy");
  await page.keyboard.press("Enter");

  // The popup closes and the playlist view is showing.
  await expect(page.getByLabel("Find in library")).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Rainy Warmup" })).toBeVisible();
});

test("a track can be queued straight from the results", async ({ page }) => {
  await openLibrary(page);
  await page.keyboard.press("ControlOrMeta+f");
  await page.getByLabel("Find in library").fill("acid");

  await page.getByRole("button", { name: "Add Acid Rain to queue" }).click();
  await expect(page.getByTestId("queue-list")).toContainText(
    "Aphex Twin — Acid Rain",
  );
});

test("Escape closes without acting", async ({ page }) => {
  await openLibrary(page);
  await page.keyboard.press("ControlOrMeta+f");
  await page.getByLabel("Find in library").fill("rain");
  await page.keyboard.press("Escape");
  await expect(page.getByLabel("Find in library")).toHaveCount(0);
});
