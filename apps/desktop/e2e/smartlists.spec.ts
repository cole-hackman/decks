import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ libraryPath }) => {
      let savedPath: string | null = null;

      interface Smartlist {
        id: string;
        name: string;
        parent_folder_id: string | null;
        combinator: string;
        clauses: Array<{ rules: Array<Record<string, unknown>> }>;
        created_at: number;
        updated_at: number;
      }

      let smartlists: Smartlist[] = [];
      let nextId = 1;

      const tracks = [
        {
          id: "1",
          title: "Deep Cut",
          artist: "DJ One",
          album: null,
          genre: "House",
          musical_key: "8A",
          bpm: 126,
          duration_secs: 360,
          rating: 3,
          comment: null,
          folder_path: "/music/a.mp3",
          analysis_data_path: null,
          file_type: 1,
          sample_rate: null,
          bit_rate: null,
          release_year: 2020,
          dj_play_count: 2,
          energy: null,
        },
        {
          id: "2",
          title: "Hard Groove",
          artist: "DJ Two",
          album: null,
          genre: "Techno",
          musical_key: "11B",
          bpm: 140,
          duration_secs: 300,
          rating: 5,
          comment: null,
          folder_path: "/music/b.mp3",
          analysis_data_path: null,
          file_type: 1,
          sample_rate: null,
          bit_rate: null,
          release_year: 2021,
          dj_play_count: 0,
          energy: null,
        },
      ];

      /** Minimal stand-in for the Rust evaluator: enough to prove the UI wiring
       *  round-trips. Rule semantics themselves are covered by Rust tests. */
      function evaluate(list: Smartlist) {
        return tracks.filter((t) =>
          list.clauses.every((clause) =>
            clause.rules.some((r) => {
              const value = r.value as { type: string; value?: unknown };
              if (r.field === "genre" && r.op === "equals") {
                return t.genre === value.value;
              }
              if (r.field === "bpm" && r.op === "between") {
                const [lo, hi] = value.value as [number, number];
                return t.bpm >= lo && t.bpm <= hi;
              }
              return false;
            }),
          ),
        );
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
              return 2;
            case "set_library_path":
              savedPath = String(args.path);
              return null;
            case "list_tracks":
              return tracks;
            case "get_track_cues":
            case "list_playlists":
            case "list_conversations":
            case "list_changes":
              return [];
            case "get_api_key":
              return null;

            case "list_smartlists":
              return smartlists;
            case "create_smartlist": {
              const list: Smartlist = {
                id: `s${nextId++}`,
                name: String(args.name),
                parent_folder_id: (args.parentFolderId as string | null) ?? null,
                combinator: String(args.combinator),
                clauses: args.clauses as Smartlist["clauses"],
                created_at: 1,
                updated_at: 1,
              };
              smartlists = [...smartlists, list];
              return list;
            }
            case "delete_smartlist":
              smartlists = smartlists.filter((s) => s.id !== args.id);
              return null;
            case "evaluate_smartlist": {
              const list = smartlists.find((s) => s.id === args.id);
              return list ? evaluate(list) : [];
            }
            case "preview_smartlist":
              return evaluate({
                id: "preview",
                name: "preview",
                parent_folder_id: null,
                combinator: String(args.combinator),
                clauses: args.clauses as Smartlist["clauses"],
                created_at: 0,
                updated_at: 0,
              });
            case "smartlist_counts":
              return Object.fromEntries(
                smartlists.map((s) => [s.id, evaluate(s).length]),
              );
            case "smartlist_compatibility":
              // Every rule here is a non-tag rule, so Rekordbox flattens them.
              return Object.fromEntries(
                smartlists.map((s) => [
                  s.id,
                  {
                    materialised: {
                      reason: "Rekordbox only expresses tag (MyTag) rules",
                    },
                  },
                ]),
              );
            case "generate_smartlists": {
              const genres = [...new Set(tracks.map((t) => t.genre))].sort();
              const created = genres
                .filter((g) => !smartlists.some((s) => s.name === g))
                .map((g) => ({
                  id: `s${nextId++}`,
                  name: g,
                  parent_folder_id: "Lexicon",
                  combinator: "all",
                  clauses: [
                    {
                      rules: [
                        {
                          field: "genre",
                          op: "equals",
                          value: { type: "text", value: g },
                        },
                      ],
                    },
                  ],
                  created_at: 1,
                  updated_at: 1,
                }));
              smartlists = [...smartlists, ...created];
              return created;
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

async function openLibrary(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: "Smartlists" }).click();
}

test("create a smartlist with an OR clause and see it populate", async ({ page }) => {
  await openLibrary(page);

  await expect(page.getByText("No smartlists yet.")).toBeVisible();
  await page.getByRole("button", { name: "New smartlist" }).click();

  await page.getByLabel("Name").fill("House or Techno");

  // Default rule row: Genre is "" — set it to House.
  await page.getByLabel("Field").selectOption("genre");
  await page.getByLabel("Value").fill("House");

  // Add an OR condition within the same clause and set it to Techno.
  await page.getByRole("button", { name: "+ OR condition" }).click();
  await page.getByLabel("Value").nth(1).fill("Techno");

  // Both tracks match the union.
  await expect(page.getByTestId("preview-count")).toHaveText("2 track(s) match");

  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("House or Techno").first()).toBeVisible();
  await expect(page.getByText(/2 tracks · All rules/)).toBeVisible();

  // Selecting it lists the matching tracks.
  await page.getByRole("button", { name: "Show" }).click();
  await expect(page.getByText("2 matching track(s)")).toBeVisible();
  await expect(page.getByText("Deep Cut")).toBeVisible();
  await expect(page.getByText("Hard Groove")).toBeVisible();
});

test("adding a second clause narrows the result (AND across clauses)", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "New smartlist" }).click();
  await page.getByLabel("Name").fill("House 120-130");

  await page.getByLabel("Field").selectOption("genre");
  await page.getByLabel("Value").fill("House");
  await expect(page.getByTestId("preview-count")).toHaveText("1 track(s) match");

  // Second clause: BPM between 120 and 130 — AND-ed with the genre clause.
  await page.getByRole("button", { name: "+ Add rule" }).click();
  await page.getByLabel("Field").nth(1).selectOption("bpm");
  await page.getByLabel("Operator").nth(1).selectOption("between");
  await page.getByLabel("From", { exact: true }).fill("120");
  await page.getByLabel("To", { exact: true }).fill("130");

  await expect(page.getByTestId("preview-count")).toHaveText("1 track(s) match");

  // Narrowing to a BPM band the House track misses empties the result.
  await page.getByLabel("From", { exact: true }).fill("135");
  await page.getByLabel("To", { exact: true }).fill("145");
  await expect(page.getByTestId("preview-count")).toHaveText("0 track(s) match");
});

test("the generator is idempotent across runs", async ({ page }) => {
  await openLibrary(page);

  await page.getByRole("button", { name: "By genre" }).click();
  await expect(page.getByText("Generated 2 smartlist(s).")).toBeVisible();
  await expect(page.getByText(/· generated/).first()).toBeVisible();

  // Second run creates nothing, because both already sit in the Lexicon folder.
  await page.getByRole("button", { name: "By genre" }).click();
  await expect(
    page.getByText("Nothing new to generate — everything already exists."),
  ).toBeVisible();
});
