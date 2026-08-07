import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ libraryPath }) => {
    let savedPath: string | null = null;
    let stagedChanges = [
      {
        id: "change-1",
        library_path: libraryPath,
        kind: "TrackMetadataEdit",
        target_id: "1",
        field: "genre",
        old_value: "House",
        new_value: "Deep House",
        reason: "Normalize genre",
        confidence: 0.9,
        status: "Proposed",
        created_at: 1,
        updated_at: 1,
      },
    ];
    const tracks = [
      {
        id: "1",
        title: "Dark Matter",
        artist: "DJ One",
        album: "Night Drive",
        genre: "House",
        musical_key: "8A",
        bpm: 128,
        duration_secs: 360,
        rating: 255,
        comment: "",
        folder_path: "/music/dark-matter.mp3",
      },
      {
        id: "2",
        title: "Acid Rain",
        artist: "DJ Two",
        album: "Warehouse",
        genre: "Techno",
        musical_key: "9A",
        bpm: 132,
        duration_secs: 330,
        rating: 204,
        comment: "",
        folder_path: "/music/acid-rain.mp3",
      },
    ];
    const playlists = [
      { id: "p1", name: "Techno Set", parent_id: null, seq: 1, kind: "Playlist" },
    ];

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
          case "get_playlist":
            return { playlist: playlists[0], tracks };
          // The change review view mounts the undo history alongside it.
          case "list_undo_runs":
            return [];
          case "list_changes":
            return stagedChanges;
          case "list_conversations":
            return [];
          case "get_api_key":
            return null;
          case "accept_change":
            stagedChanges = stagedChanges.map((change) =>
              change.id === args.id ? { ...change, status: "Accepted" } : change,
            );
            return stagedChanges[0];
          case "reject_change":
            stagedChanges = stagedChanges.map((change) =>
              change.id === args.id ? { ...change, status: "Rejected" } : change,
            );
            return stagedChanges[0];
          case "export_accepted_changes":
            stagedChanges = stagedChanges.map((change) =>
              change.status === "Accepted"
                ? { ...change, status: "Exported" }
                : change,
            );
            return { output_path: "/tmp/rekordagent-export.xml", exported_count: 1 };

          case "multi_edit_fields":
            return ["title", "artist", "genre"];
          case "multi_edit_form":
            return {
              track_count: (args.trackIds as string[]).length,
              fields: [
                ["title", { kind: "same", value: "Dark Matter" }],
                ["artist", { kind: "same", value: "DJ One" }],
                // Nothing agrees on genre in this fixture selection.
                ["genre", { kind: "multiple" }],
              ],
            };
          case "multi_edit_apply":
            return (args.edits as unknown[]).map((_, i) => `edit-${i}`);

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

test("first-run fixture library load and track selection", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await expect(page.getByText("Library connected")).toBeVisible();
  await page.getByRole("button", { name: "Open library" }).click();
  await expect(page.getByText("Dark Matter").first()).toBeVisible();
  await page.getByText("Dark Matter").first().click();
  await expect(page.getByRole("paragraph").filter({ hasText: "DJ One" })).toBeVisible();
});

test("playlist view shows playlist tracks", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: "Playlists" }).click();
  await page.getByRole("button", { name: /Techno Set/ }).click();
  await expect(page.getByText("Acid Rain").first()).toBeVisible();
});

test("diff accept and XML export", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: "Changes" }).click();
  await expect(page.getByText("Deep House")).toBeVisible();
  await page.getByRole("button", { name: "Accept", exact: true }).click();
  await page.getByRole("button", { name: "Export XML" }).click();
  await expect(page.getByText(/Exported 1 changes/)).toBeVisible();
});

test("chat exposes the audit workflow", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: "Open agent" }).click();
  await expect(page.getByRole("button", { name: "Start Library Audit" })).toBeVisible();
});

test("manual editor: only the touched field is written", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();

  await page.getByText("Dark Matter").first().click();
  await page.keyboard.press("e");

  const editor = page.getByRole("dialog", { name: "Edit tracks" });
  await expect(editor).toBeVisible();
  // A field the selection disagrees on is a placeholder, carrying no value.
  await expect(editor.getByLabel("genre")).toHaveValue("");
  await expect(editor.getByLabel("genre")).toHaveAttribute(
    "placeholder",
    "<multiple values>",
  );

  await editor.getByLabel("genre").fill("Techno");
  await expect(page.getByTestId("multi-edit-count")).toHaveText(
    "1 field(s) changed",
  );
  await editor.getByRole("button", { name: "Save" }).click();

  // One field changed, so one change staged — the untouched title and artist
  // are not written even though the form showed values for them.
  await expect(page.getByText(/Staged 1 change\(s\) for review/)).toBeVisible();
  await expect(editor).toBeHidden();
});

test("manual editor: Escape discards", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();

  await page.getByText("Dark Matter").first().click();
  await page.keyboard.press("e");

  const editor = page.getByRole("dialog", { name: "Edit tracks" });
  await editor.getByLabel("genre").fill("Techno");
  await page.keyboard.press("Escape");
  await expect(editor).toBeHidden();
  await expect(page.getByText(/Staged/)).toHaveCount(0);
});
