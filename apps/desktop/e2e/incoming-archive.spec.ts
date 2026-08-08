import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ libraryPath }) => {
    let savedPath: string | null = null;

    const incomingTrack = {
      id: "inc-1",
      title: "Fresh Inbound",
      artist: "Newcomer",
      album: null,
      genre: null,
      musical_key: null,
      bpm: 128,
      duration_secs: 200,
      rating: null,
      comment: null,
      folder_path: "/music/fresh.mp3",
      analysis_data_path: null,
      file_type: 1,
      sample_rate: 44100,
      bit_rate: 320,
      release_year: null,
      dj_play_count: null,
      label: null,
      remixer: null,
      mix: null,
      color: null,
      date_added: null,
    };

    const archivedTrack = {
      id: "arc-1",
      title: "Old Banger",
      artist: "Yesteryear",
      album: null,
      genre: null,
      musical_key: null,
      bpm: 124,
      duration_secs: 220,
      rating: null,
      comment: null,
      folder_path: "/music/old.mp3",
      analysis_data_path: null,
      file_type: 1,
      sample_rate: 44100,
      bit_rate: 320,
      release_year: null,
      dj_play_count: null,
      label: null,
      remixer: null,
      mix: null,
      color: null,
      date_added: null,
    };

    let incoming: typeof incomingTrack[] = [incomingTrack];
    let archived: typeof archivedTrack[] = [archivedTrack];

    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: Record<string, unknown>) => {
        switch (cmd) {
          case "plugin:dialog|open":
            return libraryPath;
          case "plugin:dialog|save":
            return "/tmp/rekordagent-export.xml";
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
            return [];
          case "get_track_cues":
            return [];
          case "list_playlists":
            return [];
          case "list_conversations":
            return [];
          case "get_api_key":
            return null;
          case "list_genres":
            return [];
          case "list_artists":
            return [];
          case "list_tracks_with_cues":
            return [];
          case "list_tracks_in_any_playlist":
            return [];
          case "list_tracks_with_missing_files":
            return [];
          case "list_archived_track_ids":
            return archived.map((t) => t.id);
          case "list_changes":
            return [];
          case "list_tracks_with_audio_features":
            return [];
          case "list_smart_fix_proposals":
            return [];
          case "list_incoming_tracks":
            return incoming;
          case "list_archived_tracks":
            return archived;
          case "clear_incoming":
            incoming = [];
            return null;
          case "archive_tracks": {
            const ids = (args.trackIds as string[]) ?? [];
            const moved = incoming.filter((t) => ids.includes(t.id));
            incoming = incoming.filter((t) => !ids.includes(t.id));
            archived = [...archived, ...moved];
            return null;
          }
          case "unarchive_tracks": {
            const ids = (args.trackIds as string[]) ?? [];
            archived = archived.filter((t) => !ids.includes(t.id));
            return null;
          }
          case "list_tags":
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
}

test("incoming: shows count badge, clears inbox, count updates", async ({ page }) => {
  await openLibrary(page);

  await page.getByRole("button", { name: "Incoming", exact: true }).click();

  // Count badge / header text reflects one incoming track.
  await expect(page.getByText(/1 new track/)).toBeVisible();
  await expect(page.getByText("Fresh Inbound")).toBeVisible();

  // Primary action: "Mark all reviewed" → confirm dialog → Clear.
  await page.getByRole("button", { name: /Mark all reviewed/ }).click();
  await page.getByRole("button", { name: "Clear" }).click();

  // After clearing, the header reads "0 new tracks" and the row is gone.
  await expect(page.getByText(/0 new tracks/)).toBeVisible();
  await expect(page.getByText("Fresh Inbound")).toHaveCount(0);
});

test("archive: lists track, unarchive removes it from view", async ({ page }) => {
  await openLibrary(page);

  await page.getByRole("button", { name: "Archive", exact: true }).click();

  await expect(page.getByText(/1 archived track/)).toBeVisible();
  await expect(page.getByText("Old Banger")).toBeVisible();

  // Select the row by clicking on the title text (TrackTable rows are clickable).
  await page.getByText("Old Banger").first().click();

  // Unarchive button shows a selection count; click it.
  await page.getByRole("button", { name: /Unarchive/ }).click();

  // Track disappears and header updates.
  await expect(page.getByText(/0 archived tracks/)).toBeVisible();
  await expect(page.getByText("Old Banger")).toHaveCount(0);
});
