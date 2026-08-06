import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ libraryPath }) => {
      let savedPath: string | null = null;

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
          folder_path: "/incoming/a.mp3",
          analysis_data_path: null,
          file_type: 1,
          sample_rate: null,
          bit_rate: 320,
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
          folder_path: "/music/Techno/DJ Two - Hard Groove.mp3",
          analysis_data_path: null,
          file_type: 1,
          sample_rate: null,
          bit_rate: 256,
          release_year: 2021,
          dj_play_count: 0,
          energy: null,
        },
      ];

      /** Stand-in for the Rust planner: enough to prove the UI round-trips.
       *  Pattern and subfolder semantics are covered by Rust tests. */
      function plan(args: Record<string, unknown>) {
        const request = args.request as {
          target_folder: string | null;
          filename_pattern: string | null;
          subfolders: { levels: Array<{ kind: string; name?: string }> };
        };
        const ids = args.trackIds as string[];
        return tracks
          .filter((t) => ids.includes(t.id))
          .map((t) => {
            const stem = (request.filename_pattern ?? "")
              .replace(/%artist%/g, t.artist ?? "")
              .replace(/%title%/g, t.title);
            const levels = request.subfolders.levels
              .map((l) =>
                l.kind === "field" && l.name === "genre"
                  ? t.genre
                  : l.kind === "bitrate_bucket"
                    ? t.bit_rate >= 320
                      ? "320+"
                      : "320-"
                    : null,
              )
              .filter(Boolean);
            const base = request.target_folder ?? "/incoming";
            const destination = [base, ...levels, `${stem}.mp3`].join("/");
            return {
              track_id: t.id,
              source: t.folder_path,
              destination:
                destination === t.folder_path ? null : destination,
              title: t.title,
              artist: t.artist,
            };
          });
      }

      const staged: Array<Record<string, unknown>> = [];
      let quickMoveFolders = [
        {
          id: "q1",
          path: "/music/Techno",
          favourite: true,
          last_used_at: 2,
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
            case "get_track_cues":
            case "list_playlists":
            case "list_conversations":
            case "list_changes":
            case "list_smartlists":
              return [];
            case "get_api_key":
              return null;

            case "pattern_fields":
              return [
                { name: "artist", supported: true },
                { name: "title", supported: true },
                { name: "remixer", supported: false },
              ];
            case "validate_pattern": {
              const p = String(args.pattern);
              if ((p.match(/%/g) ?? []).length % 2 !== 0) {
                throw new Error("unterminated %field% in pattern");
              }
              return [...p.matchAll(/%([^%]+)%/g)].map((m) => m[1]);
            }
            case "list_quick_move_folders":
              return quickMoveFolders;
            case "record_quick_move_folder":
              return "q1";
            case "toggle_quick_move_favourite": {
              quickMoveFolders = quickMoveFolders.map((f) =>
                f.id === args.id ? { ...f, favourite: !f.favourite } : f,
              );
              return true;
            }
            case "delete_quick_move_folder":
              quickMoveFolders = quickMoveFolders.filter((f) => f.id !== args.id);
              return true;
            case "write_tags_bulk": {
              const selection = args.selection as Record<string, boolean>;
              // Only "genre" is populated on both fixture tracks, so a
              // genre-only write hits both; anything else is a skip.
              const ids = args.trackIds as string[];
              return selection.genre
                ? { written: ids, failed: [], skipped: [] }
                : { written: [], failed: [], skipped: ids };
            }
            case "scan_unused_files": {
              const filter = args.filter as {
                mode: string;
                extensions: string[];
              };
              const candidates = [
                { path: "/music/cover.png", size_bytes: 2048 },
                { path: "/music/notes.txt", size_bytes: 10 },
              ];
              const files = candidates.filter((c) => {
                if (filter.extensions.length === 0) return true;
                const ext = c.path.split(".").pop() ?? "";
                const listed = filter.extensions.includes(ext);
                return filter.mode === "include" ? listed : !listed;
              });
              return {
                files,
                total_bytes: files.reduce((n, f) => n + f.size_bytes, 0),
                skipped_directories: ["PioneerDJ", "_Serato_"],
                errors: [],
              };
            }
            case "delete_unused_files":
              return {
                deleted: args.paths as string[],
                failed: [],
                report_path: "/data/reports/deleted-1.txt",
              };
            case "preview_organize":
              return plan(args);
            case "apply_organize": {
              const rows = args.rows as Array<{ track_id: string }>;
              rows.forEach((r) => staged.push(r));
              return {
                moved: rows.map((r) => r.track_id),
                failed: [],
                staged: rows.map((_, i) => `c${i}`),
              };
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

async function openOrganize(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: "Files" }).click();
}

test("preview a move, then apply it", async ({ page }) => {
  await openOrganize(page);

  await page.getByLabel("Target folder").fill("/music");
  await page.getByLabel("Subfolder level 1").selectOption("genre");
  await page.getByRole("button", { name: "Preview" }).click();

  const preview = page.getByTestId("organize-preview");
  await expect(preview).toBeVisible();
  await expect(
    preview.getByText("/music/House/DJ One - Deep Cut.mp3"),
  ).toBeVisible();
  // The second track already renders to where it is, so it is listed but not
  // counted as a move.
  await expect(preview.getByText("already in place")).toBeVisible();

  await page.getByRole("button", { name: "Move 1 file(s)" }).click();
  await expect(page.getByText(/Moved 1 file\(s\)/)).toBeVisible();
  await expect(page.getByText(/Sync to update Rekordbox/)).toBeVisible();
});

test("a malformed pattern blocks the preview", async ({ page }) => {
  await openOrganize(page);

  await page.getByLabel("Filename pattern").fill("%artist");
  await expect(page.getByRole("alert")).toContainText("unterminated");
  await expect(page.getByRole("button", { name: "Preview" })).toBeDisabled();
});

test("find unused files: scan, then confirm before deleting", async ({ page }) => {
  await openOrganize(page);

  await page.getByLabel("Folder to scan").fill("/music");
  await page.getByRole("button", { name: "Scan" }).click();

  const scan = page.getByTestId("unused-scan");
  await expect(scan).toBeVisible();
  await expect(scan.getByText("/music/cover.png")).toBeVisible();
  // The report says what it did not look at.
  await expect(page.getByText(/Skipped: PioneerDJ, _Serato_/)).toBeVisible();

  // Nothing is pre-selected, so deletion starts unavailable.
  await expect(
    page.getByRole("button", { name: /Delete 0 file\(s\)/ }),
  ).toBeDisabled();

  await page.getByLabel("Select /music/cover.png").check();
  await page.getByRole("button", { name: /Delete 1 file\(s\)…/ }).click();
  await expect(page.getByRole("alert")).toContainText("cannot be undone");

  await page
    .getByRole("button", { name: /Permanently delete 1 file\(s\)/ })
    .click();
  await expect(page.getByText(/Deleted 1 file\(s\)/)).toBeVisible();
  await expect(page.getByText(/deleted-1\.txt/)).toBeVisible();
});

test("write tags: nothing selected by default, then write the ticked field", async ({
  page,
}) => {
  await openOrganize(page);

  const write = page.getByRole("button", { name: /Write tags to 2 file\(s\)/ });
  await expect(write).toBeDisabled();

  await page.getByLabel("Genre").check();
  await write.click();
  await expect(page.getByText(/Wrote 2 file\(s\)/)).toBeVisible();
});

test("quick move: hotkey 1 sends the selection to the first favourite", async ({
  page,
}) => {
  await openOrganize(page);
  await expect(
    page.getByRole("button", { name: "/music/Techno", exact: true }),
  ).toBeVisible();

  // Focus something that is not a text field, then press the hotkey.
  await page.getByRole("heading", { name: "Quick Move" }).click();
  await page.keyboard.press("1");

  // Only the first fixture track moves — the second already renders to where
  // it is, so it is not part of the move.
  await expect(page.getByText(/Moved 1 file\(s\) to \/music\/Techno/)).toBeVisible();
  await expect(page.getByText(/full sync clears the old locations/)).toBeVisible();
});
