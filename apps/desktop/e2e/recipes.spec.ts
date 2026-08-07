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
              return ["title", "artist", "genre", "comment"];
            case "recipe_preview":
              return preview(args);
            case "tag_recipe_preview": {
              const recipe = args.recipe as Record<string, unknown>;
              if (recipe.op !== "import_from_text") return [];
              // The fixture's comment carries two hashtags.
              return [
                {
                  track_id: "1",
                  track_title: "get lucky",
                  added: ["Techno", "Vocals"],
                  removed: [],
                },
              ];
            }
            case "tag_recipe_apply":
              return {
                tracks_changed: 1,
                tags_added: 2,
                tags_removed: 0,
                tags_created: ["Techno"],
              };
            case "recipe_apply": {
              const proposals = args.proposals as Array<Record<string, unknown>>;
              proposals.forEach((p) => staged.push(p));
              return proposals.map((_, i) => `c${i}`);
            }

            case "cue_recipe_preview": {
              const recipe = args.recipe as Record<string, unknown>;
              const base = { track_id: "1", track_title: "get lucky" };
              // The fixture track has no analysis file, so quantize has no
              // grid to snap to and says so rather than reporting no changes.
              if (recipe.op === "quantize_cues") {
                return [
                  {
                    ...base,
                    edits: [],
                    deletions: [],
                    skipped: "this track has no beat grid",
                  },
                ];
              }
              return [
                {
                  ...base,
                  edits: [
                    {
                      cue_id: "cue-1",
                      cue_label: "1:05 Drop",
                      field: "Color",
                      before: -1,
                      after: 1,
                    },
                  ],
                  deletions: [{ cue_id: "cue-2", cue_label: "2:00" }],
                  skipped: null,
                },
              ];
            }
            case "csv_import_fields":
              return ["title", "artist", "genre"];
            case "csv_import_headers":
              return ["Artist", "Title", "Genre"];
            case "csv_import_preview":
              return {
                report: {
                  rows: 2,
                  matched: 1,
                  already_current: 0,
                  unmatched: 1,
                  ambiguous: 0,
                  changes: 1,
                },
                rows: [
                  {
                    row: {
                      line: 2,
                      location: null,
                      artist: "daft punk",
                      title: "get lucky",
                      values: { genre: "Disco" },
                    },
                    outcome: {
                      kind: "matched",
                      track_id: "1",
                      track_title: "get lucky",
                      changes: [["genre", "House", "Disco"]],
                    },
                  },
                  {
                    row: {
                      line: 3,
                      location: null,
                      artist: "nobody",
                      title: "nothing",
                      values: { genre: "Techno" },
                    },
                    outcome: { kind: "unmatched" },
                  },
                ],
              };
            case "csv_import_apply":
              return ["csv-change-0"];

            case "cue_recipe_apply": {
              const tracks = args.tracks as Array<Record<string, unknown>>;
              tracks.forEach((t) => staged.push(t));
              return tracks.map((_, i) => `cue-change-${i}`);
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

/**
 * The field-recipe Preview button.
 *
 * `exact` matters twice over: getByRole matches the accessible name as a
 * case-insensitive substring, so a plain "Preview" also picks up the cue
 * section's "Preview cues", and the tag section has a "Preview" of its own.
 */
function fieldPreview(page: import("@playwright/test").Page) {
  return page.getByRole("button", { name: "Preview", exact: true }).first();
}

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
  await expect(fieldPreview(page)).toBeDisabled();

  await page.getByRole("button", { name: "Add", exact: true }).click();
  await fieldPreview(page).click();

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
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await fieldPreview(page).click();
  await expect(page.getByTestId("recipe-preview")).toBeVisible();

  await page.getByLabel("Keep get lucky title").uncheck();
  await expect(page.getByRole("button", { name: /Stage 0 change/ })).toBeDisabled();
});

test("tag recipes: import from text previews the tags it would add", async ({
  page,
}) => {
  await openRecipes(page);

  // Defaults come from the spec: source Comment, marker "#".
  await expect(page.getByLabel("Import source field")).toHaveValue("comment");
  await expect(page.getByLabel("Tag marker")).toHaveValue("#");

  await page.getByRole("button", { name: "Preview", exact: true }).nth(1).click();

  const preview = page.getByTestId("tag-recipe-preview");
  await expect(preview).toBeVisible();
  await expect(preview.getByText("+Techno")).toBeVisible();
  await expect(preview.getByText("+Vocals")).toBeVisible();

  await page.getByRole("button", { name: /Apply to 1 track/ }).click();
  // Importing may have to invent tags, and says which.
  await expect(page.getByText(/created Techno/)).toBeVisible();
});

test("cue recipes: preview edits and deletions, then stage them", async ({
  page,
}) => {
  await openRecipes(page);

  await expect(page.getByRole("button", { name: /Stage 0 track/ })).toBeDisabled();
  await page.getByRole("button", { name: "Preview cues" }).click();

  const preview = page.getByTestId("cue-recipe-preview");
  await expect(preview).toBeVisible();
  await expect(preview.getByText("1 edit(s)")).toBeVisible();
  await expect(preview.getByText("−1 cue(s)")).toBeVisible();

  await page.getByRole("button", { name: /Stage 1 track/ }).click();
  await expect(page.getByText(/Staged 1 cue change\(s\) for review/)).toBeVisible();
  // The preview clears, so a second Stage cannot double-stage the same edits.
  await expect(preview).toBeHidden();
});

test("cue recipes: quantizing an unanalysed track says why, and stages nothing", async ({
  page,
}) => {
  await openRecipes(page);

  await page.getByLabel("Cue operation").selectOption("quantize_cues");
  await expect(page.getByLabel("Quantize resolution")).toBeVisible();
  await page.getByRole("button", { name: "Preview cues" }).click();

  const preview = page.getByTestId("cue-recipe-preview");
  await expect(preview.getByText("this track has no beat grid")).toBeVisible();
  // Honest labelling: the track is listed with its reason, not silently
  // dropped, but there is nothing to stage.
  await expect(page.getByRole("button", { name: /Stage 0 track/ })).toBeDisabled();
});

test("csv import: map columns, preview, and stage what matched", async ({
  page,
}) => {
  await openRecipes(page);

  // Nothing to map until a file is chosen.
  await expect(page.getByLabel("Artist column")).toHaveCount(0);

  await page.getByLabel("CSV file").setInputFiles({
    name: "tags.csv",
    mimeType: "text/csv",
    buffer: Buffer.from(
      "Artist,Title,Genre\ndaft punk,get lucky,Disco\nnobody,nothing,Techno\n",
    ),
  });

  // Artist and Title are guessed from the header names.
  await expect(page.getByLabel("Artist column")).toHaveValue("Artist");
  await expect(page.getByLabel("Title column")).toHaveValue("Title");
  // Matching is satisfied, but there is still nothing to write.
  await expect(page.getByTestId("csv-no-match-strategy")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Preview import" })).toBeDisabled();

  await page.getByLabel("Import column").selectOption("Genre");
  await page.getByLabel("Import into field").selectOption("genre");
  await page.getByRole("button", { name: "Add column" }).click();
  await page.getByRole("button", { name: "Preview import" }).click();

  const preview = page.getByTestId("csv-import-preview");
  await expect(preview).toContainText("2 row(s)");
  // The row that matched nothing is reported, not silently dropped.
  await expect(preview).toContainText("no matching track");

  await page.getByRole("button", { name: /Stage 1 change/ }).click();
  await expect(page.getByText(/Staged 1 change\(s\) for review/)).toBeVisible();
});
