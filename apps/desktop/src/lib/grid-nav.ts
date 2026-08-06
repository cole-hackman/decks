/**
 * Spreadsheet keyboard navigation for the track table.
 *
 * Per `docs/lexicon/02-library.md §Browser`. The browser already moved *rows*
 * with j/k and the arrows; what it lacked was a **cell cursor** — a focused
 * (row, column) pair you can walk left and right, page through, and open for
 * editing without reaching for the mouse.
 *
 * All of it is pure functions over a cursor and the grid's dimensions so the
 * movement rules are testable without rendering a virtualized table. The
 * component owns focus, scrolling and the editor; this owns *where the cursor
 * goes*, which is the part with all the edge cases.
 */

export interface Cell {
  row: number;
  /** Index into the visible column list, not a column id — the same column can
   *  sit at different indices as columns are hidden or reordered, and the
   *  cursor follows the position the user is looking at. */
  col: number;
}

export interface GridSize {
  rows: number;
  cols: number;
}

/**
 * Every key the grid answers to.
 *
 * `home`/`end` move within the row; the `ctrl` variants jump to the first and
 * last row, matching every spreadsheet.
 */
export type GridMove =
  | "up"
  | "down"
  | "left"
  | "right"
  | "home"
  | "end"
  | "documentStart"
  | "documentEnd"
  | "pageUp"
  | "pageDown";

/** Rows a page-up/page-down covers. The component passes its viewport height. */
export const DEFAULT_PAGE_ROWS = 20;

const clamp = (value: number, max: number) => Math.max(0, Math.min(max, value));

/**
 * Where the cursor lands.
 *
 * Movement **clamps rather than wraps**. Wrapping is briefly clever and then
 * permanently confusing: holding `↓` in a 4,000-track library should stop at
 * the bottom, not silently return to the top and let you edit the wrong row.
 *
 * Horizontal movement does not spill onto the next row either, for the same
 * reason — `→` at the last column is a no-op, not a jump to a different track.
 */
export function moveCell(
  cursor: Cell,
  move: GridMove,
  size: GridSize,
  pageRows: number = DEFAULT_PAGE_ROWS,
): Cell {
  const lastRow = size.rows - 1;
  const lastCol = size.cols - 1;
  if (lastRow < 0 || lastCol < 0) return cursor;

  const row = clamp(cursor.row, lastRow);
  const col = clamp(cursor.col, lastCol);

  switch (move) {
    case "up":
      return { row: clamp(row - 1, lastRow), col };
    case "down":
      return { row: clamp(row + 1, lastRow), col };
    case "left":
      return { row, col: clamp(col - 1, lastCol) };
    case "right":
      return { row, col: clamp(col + 1, lastCol) };
    case "home":
      return { row, col: 0 };
    case "end":
      return { row, col: lastCol };
    case "documentStart":
      return { row: 0, col: 0 };
    case "documentEnd":
      return { row: lastRow, col: lastCol };
    case "pageUp":
      return { row: clamp(row - pageRows, lastRow), col };
    case "pageDown":
      return { row: clamp(row + pageRows, lastRow), col };
  }
}

/**
 * Translate a keydown into a move, or `null` when the grid should not act.
 *
 * Returns `null` for anything with a modifier the grid does not claim, so
 * `Cmd+A`, `Cmd+C` and the browser's own shortcuts keep working — a grid that
 * swallows every keystroke is worse than one with no keyboard support.
 */
export function moveForKey(event: {
  key: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
}): GridMove | null {
  const jump = Boolean(event.ctrlKey || event.metaKey);
  if (event.altKey) return null;

  switch (event.key) {
    case "ArrowUp":
      return jump ? "documentStart" : "up";
    case "ArrowDown":
      return jump ? "documentEnd" : "down";
    case "ArrowLeft":
      return jump ? "home" : "left";
    case "ArrowRight":
      return jump ? "end" : "right";
    case "Home":
      return jump ? "documentStart" : "home";
    case "End":
      return jump ? "documentEnd" : "end";
    case "PageUp":
      return "pageUp";
    case "PageDown":
      return "pageDown";
    default:
      return null;
  }
}

/**
 * Does this keystroke start editing the focused cell?
 *
 * `Enter` and `F2` always do. A printable character does too — typing over a
 * cell is the thing that makes a grid feel like a spreadsheet rather than a
 * list — but only when unmodified, so `Cmd+A` still selects all rather than
 * replacing a title with the letter "a".
 */
export function startsEdit(event: {
  key: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
}): boolean {
  if (event.ctrlKey || event.metaKey || event.altKey) return false;
  if (event.key === "Enter" || event.key === "F2") return true;
  // `key` is the character for printable input and a name like "Shift" or
  // "ArrowUp" otherwise, so a single-character key is exactly "printable".
  return event.key.length === 1 && event.key !== " ";
}

/** The seed text an edit opens with: the typed character, or the current value. */
export function initialEditValue(
  event: { key: string },
  currentValue: string,
): string {
  return event.key.length === 1 ? event.key : currentValue;
}

/**
 * The row range a shift-extended move covers.
 *
 * The anchor is where the selection started, not where the cursor is, so
 * shift-↓ then shift-↑ shrinks the selection back rather than growing it in the
 * other direction — the behaviour every list with shift-select has.
 */
export function selectionRange(anchor: number, cursor: number): number[] {
  const start = Math.min(anchor, cursor);
  const end = Math.max(anchor, cursor);
  return Array.from({ length: end - start + 1 }, (_, i) => start + i);
}
