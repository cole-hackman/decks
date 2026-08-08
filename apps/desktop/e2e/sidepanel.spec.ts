import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ libraryPath }) => {
      let savedPath: string | null = null;

      const base = {
        album: null,
        genre: "House",
        duration_secs: 300,
        rating: 3,
        comment: null,
        analysis_data_path: null,
        file_type: 1,
        sample_rate: null,
        bit_rate: 320,
        release_year: 2020,
        dj_play_count: 0,
        label: null,
        remixer: null,
        mix: null,
        color: null,
        date_added: null,
        energy: null,
      };

      const tracks = [
        { ...base, id: "1", title: "Warm One", artist: "A", bpm: 122, musical_key: "8A", folder_path: "/a.mp3" },
        { ...base, id: "2", title: "Peak One", artist: "B", bpm: 130, musical_key: "9A", folder_path: "/b.mp3" },
      ];

      const playlists = [
        { id: "p1", name: "Warmup", parent_id: null, seq: 1, kind: "Playlist" },
        { id: "p2", name: "Peak", parent_id: null, seq: 2, kind: "Playlist" },
      ];

      const members: Record<string, string[]> = { p1: ["1"], p2: ["2"] };

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
              return tracks.length;
            case "set_library_path":
              savedPath = String(args.path);
              return null;
            case "list_tracks":
              return tracks;
            case "list_playlists":
              return playlists;
            case "get_playlist": {
              const id = String(args.playlistId ?? args.id);
              return {
                playlist: playlists.find((p) => p.id === id),
                tracks: (members[id] ?? []).map(
                  (tid) => tracks.find((t) => t.id === tid)!,
                ),
              };
            }
            case "get_track_cues":
            case "list_conversations":
            case "list_changes":
            case "list_smartlists":
            case "list_undo_runs":
            case "list_favourite_playlists":
              return [];
            case "get_api_key":
              return null;
            default:
              return null;
          }
        },
        transformCallback: () => 1,
        unregisterCallback: () => {},
        convertFileSrc: (path: string) => path,
        metadata: { currentWindow: { label: "main" } },
      };
    },
    { libraryPath: LIBRARY_PATH },
  );
});

async function openLibrary(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
}

test("the sidepanel opens a second browser beside the main one", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "Playlists" }).click();

  // One browser to start with.
  await expect(page.getByTestId("playlist-track-list")).toHaveCount(1);

  await page.getByRole("button", { name: "Open sidepanel" }).click();
  await expect(page.getByTestId("playlist-track-list")).toHaveCount(2);

  await page.getByRole("button", { name: "Close sidepanel" }).click();
  await expect(page.getByTestId("playlist-track-list")).toHaveCount(1);
});

test("the two browsers hold different playlists", async ({ page }) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "Playlists" }).click();
  await page.getByRole("button", { name: "Open sidepanel" }).click();

  const panels = page.getByTestId("playlist-track-list");
  await expect(panels).toHaveCount(2);

  // Point the sidepanel at the other playlist; the main browser stays put.
  // A shared selection would make it a mirror rather than a second view.
  await page.getByRole("button", { name: /^Peak/ }).last().click();
  await expect(panels.nth(1)).toContainText("Peak One");
  await expect(panels.nth(0)).toContainText("Warm One");
});

test("the sidepanel opens from the library view too, not just playlists", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "Open sidepanel" }).click();
  await expect(page.getByTestId("playlist-track-list")).toHaveCount(1);
});
