import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ libraryPath }) => {
      let savedPath: string | null = null;

      const base = {
        album: null,
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
        {
          ...base,
          id: "1",
          title: "Seed Track",
          artist: "DJ One",
          genre: "House",
          musical_key: "8A",
          bpm: 128,
          folder_path: "/music/seed.mp3",
        },
        {
          ...base,
          id: "2",
          title: "Same Key",
          artist: "DJ Two",
          genre: "House",
          musical_key: "8A",
          bpm: 129,
          folder_path: "/music/same.mp3",
        },
        {
          ...base,
          id: "3",
          title: "Wrong Key",
          artist: "DJ Three",
          genre: "Techno",
          musical_key: "11B",
          bpm: 128,
          folder_path: "/music/wrong.mp3",
        },
      ];

      const BASIC = {
        bpm_tolerance_pct: 6,
        match_key: true,
        key_mixing_mode: "harmonically_compatible",
        include_half_double: false,
        must_have_cues: false,
        genres: [],
        year: { kind: "off" },
        energy: { kind: "off" },
        rating: { kind: "off" },
        must_have_tags: [],
        must_not_have_tags: [],
        match_color: false,
        added_since: null,
        limit: 25,
      };

      let keyMixingMode = "harmonically_compatible";
      let templates: Array<Record<string, unknown>> = [];

      /** Stand-in for `scoring::find_mixable` — key and genre only. The real
       *  rule semantics are covered by the Rust tests; this proves the panel
       *  round-trips its options and renders what comes back. */
      function mixable(args: Record<string, unknown>) {
        const options = (args.options ?? BASIC) as typeof BASIC;
        const source = tracks.find((t) => t.id === String(args.trackId))!;
        const matches = tracks
          .filter((t) => t.id !== source.id)
          .filter((t) => !options.match_key || t.musical_key === source.musical_key)
          .filter(
            (t) =>
              options.genres.length === 0 ||
              options.genres.includes(t.genre ?? ""),
          )
          .map((t) => ({
            track: t,
            score: t.musical_key === source.musical_key ? 100 : 60,
            reasons: ["Perfect Harmonic Match"],
            bpm_relation: "direct",
            key_relation: "same",
          }));
        return {
          source,
          matches,
          considered: tracks.length - 1,
          compatible_keys: ["8A", "8B", "7A", "9A"],
        };
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
              return [];
            case "get_api_key":
              return null;

            case "mixable_default_options":
              return { ...BASIC, key_mixing_mode: keyMixingMode };
            case "get_key_mixing_mode":
              return keyMixingMode;
            case "set_key_mixing_mode":
              keyMixingMode = String(args.mode);
              return null;
            case "find_mixable_tracks":
              return mixable(args);
            case "list_mixable_templates":
              return templates;
            case "save_mixable_template":
              templates = [
                ...templates.filter((t) => t.name !== args.name),
                {
                  id: "t1",
                  name: String(args.name),
                  options: args.options,
                  created_at: 1770000000,
                },
              ];
              return "t1";
            case "delete_mixable_template":
              templates = templates.filter((t) => t.id !== args.id);
              return true;

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

async function openMixable(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  // Seed the panel by selecting a track, then open it from the header.
  await page.getByText("Seed Track").first().click();
  await page.getByRole("button", { name: "Show mixable tracks" }).click();
}

test("rank the library against a track, then re-seed from a result", async ({
  page,
}) => {
  await openMixable(page);

  const panel = page.getByLabel("Mixable tracks", { exact: true });
  await expect(panel).toBeVisible();
  await expect(page.getByTestId("compatible-keys")).toContainText("8A, 8B");

  // Key rule on: only the same-key track survives.
  await expect(page.getByTestId("mixable-count")).toContainText("1 of 2");
  const results = page.getByTestId("mixable-results");
  await expect(results.getByText("Same Key")).toBeVisible();
  await expect(results.getByText("Wrong Key")).toHaveCount(0);

  // "Use as next track" re-seeds the panel from the track just picked.
  await results.getByRole("button", { name: "Use as next track" }).first().click();
  await expect(panel.getByText("Same Key").first()).toBeVisible();
});

test("turning off Match key widens the list", async ({ page }) => {
  await openMixable(page);

  await expect(page.getByTestId("mixable-count")).toContainText("1 of 2");
  await page.getByLabel("Match key").uncheck();
  await expect(page.getByTestId("mixable-count")).toContainText("2 of 2");
  await expect(
    page.getByTestId("mixable-results").getByText("Wrong Key"),
  ).toBeVisible();
});

test("advanced rules are hidden until asked for, and save as a template", async ({
  page,
}) => {
  await openMixable(page);

  await expect(page.getByTestId("advanced-rules")).toHaveCount(0);
  await page.getByRole("button", { name: "Advanced rules" }).click();

  // Narrow to a genre the only candidate does not have.
  await page.getByLabel("Genres").fill("Techno");
  await expect(page.getByTestId("mixable-count")).toContainText("0 of 2");
  await expect(
    page.getByText("Nothing matched. Widen the BPM range, or turn off Match key."),
  ).toBeVisible();

  await page.getByLabel("Template name").fill("Techno only");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByTestId("mixable-templates")).toContainText("Techno only");
});

test("the key mixing mode is a global setting, not a per-search one", async ({
  page,
}) => {
  await openMixable(page);

  await page.getByLabel("Key mixing mode").selectOption("fuzzy");
  // Close and reopen: the mode came from the backend, so it survives.
  await page.getByRole("button", { name: "Close mixable tracks" }).click();
  await page.getByRole("button", { name: "Show mixable tracks" }).click();
  await expect(page.getByLabel("Key mixing mode")).toHaveValue("fuzzy");
});
