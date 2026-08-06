import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ libraryPath }) => {
    let savedPath: string | null = null;

    // Three "fix_casing" proposals — caller will deselect one before applying.
    const previewRows = [
      {
        id: "p1",
        track_id: "t1",
        track_title: "BIG TITLE ONE",
        field: "Title",
        old_value: "BIG TITLE ONE",
        new_value: "Big Title One",
      },
      {
        id: "p2",
        track_id: "t2",
        track_title: "ALL CAPS TWO",
        field: "Title",
        old_value: "ALL CAPS TWO",
        new_value: "All Caps Two",
      },
      {
        id: "p3",
        track_id: "t3",
        track_title: "SHOUTING THREE",
        field: "Title",
        old_value: "SHOUTING THREE",
        new_value: "Shouting Three",
      },
    ];

    let stagedChanges: Array<Record<string, unknown>> = [];
    let nextChangeId = 1;

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
            return [];
          case "get_track_cues":
            return [];
          case "list_playlists":
            return [];
          case "list_conversations":
            return [];
          case "get_api_key":
            return null;
          case "list_genres":
            return [];
          case "list_artists":
            return [];
          case "smart_fix_preview":
            // Only respond for fix_casing in this test.
            if (args.fixName === "fix_casing") return previewRows;
            return [];
          case "smart_fix_apply": {
            const ids = (args.proposalIds as string[]) ?? [];
            for (const id of ids) {
              const row = previewRows.find((r) => r.id === id);
              if (!row) continue;
              stagedChanges.push({
                id: `change-${nextChangeId++}`,
                library_path: libraryPath,
                kind: "TrackMetadataEdit",
                target_id: row.track_id,
                field: row.field,
                old_value: row.old_value,
                new_value: row.new_value,
                reason: `smart_fix:${args.fixName}`,
                confidence: 0.95,
                status: "Proposed",
                created_at: 1,
                updated_at: 1,
              });
            }
            return ids.length;
          }
          // The change review view mounts the undo history alongside it.
          case "list_undo_runs":
            return [];
          case "list_changes":
            return stagedChanges;
          case "accept_change":
            stagedChanges = stagedChanges.map((c) =>
              c.id === args.id ? { ...c, status: "Accepted" } : c,
            );
            return stagedChanges.find((c) => c.id === args.id);
          case "reject_change":
            stagedChanges = stagedChanges.map((c) =>
              c.id === args.id ? { ...c, status: "Rejected" } : c,
            );
            return stagedChanges.find((c) => c.id === args.id);
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

test("smart fixes: preview, deselect one, stage, and changes appear in review", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();

  // Navigate to Smart Fixes view via the sidebar.
  await page.getByRole("button", { name: "Smart Fixes" }).click();

  // Expand the "Fix Casing" card.
  await page.getByText("Fix Casing", { exact: true }).click();

  // Run the scan.
  await page.getByRole("button", { name: "Scan" }).click();

  // All three preview rows render.
  await expect(page.getByRole("cell", { name: "BIG TITLE ONE" }).first()).toBeVisible();
  await expect(page.getByRole("cell", { name: "ALL CAPS TWO" }).first()).toBeVisible();
  await expect(page.getByRole("cell", { name: "SHOUTING THREE" }).first()).toBeVisible();
  await expect(page.getByRole("cell", { name: "Big Title One", exact: true })).toBeVisible();

  // Initially, "3 of 3 included" is shown.
  await expect(page.getByText("3 of 3 included")).toBeVisible();

  // Deselect the second proposal by unchecking its row.
  const checkboxes = page.getByRole("checkbox");
  await expect(checkboxes).toHaveCount(3);
  await checkboxes.nth(1).uncheck();

  await expect(page.getByText("2 of 3 included")).toBeVisible();

  // Apply the remaining two.
  await page.getByRole("button", { name: /Stage 2 changes/ }).click();

  // Toast confirms 2 staged.
  await expect(page.getByText(/Staged 2 proposal/)).toBeVisible();

  // Navigate to Changes view and confirm the rows have the right field + values.
  // The sidebar entry now reads "Changes 2 pending" because we just staged two.
  await page.getByRole("button", { name: /^Changes(\s|$)/ }).click();

  await expect(page.getByText("Big Title One", { exact: true })).toBeVisible();
  await expect(page.getByText("Shouting Three", { exact: true })).toBeVisible();
  // The deselected proposal must NOT be present in the staged-changes list.
  await expect(page.getByText("All Caps Two", { exact: true })).toHaveCount(0);
});
