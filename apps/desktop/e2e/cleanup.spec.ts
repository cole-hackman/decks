import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ libraryPath }) => {
    let savedPath: string | null = null;

    let genres = [
      { genre: "House", count: 12 },
      { genre: "Techno", count: 7 },
      { genre: "Trance", count: 3 },
    ];
    let stagedChanges: Array<Record<string, unknown>> = [];
    const tracks = [
      {
        id: "1",
        title: "Track A",
        artist: "DJ One",
        album: "Album",
        genre: "House",
        musical_key: "8A",
        bpm: 128,
        duration_secs: 360,
        rating: 0,
        comment: "",
        folder_path: "/music/a.mp3",
      },
    ];
    const playlists: unknown[] = [];

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
            return tracks;
          case "get_track_cues":
            return [];
          case "list_playlists":
            return playlists;
          case "list_conversations":
            return [];
          case "get_api_key":
            return null;
          case "list_genres":
            return genres;
          case "list_artists":
            return [];
          case "rename_genre": {
            const oldGenre = String(args.oldGenre);
            const newGenre = String(args.newGenre);
            const id = `change-${stagedChanges.length + 1}`;
            stagedChanges.push({
              id,
              library_path: libraryPath,
              kind: "TrackMetadataEdit",
              target_id: "1",
              field: "genre",
              old_value: oldGenre,
              new_value: newGenre,
              reason: `Rename genre ${oldGenre} -> ${newGenre}`,
              confidence: 1.0,
              status: "Proposed",
              created_at: 1,
              updated_at: 1,
            });
            genres = genres.map((g) =>
              g.genre === oldGenre ? { genre: newGenre, count: g.count } : g,
            );
            return { affected_tracks: 12, staged_change_ids: [id] };
          }
          case "list_changes":
            return stagedChanges;
          case "accept_change":
            stagedChanges = stagedChanges.map((change) =>
              change.id === args.id ? { ...change, status: "Accepted" } : change,
            );
            return stagedChanges.find((c) => c.id === args.id);
          case "reject_change":
            stagedChanges = stagedChanges.map((change) =>
              change.id === args.id ? { ...change, status: "Rejected" } : change,
            );
            return stagedChanges.find((c) => c.id === args.id);
          case "export_accepted_changes":
            stagedChanges = stagedChanges.map((change) =>
              change.status === "Accepted"
                ? { ...change, status: "Exported" }
                : change,
            );
            return { output_path: "/tmp/rekordagent-export.xml", exported_count: 1 };
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

test("genre cleanup: rename stages a TrackMetadataEdit and exports", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();

  await page.getByRole("button", { name: "Genre Cleanup" }).click();

  // The three genre chips render with counts.
  await expect(page.getByRole("button", { name: /^House\s*12$/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /^Techno\s*7$/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /^Trance\s*3$/ })).toBeVisible();

  // Select House and rename it.
  await page.getByRole("button", { name: /^House\s*12$/ }).click();
  await page.getByRole("button", { name: "Rename", exact: true }).click();

  const dialogInput = page.getByPlaceholder("New genre name");
  await dialogInput.fill("Deep House");
  await page.getByRole("button", { name: "Stage rename" }).click();

  // Toast should confirm staging.
  await expect(page.getByText(/Staged 12 change/)).toBeVisible();

  // Go to Changes view; staged row is visible.
  await page.getByRole("button", { name: "Changes" }).click();
  await expect(page.getByText("Deep House", { exact: true })).toBeVisible();
  await expect(page.getByText("House", { exact: true })).toBeVisible();

  // Accept the change.
  await page.getByRole("button", { name: "Accept", exact: true }).click();

  // Export and verify the IPC was called.
  await page.getByRole("button", { name: "Export XML" }).click();
  await expect(page.getByText(/Exported 1 changes/)).toBeVisible();
});
