import { expect, test } from "@playwright/test";

/**
 * The play queue, end to end.
 *
 * Per `docs/lexicon/05-cues-player.md §Music player`. What matters here is the
 * wiring: right-click queues, the panel shows the queue with a marker on what
 * is playing, and Clear leaves the current track alone rather than stopping
 * the music.
 */

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ libraryPath }) => {
    let savedPath: string | null = null;
    let played: string[] = [];

    const tracks = [
      {
        id: "1",
        title: "Dark Matter",
        artist: "Surgeon",
        album: null,
        genre: "Techno",
        musical_key: "8A",
        bpm: 140,
        duration_secs: 360,
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
        title: "Acid Rain",
        artist: "Aphex Twin",
        album: null,
        genre: "Ambient",
        musical_key: "11B",
        bpm: 130,
        duration_secs: 240,
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
          case "search_has_operators":
            return false;
          case "play_track":
            played = [...played, String(args.path)];
            return null;
          case "get_playback_status":
            return { is_playing: true, time: 0, duration: 360 };
          case "list_undo_runs":
          case "list_playlists":
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
  await expect(page.getByText("Dark Matter")).toBeVisible();
}

test("right-click queues a track and the panel opens showing it", async ({
  page,
}) => {
  await openLibrary(page);

  await page.getByText("Dark Matter").first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Add to queue" }).click();

  const queue = page.getByRole("complementary", { name: "Play queue" });
  await expect(queue).toBeVisible();
  await expect(page.getByTestId("queue-list")).toHaveText(
    /Surgeon — Dark Matter/,
  );
});

test("the queue is empty until something is added, and says how", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "Open play queue" }).click();
  await expect(page.getByText(/Right-click a track/)).toBeVisible();
});

test("Clear keeps the playing track rather than stopping the music", async ({
  page,
}) => {
  await openLibrary(page);

  // Queue both, then play the first.
  await page.getByText("Dark Matter").first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Add to queue" }).click();
  await page.getByText("Acid Rain").first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Add to queue" }).click();

  const list = page.getByTestId("queue-list");
  await expect(list).toHaveText(/Acid Rain/);

  await page
    .getByRole("button", { name: "Play Surgeon — Dark Matter" })
    .click();
  await expect(list.locator('[aria-current="true"]')).toHaveText(
    /Dark Matter/,
  );

  await page.getByRole("button", { name: /Clear/ }).click();
  await expect(list).toHaveText(/Dark Matter/);
  await expect(list).not.toHaveText(/Acid Rain/);
});
