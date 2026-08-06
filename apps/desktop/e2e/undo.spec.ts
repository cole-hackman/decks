import { expect, test } from "@playwright/test";

const LIBRARY_PATH = "/fixture/master.db";

/**
 * Undo History round-trip.
 *
 * An undo does not write: it stages the inverse of a sync run as ordinary
 * proposed changes, which then go through the same review and the same guarded
 * Sync. This spec asserts exactly that — the inverses land in the change list,
 * and what could not be reversed is reported rather than swallowed.
 */
test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ libraryPath }) => {
      let savedPath: string | null = null;
      const stagedChanges: Array<Record<string, unknown>> = [];
      let nextChangeId = 1;

      const runs = [
        {
          id: "r1",
          library_path: libraryPath,
          applied_at: 1_700_000_000,
          undone_at: null as number | null,
          reversible: 2,
          blocked: 1,
        },
      ];

      const entries = [
        {
          id: "e1",
          source_change_id: "c1",
          kind: "TrackMetadataEdit",
          target_id: "t1",
          field: "Title",
          old_value: "Get Lucky",
          new_value: "get lucky",
          description: 'Title: "Get Lucky" → "get lucky"',
          blocked_reason: null as string | null,
        },
        {
          id: "e2",
          source_change_id: "c2",
          kind: "TrackMetadataEdit",
          target_id: "t1",
          field: "Genre",
          old_value: "HOUSE",
          new_value: "House",
          description: 'Genre: "House" → "HOUSE"',
          blocked_reason: null,
        },
        {
          id: "e3",
          source_change_id: "c3",
          kind: null,
          target_id: null,
          field: null,
          old_value: null,
          new_value: null,
          description: "TrackAddCue on t1",
          blocked_reason:
            "the new row's id is generated when the change is applied, so there is nothing to remove",
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
              return 1;
            case "set_library_path":
              savedPath = String(args.path);
              return null;
            case "list_tracks":
            case "get_track_cues":
            case "list_playlists":
            case "list_conversations":
            case "list_smartlists":
              return [];
            case "get_api_key":
              return null;

            case "list_changes":
              // A fresh array each call: returning the live reference means a
              // refetch hands React the same object and nothing re-renders.
              return stagedChanges.map((c) => ({ ...c }));
            case "list_undo_runs":
              return runs;
            case "undo_run_entries":
              return entries.filter(() => args.runId === "r1");

            case "undo_run": {
              const run = runs.find((r) => r.id === args.runId);
              if (!run || run.undone_at != null) {
                throw new Error("that sync run has already been undone");
              }
              run.undone_at = 1_700_000_100;
              const staged: string[] = [];
              const blocked: [string, string][] = [];
              for (const e of entries) {
                if (e.kind == null) {
                  blocked.push([e.description, e.blocked_reason ?? "not reversible"]);
                  continue;
                }
                const id = `u${nextChangeId++}`;
                stagedChanges.push({
                  id,
                  library_path: libraryPath,
                  kind: e.kind,
                  target_id: e.target_id,
                  field: e.field,
                  // The inverse points the other way round from the original.
                  old_value: e.new_value,
                  new_value: e.old_value,
                  reason: `Undo — ${e.description}`,
                  confidence: 1.0,
                  status: "Proposed",
                  created_at: 0,
                  updated_at: 0,
                });
                staged.push(id);
              }
              return { staged, blocked };
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

async function openChanges(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("button", { name: "Browse…" }).click();
  await page.getByRole("button", { name: "Open library" }).click();
  await page.getByRole("button", { name: /^Changes(\s|$)/ }).click();
}

test("undo a sync run: inverses are staged for review, not written", async ({
  page,
}) => {
  await openChanges(page);

  const history = page.getByLabel("Undo history");
  await expect(history).toBeVisible();
  await expect(history.getByText("2 reversible, 1 not")).toBeVisible();

  // Expanding says what the undo would do, in the undo's direction.
  await page.getByRole("button", { name: /Sync of/ }).click();
  const entries = page.getByTestId("undo-entries");
  await expect(entries).toContainText('Title: "Get Lucky" → "get lucky"');
  // And what it would not do, with the reason attached.
  await expect(entries).toContainText("id is generated when the change is applied");

  await page.getByRole("button", { name: "Undo 2" }).click();
  await expect(page.getByText(/Staged 2 change\(s\) for review/)).toBeVisible();
  await expect(page.getByText(/1 could not be reversed/)).toBeVisible();

  // The inverses are ordinary proposed changes in the review list.
  await expect(page.getByText("get lucky", { exact: true })).toBeVisible();

  // A run cannot be undone twice into a double pile.
  await expect(page.getByText("Undone")).toBeVisible();
  await expect(page.getByRole("button", { name: "Undo 2" })).toHaveCount(0);
});
