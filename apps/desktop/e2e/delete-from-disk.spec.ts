import { expect, test } from "@playwright/test";

/**
 * Delete from disk, end to end.
 *
 * The point of the flow is that the destructive step is never one click and
 * never silent: a file must be inside a configured music folder, the dialog
 * previews exactly what will and will not move, and what does move goes to a
 * restorable batch rather than to `unlink`. This walks the whole of that,
 * including the fail-closed state before any music folder exists.
 */

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ libraryPath }) => {
    let savedPath: string | null = null;
    let roots: string[] = [];

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

    let archived = [archivedTrack];
    let batches: Array<Record<string, unknown>> = [];

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
          case "list_archived_tracks":
            return archived;
          case "list_archived_track_ids":
            return archived.map((t) => t.id);

          // ── The feature under test ──
          case "music_roots":
            return roots;
          case "set_music_roots":
            roots = (args.roots as string[]) ?? [];
            return null;
          case "suggest_music_roots":
            return [{ path: "/music", track_count: 1 }];
          case "plan_delete_from_disk": {
            const request = args.request as {
              track_ids: string[];
              allow_playlist_members: boolean;
            };
            const labels = { "arc-1": "Yesteryear — Old Banger" };
            if (roots.length === 0) {
              return {
                deletable: [],
                refused: request.track_ids.map((id) => ({
                  track_id: id,
                  path: "/music/old.mp3",
                  reason: { kind: "outside_music_roots" },
                  message: "Outside every folder you have marked as music.",
                })),
                total_bytes: 0,
                labels,
                no_roots_configured: true,
              };
            }
            return {
              deletable: request.track_ids.map((id) => ({
                track_id: id,
                source: "/music/old.mp3",
                bytes: 5_242_880,
              })),
              refused: [],
              total_bytes: 5_242_880 * request.track_ids.length,
              labels,
              no_roots_configured: false,
            };
          }
          case "delete_from_disk": {
            const request = args.request as { track_ids: string[]; reason: string };
            const manifest = {
              batch_id: "2025-08-06T14-22-01",
              created_at: 1_754_490_121,
              library_path: libraryPath,
              reason: request.reason,
              entries: request.track_ids.map((id) => ({
                track_id: id,
                original_path: "/music/old.mp3",
                stored_as: "old.mp3",
                bytes: 5_242_880,
              })),
            };
            batches = [
              {
                manifest,
                total_bytes: 5_242_880 * request.track_ids.length,
                file_count: request.track_ids.length,
              },
            ];
            archived = archived.filter((t) => !request.track_ids.includes(t.id));
            return { manifest, failed: [] };
          }
          case "list_deleted_batches":
            return batches;
          case "restore_deleted_batch":
            batches = [];
            return {
              batch_id: String(args.batchId),
              results: [
                {
                  track_id: "arc-1",
                  original_path: "/music/old.mp3",
                  outcome: { outcome: "restored", path: "/music/old.mp3" },
                },
              ],
              restored: 1,
              batch_emptied: true,
            };
          case "purge_deleted_batch":
            batches = [];
            return 5_242_880;

          case "list_tracks":
          case "list_playlists":
          case "list_conversations":
          case "list_genres":
          case "list_artists":
          case "list_tracks_with_cues":
          case "list_tracks_in_any_playlist":
          case "list_tracks_with_missing_files":
          case "list_changes":
          case "list_tracks_with_audio_features":
          case "list_smart_fix_proposals":
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
}

test("refuses to delete anything until a music folder is configured", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "Archive", exact: true }).click();
  await page.getByText("Old Banger").first().click();
  await page.getByRole("button", { name: "Delete from disk" }).click();

  await expect(page.getByText(/No music folders are set up yet/)).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Delete 0 from disk/ }),
  ).toBeDisabled();
});

test("configure a folder, delete, then restore the batch", async ({ page }) => {
  await openLibrary(page);

  // 1. Turn the feature on by saying where the music lives.
  await page.getByRole("button", { name: "Settings" }).click();
  await expect(page.getByTestId("no-music-roots")).toBeVisible();
  await page.getByRole("button", { name: "Suggest from library" }).click();
  await page.getByTestId("root-suggestions").getByRole("button", { name: "Add" }).click();
  await expect(page.getByTestId("no-music-roots")).toHaveCount(0);
  await expect(page.getByText("/music", { exact: true })).toBeVisible();

  // 2. Delete a track's audio from the Archive, with the preview in between.
  await page.getByRole("button", { name: "Archive", exact: true }).click();
  await page.getByText("Old Banger").first().click();
  await page.getByRole("button", { name: "Delete from disk" }).click();

  await expect(page.getByText("Yesteryear — Old Banger")).toBeVisible();
  await expect(page.getByText(/Restorable from Settings/)).toBeVisible();

  // Two clicks, not one.
  await page.getByRole("button", { name: /Delete 1 from disk/ }).click();
  await page.getByRole("button", { name: /Yes, move 1 file/ }).click();
  await expect(
    page.getByText(/Moved 1 file\(s\) to the deleted-audio folder/),
  ).toBeVisible();

  // 3. The batch is listed, and restoring puts it back.
  await page.getByRole("button", { name: "Settings" }).click();
  await expect(page.getByText(/1 file · 5\.0 MB/)).toBeVisible();
  await expect(page.getByText(/Archive cleanup/)).toBeVisible();

  await page.getByRole("button", { name: "Restore", exact: true }).click();
  await expect(page.getByText(/Restored 1 file/)).toBeVisible();
  await expect(page.getByText(/Nothing has been deleted from disk/)).toBeVisible();
});

test("emptying a batch asks first and says it cannot be undone", async ({
  page,
}) => {
  await openLibrary(page);

  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("button", { name: "Suggest from library" }).click();
  await page.getByTestId("root-suggestions").getByRole("button", { name: "Add" }).click();

  await page.getByRole("button", { name: "Archive", exact: true }).click();
  await page.getByText("Old Banger").first().click();
  await page.getByRole("button", { name: "Delete from disk" }).click();
  await page.getByRole("button", { name: /Delete 1 from disk/ }).click();
  await page.getByRole("button", { name: /Yes, move 1 file/ }).click();

  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("button", { name: /Empty/ }).click();
  await expect(page.getByText(/cannot be undone/)).toBeVisible();

  await page.getByRole("button", { name: "Delete permanently" }).click();
  await expect(page.getByText(/Freed 5\.0 MB/)).toBeVisible();
  await expect(page.getByText(/Nothing has been deleted from disk/)).toBeVisible();
});
