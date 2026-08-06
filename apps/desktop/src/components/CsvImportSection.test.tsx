import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CsvImportSection } from "./CsvImportSection";
import {
  csvImportApply,
  csvImportFields,
  csvImportHeaders,
  csvImportPreview,
} from "../ipc";
import { WithProviders } from "../test-utils/providers";
import type { CsvImportPreview } from "../types";

vi.mock("../ipc", () => ({
  csvImportHeaders: vi.fn(),
  csvImportFields: vi.fn(),
  csvImportPreview: vi.fn(),
  csvImportApply: vi.fn(),
}));

const PREVIEW: CsvImportPreview = {
  rows: [
    {
      row: { line: 2, location: null, artist: "a", title: "b", values: {} },
      outcome: {
        kind: "matched",
        track_id: "t1",
        track_title: "get lucky",
        changes: [["genre", "House", "Disco"]],
      },
    },
    {
      row: { line: 3, location: null, artist: "x", title: "y", values: {} },
      outcome: { kind: "unmatched" },
    },
    {
      row: { line: 4, location: null, artist: "p", title: "q", values: {} },
      outcome: { kind: "ambiguous", count: 2 },
    },
  ],
  report: {
    rows: 3,
    matched: 1,
    already_current: 0,
    unmatched: 1,
    ambiguous: 1,
    changes: 1,
  },
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(csvImportFields).mockResolvedValue(["genre", "comment"]);
  vi.mocked(csvImportHeaders).mockResolvedValue(["Artist", "Title", "Genre"]);
  vi.mocked(csvImportPreview).mockResolvedValue(PREVIEW);
  vi.mocked(csvImportApply).mockResolvedValue(["c1"]);
});

function renderSection() {
  render(
    <WithProviders>
      <CsvImportSection libraryPath="/lib.db" />
    </WithProviders>,
  );
}

/** Upload a CSV and wait for the column pickers to appear. */
async function upload(user: ReturnType<typeof userEvent.setup>) {
  const file = new File(["Artist,Title,Genre\na,b,Disco\n"], "tags.csv", {
    type: "text/csv",
  });
  await user.upload(screen.getByLabelText("CSV file"), file);
  await screen.findByLabelText("Artist column");
}

describe("CsvImportSection", () => {
  it("shows no mapping controls until a file is chosen", async () => {
    renderSection();
    // `waitFor` rather than a bare assertion: the field list arrives from an
    // effect, and letting that promise land after the test has finished is
    // what produces React's "not wrapped in act(...)" warning.
    await waitFor(() => {
      expect(screen.queryByLabelText("Artist column")).not.toBeInTheDocument();
    });
  });

  it("guesses the obvious columns from the header names", async () => {
    const user = userEvent.setup();
    renderSection();
    await upload(user);
    expect(screen.getByLabelText("Artist column")).toHaveValue("Artist");
    expect(screen.getByLabelText("Title column")).toHaveValue("Title");
    // No Location column in this file, so it stays unset rather than guessing.
    expect(screen.getByLabelText("Location column")).toHaveValue("");
  });

  it("says why nothing can match when no strategy is configured", async () => {
    const user = userEvent.setup();
    vi.mocked(csvImportHeaders).mockResolvedValue(["Col A", "Col B"]);
    renderSection();
    await upload(user);
    expect(screen.getByTestId("csv-no-match-strategy")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preview import" })).toBeDisabled();
  });

  it("cannot preview until a column has been mapped to a field", async () => {
    const user = userEvent.setup();
    renderSection();
    await upload(user);
    // Artist and Title were guessed, so matching is fine — but nothing to write.
    expect(screen.queryByTestId("csv-no-match-strategy")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preview import" })).toBeDisabled();
  });

  it("sends the mapping the user built", async () => {
    const user = userEvent.setup();
    renderSection();
    await upload(user);
    await user.selectOptions(screen.getByLabelText("Import column"), "Genre");
    await user.selectOptions(screen.getByLabelText("Import into field"), "genre");
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await user.click(screen.getByRole("button", { name: "Preview import" }));

    await waitFor(() => {
      expect(csvImportPreview).toHaveBeenCalledWith(
        "/lib.db",
        "Artist,Title,Genre\na,b,Disco\n",
        {
          location: null,
          artist: "Artist",
          title: "Title",
          fields: [["Genre", "genre"]],
        },
      );
    });
  });

  it("reports every row, including the ones it could not use", async () => {
    const user = userEvent.setup();
    renderSection();
    await upload(user);
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await user.click(screen.getByRole("button", { name: "Preview import" }));

    const preview = await screen.findByTestId("csv-import-preview");
    expect(preview).toHaveTextContent("3 row(s)");
    // Unmatched and ambiguous rows are shown with a reason, not dropped —
    // an import that silently skipped a third of the file would look fine.
    expect(preview).toHaveTextContent("no matching track");
    expect(preview).toHaveTextContent("2 tracks match — cannot tell which");
  });

  it("row numbers are the ones the spreadsheet shows", async () => {
    const user = userEvent.setup();
    renderSection();
    await upload(user);
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await user.click(screen.getByRole("button", { name: "Preview import" }));
    const preview = await screen.findByTestId("csv-import-preview");
    expect(preview).toHaveTextContent("2");
    expect(preview).toHaveTextContent("4");
  });

  it("stages exactly what the preview showed", async () => {
    const user = userEvent.setup();
    renderSection();
    await upload(user);
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await user.click(screen.getByRole("button", { name: "Preview import" }));
    await user.click(await screen.findByRole("button", { name: /Stage 1 change/ }));
    await waitFor(() => {
      expect(csvImportApply).toHaveBeenCalledWith("/lib.db", PREVIEW.rows);
    });
  });

  it("offers nothing to stage when no row would change", async () => {
    const user = userEvent.setup();
    vi.mocked(csvImportPreview).mockResolvedValue({
      rows: [],
      report: {
        rows: 2,
        matched: 0,
        already_current: 2,
        unmatched: 0,
        ambiguous: 0,
        changes: 0,
      },
    });
    renderSection();
    await upload(user);
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await user.click(screen.getByRole("button", { name: "Preview import" }));
    expect(
      await screen.findByRole("button", { name: /Stage 0 change/ }),
    ).toBeDisabled();
  });

  it("drops a stale preview when the mapping changes", async () => {
    const user = userEvent.setup();
    renderSection();
    await upload(user);
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await user.click(screen.getByRole("button", { name: "Preview import" }));
    await screen.findByTestId("csv-import-preview");

    await user.selectOptions(screen.getByLabelText("Artist column"), "");
    expect(screen.queryByTestId("csv-import-preview")).not.toBeInTheDocument();
  });

  it("surfaces a parse failure instead of failing silently", async () => {
    const user = userEvent.setup();
    vi.mocked(csvImportHeaders).mockRejectedValue(new Error("unterminated quote"));
    renderSection();
    const file = new File(["broken"], "tags.csv", { type: "text/csv" });
    await user.upload(screen.getByLabelText("CSV file"), file);
    expect(await screen.findByText(/unterminated quote/)).toBeInTheDocument();
  });
});
