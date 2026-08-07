import type { Recipe } from "../types";

/**
 * The recipe vocabulary, as the builder offers it.
 *
 * Each entry knows which parameters its operation takes, so the form renders
 * from data rather than from a switch with eighteen branches. Adding an
 * operation is one row here plus one arm in `buildRecipe`.
 */
export interface RecipeParam {
  key: string;
  label: string;
  kind: "field" | "text" | "number" | "bool" | "delimiter" | "special-mode";
  /** Default shown when the parameter is first added. */
  initial?: string | number | boolean;
}

export interface RecipeDef {
  op: Recipe["op"];
  label: string;
  category: "Casing" | "Field" | "Text" | "Number";
  params: RecipeParam[];
}

const FIELD = (key: string, label: string): RecipeParam => ({
  key,
  label,
  kind: "field",
});
const TEXT = (key: string, label: string, initial = ""): RecipeParam => ({
  key,
  label,
  kind: "text",
  initial,
});
const BOOL = (key: string, label: string, initial = false): RecipeParam => ({
  key,
  label,
  kind: "bool",
  initial,
});

export const RECIPE_DEFS: RecipeDef[] = [
  // Casing — the ignore list is what distinguishes these from Smart Fixes.
  {
    op: "to_upper_case",
    label: "To upper case",
    category: "Casing",
    params: [FIELD("field", "Field"), TEXT("ignore_words", "Words to ignore (comma-separated)")],
  },
  {
    op: "to_lower_case",
    label: "To lower case",
    category: "Casing",
    params: [FIELD("field", "Field"), TEXT("ignore_words", "Words to ignore (comma-separated)")],
  },
  {
    op: "to_title_case",
    label: "To title case",
    category: "Casing",
    params: [FIELD("field", "Field"), TEXT("ignore_words", "Words to ignore (comma-separated)")],
  },
  {
    op: "to_sentence_case",
    label: "To sentence case",
    category: "Casing",
    params: [FIELD("field", "Field")],
  },

  // Field
  {
    op: "copy_field",
    label: "Copy field",
    category: "Field",
    params: [FIELD("from", "From"), FIELD("to", "To")],
  },
  {
    op: "move_field",
    label: "Move field",
    category: "Field",
    params: [FIELD("from", "From"), FIELD("to", "To")],
  },
  {
    op: "merge_fields",
    label: "Merge fields",
    category: "Field",
    params: [
      FIELD("first", "First"),
      FIELD("second", "Second"),
      FIELD("target", "Into"),
      TEXT("separator", "Separator", " - "),
    ],
  },
  {
    op: "prefix_field",
    label: "Prefix field",
    category: "Field",
    params: [FIELD("field", "Field"), TEXT("text", "Prefix")],
  },
  {
    op: "suffix_field",
    label: "Suffix field",
    category: "Field",
    params: [FIELD("field", "Field"), TEXT("text", "Suffix")],
  },
  {
    op: "swap_fields",
    label: "Swap fields",
    category: "Field",
    params: [FIELD("first", "First"), FIELD("second", "Second")],
  },
  {
    op: "split_field",
    label: "Split field",
    category: "Field",
    params: [
      FIELD("field", "Field"),
      TEXT("delimiter", "Delimiter", " - "),
      FIELD("first_target", "First part into"),
      FIELD("second_target", "Second part into"),
      BOOL("preserve_split_text", "Keep delimiter on first part"),
      BOOL("append", "Append rather than overwrite"),
    ],
  },

  // Text
  {
    op: "remove_text",
    label: "Remove text",
    category: "Text",
    params: [
      FIELD("field", "Field"),
      TEXT("text", "Text to remove"),
      BOOL("case_insensitive", "Ignore case"),
    ],
  },
  {
    op: "replace_text",
    label: "Replace text",
    category: "Text",
    params: [
      FIELD("field", "Field"),
      TEXT("find", "Find"),
      TEXT("replace", "Replace with"),
      BOOL("case_insensitive", "Ignore case"),
    ],
  },
  {
    op: "extract_text",
    label: "Extract text",
    category: "Text",
    params: [
      FIELD("field", "From field"),
      TEXT("start", "Start delimiter", "("),
      TEXT("end", "End delimiter", ")"),
      FIELD("target", "Into"),
      BOOL("include_delimiters", "Include delimiters"),
      BOOL("delete_from_source", "Remove from source"),
      BOOL("append", "Append rather than overwrite"),
    ],
  },
  {
    op: "shorten_text",
    label: "Shorten text",
    category: "Text",
    params: [
      FIELD("field", "Field"),
      { key: "chars_per_word", label: "Characters per word", kind: "number", initial: 2 },
    ],
  },
  {
    op: "remove_special_characters",
    label: "Remove special characters",
    category: "Text",
    params: [
      FIELD("field", "Field"),
      { key: "mode", label: "Mode", kind: "special-mode", initial: "special" },
    ],
  },
  {
    op: "remove_between",
    label: "Remove between",
    category: "Text",
    params: [
      FIELD("field", "Field"),
      { key: "pair", label: "Delimiters", kind: "delimiter", initial: "parentheses" },
    ],
  },

  // Number
  {
    op: "adjust_number",
    label: "Increase / decrease number",
    category: "Number",
    params: [
      FIELD("field", "Field"),
      { key: "amount", label: "Amount (negative to decrease)", kind: "number", initial: 1 },
    ],
  },
];

