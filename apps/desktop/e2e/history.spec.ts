import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ libraryPath }) => {
      let savedPath: string | null = null;

      const tracks = [
        {
          id: "1",
          title: "Alpha",
          artist: "A",
          album: null,
          genre: "House",
          musical_key: "8A",
          bpm: 128,
          duration_secs: 300,
          rating: 3,
          comment: null,
          folder_path: "/music/a.mp3",
          analysis_data_path: null,
          file_type: 1,
          sample_rate: null,
          bit_rate: 320,
          release_year: 2020,
          dj_play_count: 1,
          energy: null,
        },
      ];

      /** Stand-in for the Rust snapshot store. The idempotency and ledger
       *  semantics are covered by the Rust tests; this proves the view
       *  round-trips and reports honestly. */
      let sets: Array<Record<string, unknown>> = [];
      let deleted: string[] = [];
      const snapshot = [
        {
          id: "ht1",
          seq: 1,
          content_id: "1",
          title: "Alpha",
          artist: "A",
          album: null,
          genre: "House",
          musical_key: "8A",
          bpm: 128,
          duration_secs: 300,
          folder_path: "/music/a.mp3",
        },
        {
          id: "ht2",
          seq: 2,
          content_id: "gone",
          title: "Vinyl Only",
          artist: "B",
          album: null,
          genre: null,
          musical_key: null,
          bpm: null,
          duration_secs: null,
          folder_path: "/music/missing.mp3",
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
              return tracks.length;
            case "set_library_path":
              savedPath = String(args.path);
              return null;
            case "list_tracks":
              return tracks;
            case "get_track_cues":
            case "list_playlists":
            case "list_conversations":
            case "list_changes":
            case "list_smartlists":
            case "list_undo_runs":
            case "list_favourite_playlists":
              return [];
            case "get_api_key":
              return null;

            case "import_history": {
              const source = "h1";
              if (deleted.includes(source)) {
                return { imported: 0, already_known: 0, previously_deleted: 1 };
              }
              if (sets.some((s) => s.source_id === source)) {
                return { imported: 0, already_known: 1, previously_deleted: 0 };
              }
              sets = [
                {
                  id: "s1",
                  source_id: source,
                  name: "2026-05-01 Basement",
                  played_at: "2026-05-01T22:00:00Z",
                  rating: null,
                  location: null,
                  track_count: snapshot.length,
                },
              ];
              return { imported: 1, already_known: 0, previously_deleted: 0 };
            }
            case "list_history_sets":
              return sets;
            case "history_set_tracks":
              return snapshot;
            case "set_history_metadata":
              sets = sets.map((s) =>
                s.id === args.setId
                  ? { ...s, rating: args.rating, location: args.location }
                  : s,
              );
              return null;
            case "delete_history_set": {
              const target = sets.find((s) => s.id === args.setId);
              if (target) deleted.push(String(target.source_id));
              sets = sets.filter((s) => s.id !== args.setId);
              return true;
            }
            case "remove_history_track":
              return true;
            case "preview_history_as_playlist":
              return {
                matches: [
                  {
                    history_track_id: "ht1",
                    title: "Alpha",
                    artist: "A",
                    track_id: "1",
                    kind: "content_id",
                  },
                  {
                    history_track_id: "ht2",
                    title: "Vinyl Only",
                    artist: "B",
                    track_id: null,
                    kind: "none",
                  },
                ],
                matched: 1,
                unmatched: 1,
              };
            case "save_history_as_playlist":
              return ["c1", "c2"];

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

async function openHistory(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: "History" }).click();
}

test("import a session, then see the snapshot of what was played", async ({
  page,
}) => {
  await openHistory(page);

  await expect(page.getByTestId("history-empty")).toContainText(
    "running it again never duplicates them",
  );

  await page.getByRole("button", { name: "Import" }).click();
  await expect(page.getByText("1 imported")).toBeVisible();

  await page.getByText("2026-05-01 Basement").click();
  const list = page.getByTestId("history-tracks");
  await expect(list).toContainText("Alpha");
  // The snapshot survives the track leaving the library — the point of it.
  await expect(list).toContainText("Vinyl Only");
  await expect(
    page.getByText(/Editing them since has not changed this record/),
  ).toBeVisible();
});

test("importing twice does not duplicate a session", async ({ page }) => {
  await openHistory(page);
  await page.getByRole("button", { name: "Import" }).click();
  await expect(page.getByText("1 imported")).toBeVisible();

  await page.getByRole("button", { name: "Import" }).click();
  await expect(page.getByText(/already known/)).toBeVisible();
  await expect(page.getByTestId("history-sets").getByRole("listitem")).toHaveCount(1);
});

test("a deleted session stays deleted across a re-import", async ({ page }) => {
  await openHistory(page);
  await page.getByRole("button", { name: "Import" }).click();
  await page.getByText("2026-05-01 Basement").click();

  await page.getByRole("button", { name: "Delete set" }).click();
  await expect(
    page.getByText(/importing again will not bring it back/),
  ).toBeVisible();
  await page.getByRole("button", { name: "Delete set" }).last().click();

  // Re-import: skipped, and the reason is on screen.
  await page.getByRole("button", { name: "Import" }).click();
  await expect(page.getByText(/skipped \(deleted before\)/)).toBeVisible();
  await expect(page.getByTestId("history-empty")).toBeVisible();
});

test("saving as a playlist names what it could not find", async ({ page }) => {
  await openHistory(page);
  await page.getByRole("button", { name: "Import" }).click();
  await page.getByText("2026-05-01 Basement").click();

  await page.getByRole("button", { name: "Save as playlist" }).click();
  const report = page.getByTestId("history-match-report");
  await expect(report).toContainText("1 of 2 track(s) are still in the library");
  await expect(report).toContainText("Vinyl Only — not in the library any more");

  await page.getByRole("button", { name: "Stage playlist" }).click();
  await expect(page.getByText(/Staged “2026-05-01 Basement” with 1 track/)).toBeVisible();
});
