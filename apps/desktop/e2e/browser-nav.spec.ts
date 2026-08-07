import { expect, test } from "@playwright/test";

/**
 * Spreadsheet keyboard navigation, end to end.
 *
 * Per `docs/lexicon/02-library.md §Browser`. The point is that the browser is
 * drivable without the mouse: a cell cursor you can walk, and inline editing
 * that still goes through the staged-change pipeline rather than around it.
 */

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ libraryPath }) => {
    let savedPath: string | null = null;
    let stagedChanges: Array<Record<string, unknown>> = [];

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
          case "multi_edit_apply": {
            const edits = args.edits as Array<{
              field: string;
              value: string | null;
            }>;
            const ids = (args.trackIds as string[]) ?? [];
            const staged = edits.map((edit, i) => {
              const id = `change-${stagedChanges.length + i + 1}`;
              return {
                id,
                library_path: libraryPath,
                kind: "TrackMetadataEdit",
                target_id: ids[0],
                field: edit.field,
                old_value: "Dark Matter",
                new_value: edit.value,
                reason: "Manual edit",
                confidence: 1.0,
                status: "Proposed",
                created_at: 1,
                updated_at: 1,
              };
            });
            stagedChanges = [...stagedChanges, ...staged];
            return staged.map((s) => s.id);
          }
          case "list_changes":
            return stagedChanges;
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

test("the cell cursor walks the grid with the arrow keys", async ({ page }) => {
  await openLibrary(page);
  const grid = page.getByRole("grid");
  await grid.focus();

  const cursor = page.locator('[role="gridcell"][aria-selected="true"]');
  await expect(cursor).toHaveText("Dark Matter");

  await page.keyboard.press("ArrowRight");
  await expect(cursor).toHaveText("Surgeon");

  await page.keyboard.press("ArrowDown");
  await expect(cursor).toHaveText("Aphex Twin");

  // Home returns to the first column of the same row, not the first row.
  await page.keyboard.press("Home");
  await expect(cursor).toHaveText("Acid Rain");
});

test("typing over a cell stages a change for review rather than writing it", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("grid").focus();

  // A printable key opens the editor seeded with what was typed.
  await page.keyboard.press("N");
  const editor = page.getByLabel("Edit Title");
  await expect(editor).toHaveValue("N");

  await editor.fill("Night Drive");
  await page.keyboard.press("Enter");

  // Nothing was written — it is a proposal, in the review panel like any other.
  await page.getByRole("button", { name: "Changes" }).click();
  await expect(page.getByText("Night Drive", { exact: true })).toBeVisible();
});

test("Escape abandons an edit without staging anything", async ({ page }) => {
  await openLibrary(page);
  await page.getByRole("grid").focus();

  await page.keyboard.press("Enter");
  const editor = page.getByLabel("Edit Title");
  await editor.fill("Discarded");
  await page.keyboard.press("Escape");
  await expect(editor).toHaveCount(0);

  await page.getByRole("button", { name: "Changes" }).click();
  await expect(page.getByText("Discarded")).toHaveCount(0);
});