export type ParamValues = Record<string, string | number | boolean>;

/** The starting parameter values for an operation. */
export function initialParams(def: RecipeDef, firstField: string): ParamValues {
  const out: ParamValues = {};
  for (const p of def.params) {
    if (p.initial !== undefined) out[p.key] = p.initial;
    else if (p.kind === "field") out[p.key] = firstField;
    else if (p.kind === "bool") out[p.key] = false;
    else if (p.kind === "number") out[p.key] = 0;
    else out[p.key] = "";
  }
  return out;
}

/**
 * Turn form values into the tagged union the backend deserialises.
 *
 * The comma-separated ignore list is split here rather than in the backend so
 * the wire format stays a real list — the backend should not be parsing UI
 * conventions.
 */
export function buildRecipe(op: Recipe["op"], values: ParamValues): Recipe {
  const str = (k: string) => String(values[k] ?? "");
  const num = (k: string) => Number(values[k] ?? 0);
  const bool = (k: string) => Boolean(values[k]);
  const words = (k: string) =>
    str(k)
      .split(",")
      .map((w) => w.trim())
      .filter((w) => w !== "");

  switch (op) {
    case "to_upper_case":
    case "to_lower_case":
    case "to_title_case":
      return { op, field: str("field"), ignore_words: words("ignore_words") };
    case "to_sentence_case":
      return { op, field: str("field") };
    case "copy_field":
    case "move_field":
      return { op, from: str("from"), to: str("to") };
    case "merge_fields":
      return {
        op,
        first: str("first"),
        second: str("second"),
        target: str("target"),
        separator: str("separator"),
      };
    case "prefix_field":
    case "suffix_field":
      return { op, field: str("field"), text: str("text") };
    case "swap_fields":
      return { op, first: str("first"), second: str("second") };
    case "split_field":
      return {
        op,
        field: str("field"),
        delimiter: str("delimiter"),
        first_target: str("first_target"),
        second_target: str("second_target"),
        preserve_split_text: bool("preserve_split_text"),
        append: bool("append"),
      };
    case "remove_text":
      return {
        op,
        field: str("field"),
        text: str("text"),
        case_insensitive: bool("case_insensitive"),
      };
    case "replace_text":
      return {
        op,
        field: str("field"),
        find: str("find"),
        replace: str("replace"),
        case_insensitive: bool("case_insensitive"),
      };
    case "extract_text":
      return {
        op,
        field: str("field"),
        start: str("start"),
        end: str("end"),
        target: str("target"),
        include_delimiters: bool("include_delimiters"),
        delete_from_source: bool("delete_from_source"),
        append: bool("append"),
      };
    case "shorten_text":
      return { op, field: str("field"), chars_per_word: num("chars_per_word") };
    case "remove_special_characters":
      return {
        op,
        field: str("field"),
        mode: str("mode") === "emojis" ? "emojis" : "special",
      };
    case "remove_between":
      return {
        op,
        field: str("field"),
        pair: str("pair") as Recipe extends { op: "remove_between"; pair: infer P }
          ? P
          : never,
      };
    case "adjust_number":
      return { op, field: str("field"), amount: num("amount") };
  }
}

/** A one-line description, for the recipe list. */
export function describeRecipe(recipe: Recipe): string {
  const def = RECIPE_DEFS.find((d) => d.op === recipe.op);
  const label = def?.label ?? recipe.op;
  const r = recipe as unknown as Record<string, unknown>;
  const target = r.field ?? r.from ?? r.first;
  return target ? `${label} — ${String(target)}` : label;
}
