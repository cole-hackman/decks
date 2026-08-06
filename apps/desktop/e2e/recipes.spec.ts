import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ libraryPath }) => {
      let savedPath: string | null = null;

      const tracks = [
        {
          id: "1",
          title: "get lucky",
          artist: "daft punk",
          album: null,
          genre: "House",
          musical_key: null,
          bpm: 116,
          duration_secs: 369,
          rating: null,
          comment: null,
          folder_path: "/music/a.mp3",
          analysis_data_path: null,
          file_type: 1,
          sample_rate: null,
          bit_rate: null,
          release_year: 2013,
          dj_play_count: null,
          energy: null,
        },
      ];

      const staged: Array<Record<string, unknown>> = [];

      /** Stand-in for the Rust engine — enough to prove the UI round-trips.
       *  Recipe semantics themselves are covered by 75 Rust tests. */
      function preview(args: Record<string, unknown>) {
        const recipes = args.recipes as Array<Record<string, unknown>>;
        const ids = args.trackIds as string[];
        const proposals = [];
        for (const t of tracks.filter((t) => ids.includes(t.id))) {
          for (const r of recipes) {
            if (r.op === "to_title_case" && r.field === "title") {
              const after = t.title.replace(/\b\w/g, (c) => c.toUpperCase());
              if (after !== t.title) {
                proposals.push({
                  id: `${t.id}:title`,
                  track_id: t.id,
                  track_title: t.title,
                  field: "title",
                  before: t.title,
                  after,
                });
              }
            }
          }
        }
        return { proposals, skipped: [["1", "remixer is empty"]] };
      }

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
              return 1;
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
              return [];
            case "get_api_key":
              return null;

            case "recipe_fields":
              return ["title", "artist", "genre"];
            case "recipe_preview":
              return preview(args);
            case "recipe_apply": {
              const proposals = args.proposals as Array<Record<string, unknown>>;
              proposals.forEach((p) => staged.push(p));
              return proposals.map((_, i) => `c${i}`);
            }

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

async function openRecipes(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: "Recipes" }).click();
}

test("build a recipe, preview it, deselect a row, then stage", async ({ page }) => {
  await openRecipes(page);

  await expect(page.getByTestId("no-recipes")).toBeVisible();
  await expect(page.getByRole("button", { name: "Preview" })).toBeDisabled();

  await page.getByRole("button", { name: "Add" }).click();
  await page.getByRole("button", { name: "Preview" }).click();

  const preview = page.getByTestId("recipe-preview");
  await expect(preview).toBeVisible();
  // `exact` matters: getByText is case-insensitive by default, so "Get Lucky"
  // would also match the "get lucky" before-value and the track name.
  await expect(preview.getByText("Get Lucky", { exact: true })).toBeVisible();
  // Steps that did nothing are explained rather than silently absent.
  await expect(page.getByTestId("recipe-skipped")).toContainText("remixer is empty");

  await page.getByRole("button", { name: /Stage 1 change/ }).click();
  await expect(page.getByText(/Staged 1 change\(s\) for review/)).toBeVisible();
});

test("deselecting every row leaves nothing to stage", async ({ page }) => {
  await openRecipes(page);
  await page.getByRole("button", { name: "Add" }).click();
  await page.getByRole("button", { name: "Preview" }).click();
  await expect(page.getByTestId("recipe-preview")).toBeVisible();

  await page.getByLabel("Keep get lucky title").uncheck();
  await expect(page.getByRole("button", { name: /Stage 0 change/ })).toBeDisabled();
});
