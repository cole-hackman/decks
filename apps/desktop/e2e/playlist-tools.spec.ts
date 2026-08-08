import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ libraryPath }) => {
      let savedPath: string | null = null;

      const base = {
        album: null,
        genre: "House",
        musical_key: "8A",
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
      };

      const tracks = [
        { ...base, id: "1", title: "Loud", artist: "A", bpm: 128, folder_path: "/a.mp3", energy: 9 },
        { ...base, id: "2", title: "Quiet", artist: "B", bpm: 124, folder_path: "/b.mp3", energy: 2 },
        { ...base, id: "3", title: "Middle", artist: "C", bpm: 126, folder_path: "/c.mp3", energy: 5 },
      ];

      const playlists = [
        { id: "f1", name: "Sets", parent_id: null, seq: 1, kind: "Folder" },
        { id: "p1", name: "Warmup", parent_id: "f1", seq: 1, kind: "Playlist" },
        { id: "p2", name: "Peak", parent_id: "f1", seq: 2, kind: "Playlist" },
      ];

      const members: Record<string, string[]> = {
        p1: ["1", "2"],
        p2: ["2", "3"],
      };

      const staged: Array<Record<string, unknown>> = [];
      let favourites: Array<{ playlist_id: string; seq: number }> = [];

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
              return [];
            case "get_api_key":
              return null;

            case "preview_playlist_merge": {
              const ids = args.playlistIds as string[];
              const rows = ids.flatMap((id) => members[id] ?? []);
              return {
                track_ids: [...new Set(rows)],
                source_rows: rows.length,
              };
            }
            case "apply_playlist_merge":
              staged.push({ kind: "PlaylistCreate", name: args.name });
              return ["c1"];

            case "preview_playlist_sort": {
              const siblings = playlists.filter(
                (p) => p.parent_id === (args.parentId ?? null),
              );
              const sorted = [...siblings].sort((a, b) =>
                args.mode === "name_desc"
                  ? b.name.localeCompare(a.name)
                  : a.name.localeCompare(b.name),
              );
              return {
                order: sorted.map((p) => [p.id, p.name]),
                unchanged:
                  sorted.map((p) => p.id).join() ===
                  siblings.map((p) => p.id).join(),
              };
            }
            case "apply_playlist_sort":
              staged.push({ kind: "PlaylistReorder", order: args.order });
              return "c1";

            case "preview_cross_reference": {
              const ids = args.playlistIds as string[];
              const sets = ids.map((id) => new Set(members[id] ?? []));
              if (args.mode === "in_none") {
                const hit = tracks
                  .filter((t) => !sets.some((s) => s.has(t.id)))
                  .map((t) => t.id);
                return { track_ids: hit, considered: tracks.length };
              }
              const first = members[ids[0]] ?? [];
              return {
                track_ids: first.filter((t) => sets.every((s) => s.has(t))),
                considered: first.length,
              };
            }

            case "preview_playlist_prefix": {
              const ids = args.playlistIds as string[];
              const spec = args.spec as {
                text: string;
                numbering: { start: number; pad: number } | null;
              };
              return ids
                .map((id, i) => {
                  const name = playlists.find((p) => p.id === id)!.name;
                  const num = spec.numbering
                    ? String(spec.numbering.start + i).padStart(
                        spec.numbering.pad,
                        "0",
                      )
                    : "";
                  return { id, from: name, to: `${num}${spec.text}${name}` };
                })
                .filter((r) => r.from !== r.to);
            }
            case "apply_playlist_prefix": {
              const renames = args.renames as Array<{ id: string }>;
              renames.forEach((r) => staged.push({ kind: "PlaylistRename", ...r }));
              return renames.map((_, i) => `c${i}`);
            }

            case "preview_rewrite_order": {
              const req = args.request as {
                playlist_id: string;
                visible_order: string[];
              };
              const stored = members[req.playlist_id] ?? [];
              const order = req.visible_order.filter((id) => stored.includes(id));
              const appended = stored.filter((id) => !order.includes(id));
              return {
                playlist_id: req.playlist_id,
                order: [...order, ...appended],
                unknown: req.visible_order.filter((id) => !stored.includes(id)),
                appended,
                unchanged: [...order, ...appended].join() === stored.join(),
              };
            }
            case "apply_rewrite_order":
              staged.push({ kind: "PlaylistReorderTrack" });
              return "c1";

            case "share_playlist": {
              const ids = members[String(args.playlistId)] ?? [];
              const rows = ids.map((id) => tracks.find((t) => t.id === id)!);
              if (args.format === "csv") {
                const cols = args.columns as string[];
                const header = cols.join(",");
                const body = rows
                  .map((t) =>
                    cols
                      .map((c) =>
                        c === "title" ? t.title : c === "bpm" ? String(t.bpm) : "",
                      )
                      .join(","),
                  )
                  .join("\n");
                return {
                  content: `${header}\n${body}`,
                  filename: "Warmup.csv",
                  track_count: rows.length,
                  skipped: [],
                };
              }
              return {
                content: rows.map((t) => `${t.artist} - ${t.title}`).join("\n"),
                filename: "Warmup.txt",
                track_count: rows.length,
                skipped: [],
              };
            }
            case "write_share_file":
              return null;

            case "list_favourite_playlists":
              return favourites.map((f, i) => ({
                playlist_id: f.playlist_id,
                name: playlists.find((p) => p.id === f.playlist_id)!.name,
                seq: i + 1,
                track_count: (members[f.playlist_id] ?? []).length,
              }));
            case "toggle_favourite_playlist": {
              const id = String(args.playlistId);
              const had = favourites.some((f) => f.playlist_id === id);
              favourites = had
                ? favourites.filter((f) => f.playlist_id !== id)
                : [...favourites, { playlist_id: id, seq: favourites.length + 1 }];
              return !had;
            }
            case "add_tracks_to_playlist":
              return (args.trackIds as string[]).map((_, i) => `c${i}`);

            case "playlist_occurrence": {
              const counts = new Map<string, number>();
              tracks.forEach((t) => {
                const n = Object.values(members).filter((ids) =>
                  ids.includes(t.id),
                ).length;
                counts.set(t.id, n);
              });
              const distribution = new Map<number, number>();
              counts.forEach((n) =>
                distribution.set(n, (distribution.get(n) ?? 0) + 1),
              );
              return {
                tracks: tracks.filter((t) => counts.get(t.id) === args.n),
                distribution: [...distribution.entries()].sort(
                  (a, b) => a[0] - b[0],
                ),
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

async function openTools(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: "Playlist Tools" }).click();
}

test("merge: preview reports the duplicates dropped, then stages a new playlist", async ({
  page,
}) => {
  await openTools(page);

  await page.getByRole("checkbox", { name: "Warmup" }).check();
  await page.getByRole("checkbox", { name: "Peak" }).check();
  await page.getByRole("button", { name: "Preview merge" }).click();

  // Warmup [1,2] + Peak [2,3] = 4 rows, 3 tracks, 1 duplicate.
  await expect(page.getByTestId("merge-preview")).toContainText(
    "3 track(s) from 4 row(s) — 1 duplicate(s) dropped",
  );

  await page.getByLabel("Merged playlist name").fill("Combined");
  await page.getByRole("button", { name: /Stage playlist/ }).click();
  await expect(page.getByText(/Staged “Combined” with 3 track/)).toBeVisible();
});

test("sort: previewing the folder order and staging it", async ({ page }) => {
  await openTools(page);
  await page.getByRole("button", { name: "Sort", exact: true }).click();

  await page.getByLabel("Folder to sort").selectOption("f1");
  await page.getByRole("button", { name: "Preview sort" }).click();

  // Peak sorts before Warmup alphabetically, so the order changes.
  const preview = page.getByTestId("sort-preview");
  await expect(preview).toContainText("Peak");
  await page.getByRole("button", { name: "Stage order" }).click();
  await expect(page.getByText(/Staged the new playlist order/)).toBeVisible();
});

test("cross reference: the in-none mode warns before it runs", async ({ page }) => {
  await openTools(page);
  await page.getByRole("button", { name: "Cross Reference", exact: true }).click();

  await expect(page.getByTestId("xref-warning")).toHaveCount(0);
  await page.getByLabel("Cross reference mode").selectOption("in_none");
  await expect(page.getByTestId("xref-warning")).toContainText(
    "can return most of the library",
  );

  await page.getByRole("checkbox", { name: "Warmup" }).check();
  await page.getByRole("button", { name: "Run cross reference" }).click();
  // Warmup holds 1 and 2, so only track 3 is in none of the selection.
  await expect(page.getByTestId("xref-result")).toContainText("1 of 3 track(s)");
});

test("prefix: numbering follows tick order and previews before staging", async ({
  page,
}) => {
  await openTools(page);
  await page.getByRole("button", { name: "Prefix", exact: true }).click();

  await page.getByRole("checkbox", { name: "Peak" }).check();
  await page.getByRole("checkbox", { name: "Warmup" }).check();
  await page.getByRole("checkbox", { name: "Number them" }).check();
  await page.getByLabel("Prefix text").fill(" - ");
  await page.getByRole("button", { name: "Preview names" }).click();

  const preview = page.getByTestId("prefix-preview");
  await expect(preview).toContainText("01 - Peak");
  await expect(preview).toContainText("02 - Warmup");

  await page.getByRole("button", { name: /Stage 2 rename/ }).click();
  await expect(page.getByText(/Staged 2 rename\(s\)/)).toBeVisible();
});

test("rewrite order: sort by Energy and store that order", async ({ page }) => {
  await openTools(page);
  await page.getByRole("button", { name: "Rewrite Order" }).click();

  await expect(page.getByText(/knows nothing about Energy/)).toBeVisible();
  await page.getByRole("radio", { name: "Warmup" }).check();
  await page.getByRole("button", { name: "Preview order" }).click();

  // Warmup holds Loud (energy 9) and Quiet (2); ascending puts Quiet first.
  const preview = page.getByTestId("rewrite-order-preview");
  await expect(preview.locator("li").first()).toHaveText("Quiet");

  await page.getByRole("button", { name: "Stage order" }).click();
  await expect(page.getByText(/Staged the new track order/)).toBeVisible();
});

test("occurrence: exactly N, with the distribution to pick N from", async ({
  page,
}) => {
  await openTools(page);
  await page.getByRole("button", { name: "Occurrence" }).click();

  // Track 2 is in both playlists; tracks 1 and 3 are in one each.
  await page.getByLabel("Playlist count").fill("2");
  await page.getByRole("button", { name: "Find tracks" }).click();

  const result = page.getByTestId("occurrence-result");
  await expect(result).toContainText("1 track(s) are in exactly 2 playlist(s).");
  await expect(result).toContainText("Quiet");

  // Clicking a distribution row re-runs for that N.
  await page
    .getByTestId("occurrence-distribution")
    .getByRole("button", { name: "1", exact: true })
    .click();
  await expect(result).toContainText("2 track(s) are in exactly 1 playlist(s).");
});

test("share: pick columns, preview the CSV, and see the order honoured", async ({
  page,
}) => {
  await openTools(page);
  await page.getByRole("button", { name: "Share", exact: true }).click();

  await expect(
    page.getByText(/Sharing produces a file. Syncing updates Rekordbox/),
  ).toBeVisible();

  await page.getByLabel("Playlist to share").selectOption("p1");
  await page.getByRole("button", { name: "Preview export" }).click();

  const preview = page.getByTestId("share-preview");
  await expect(preview).toContainText("2 track(s)");
  await expect(preview).toContainText("Warmup.csv");
  // Default columns lead with title.
  await expect(preview).toContainText("title,artist,bpm,key,duration");
});

test("share: HTML says how to get a PDF instead of pretending to write one", async ({
  page,
}) => {
  await openTools(page);
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByLabel("Export format").selectOption("html");
  await expect(page.getByTestId("share-format-blurb")).toContainText(
    "Use the browser's Save to PDF",
  );
});

test("favourites: star a playlist, then jump to it with its hotkey", async ({
  page,
}) => {
  await openTools(page);
  await page.getByRole("button", { name: "Favourites", exact: true }).click();

  await page.getByRole("button", { name: "Star Peak" }).click();
  await expect(page.getByRole("button", { name: "Unstar Peak" })).toBeVisible();

  // The bar lives above the track browser, so it shows up in the Library view.
  await page.getByRole("button", { name: "Library" }).click();
  const bar = page.getByTestId("favourite-playlists");
  await expect(bar).toContainText("Peak");
  await expect(bar).toContainText(
    "1–9 opens · Shift+1–9 or drag files the selection",
  );

  // Hotkey 1 opens it in the playlist browser.
  await page.keyboard.press("Digit1");
  await expect(page.getByRole("heading", { name: "Peak" })).toBeVisible();
});
