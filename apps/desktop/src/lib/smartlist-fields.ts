/**
 * Field and operator metadata for the smartlist editor.
 *
 * Mirrors `crates/smartlists::model`. Kept as pure data so the editor can offer
 * only the operators that make sense for a field, and so the same validation
 * the Rust side enforces is visible in the UI before a save round-trip.
 */

import type {
  SmartlistClause,
  SmartlistField,
  SmartlistFieldKind,
  SmartlistOperator,
  SmartlistValue,
} from "../types";

export const FIELD_KINDS: Record<SmartlistField, SmartlistFieldKind> = {
  title: "text",
  artist: "text",
  album: "text",
  genre: "text",
  comment: "text",
  file_path: "text",
  musical_key: "key",
  bpm: "number",
  rating: "number",
  year: "number",
  duration_secs: "number",
  bit_rate: "number",
  sample_rate: "number",
  play_count: "number",
  energy: "number",
  has_cues: "bool",
  in_any_playlist: "bool",
  is_file_missing: "bool",
  is_archived: "bool",
  tags: "tags",
};

export const FIELD_LABELS: Record<SmartlistField, string> = {
  title: "Title",
  artist: "Artist",
  album: "Album",
  genre: "Genre",
  comment: "Comment",
  file_path: "File path",
  musical_key: "Key",
  bpm: "BPM",
  rating: "Rating",
  year: "Year",
  duration_secs: "Duration (s)",
  bit_rate: "Bitrate",
  sample_rate: "Sample rate",
  play_count: "Play count",
  energy: "Energy",
  has_cues: "Has cues",
  in_any_playlist: "In any playlist",
  is_file_missing: "File is missing",
  is_archived: "Is archived",
  tags: "Custom tags",
};

export const OPERATOR_LABELS: Record<SmartlistOperator, string> = {
  contains: "contains",
  not_contains: "does not contain",
  equals: "is",
  not_equals: "is not",
  is_none: "is empty",
  is_not_none: "is not empty",
  greater_than: ">",
  less_than: "<",
  greater_or_equal: "≥",
  less_or_equal: "≤",
  between: "between",
  is_true: "is true",
  is_false: "is false",
  has_all: "has all of",
  has_any: "has any of",
  has_none: "has none of",
};

/** Operators valid for each field kind — the same table `Rule::validate`
 *  enforces in Rust. */
export const OPERATORS_BY_KIND: Record<SmartlistFieldKind, SmartlistOperator[]> = {
  text: [
    "contains",
    "not_contains",
    "equals",
    "not_equals",
    "is_none",
    "is_not_none",
  ],
  key: ["equals", "not_equals", "is_none", "is_not_none"],
  number: [
    "equals",
    "not_equals",
    "greater_than",
    "less_than",
    "greater_or_equal",
    "less_or_equal",
    "between",
    "is_none",
    "is_not_none",
  ],
  bool: ["is_true", "is_false"],
  tags: ["has_all", "has_any", "has_none"],
};

export const ALL_FIELDS = Object.keys(FIELD_LABELS) as SmartlistField[];

export function operatorsFor(field: SmartlistField): SmartlistOperator[] {
  return OPERATORS_BY_KIND[FIELD_KINDS[field]];
}

/** Whether an operator takes an operand. `is empty` / `is true` do not. */
export function takesOperand(op: SmartlistOperator): boolean {
  return !["is_none", "is_not_none", "is_true", "is_false"].includes(op);
}

/** A sensible default value when the field or operator changes, so the editor
 *  never holds a rule the backend would reject. */
export function defaultValueFor(
  field: SmartlistField,
  op: SmartlistOperator,
): SmartlistValue {
  if (!takesOperand(op)) return { type: "none" };
  const kind = FIELD_KINDS[field];
  if (kind === "tags") return { type: "tags", value: [] };
  if (kind === "number") {
    return op === "between"
      ? { type: "range", value: [0, 0] }
      : { type: "number", value: 0 };
  }
  return { type: "text", value: "" };
}

/** Keep a rule internally consistent after the user changes its field: pick the
 *  first valid operator if the current one no longer applies, and reset the
 *  value to match. */
export function coerceRule(
  field: SmartlistField,
  op: SmartlistOperator,
): { field: SmartlistField; op: SmartlistOperator; value: SmartlistValue } {
  const valid = operatorsFor(field);
  const nextOp = valid.includes(op) ? op : valid[0];
  return { field, op: nextOp, value: defaultValueFor(field, nextOp) };
}

export function emptyClause(): SmartlistClause {
  return {
    rules: [{ field: "genre", op: "equals", value: { type: "text", value: "" } }],
  };
}

/** Human-readable summary of a rule, used in the smartlist list rows. */
export function describeRule(
  field: SmartlistField,
  op: SmartlistOperator,
  value: SmartlistValue,
): string {
  const head = `${FIELD_LABELS[field]} ${OPERATOR_LABELS[op]}`;
  switch (value.type) {
    case "text":
      return `${head} "${value.value}"`;
    case "number":
      return `${head} ${value.value}`;
    case "range":
      return `${head} ${value.value[0]}–${value.value[1]}`;
    case "tags":
      return `${head} ${value.value.length} tag(s)`;
    case "none":
      return head;
  }
}
