import { describe, expect, it } from "vitest";
import {
  RECIPE_DEFS,
  buildRecipe,
  describeRecipe,
  initialParams,
} from "./recipe-forms";

describe("recipe-forms", () => {
  it("every definition builds a recipe whose op matches", () => {
    // Guards the switch in buildRecipe against drifting from RECIPE_DEFS.
    for (const def of RECIPE_DEFS) {
      const values = initialParams(def, "title");
      const recipe = buildRecipe(def.op, values);
      expect(recipe.op).toBe(def.op);
    }
  });

  it("field parameters default to the first available field", () => {
    const def = RECIPE_DEFS.find((d) => d.op === "copy_field")!;
    const values = initialParams(def, "artist");
    expect(values.from).toBe("artist");
    expect(values.to).toBe("artist");
  });

  it("splits the comma-separated ignore list into a real list", () => {
    // The backend should not be parsing UI conventions.
    const recipe = buildRecipe("to_title_case", {
      field: "title",
      ignore_words: "EDM, DJ ,, NYC",
    });
    expect(recipe).toEqual({
      op: "to_title_case",
      field: "title",
      ignore_words: ["EDM", "DJ", "NYC"],
    });
  });

  it("an empty ignore list becomes an empty array, not [''] ", () => {
    const recipe = buildRecipe("to_upper_case", { field: "title", ignore_words: "" });
    expect(recipe).toEqual({
      op: "to_upper_case",
      field: "title",
      ignore_words: [],
    });
  });

  it("number parameters are numbers on the wire", () => {
    const recipe = buildRecipe("adjust_number", { field: "year", amount: "-1" });
    expect(recipe).toEqual({ op: "adjust_number", field: "year", amount: -1 });
  });

  it("booleans default to false rather than undefined", () => {
    const def = RECIPE_DEFS.find((d) => d.op === "split_field")!;
    const values = initialParams(def, "title");
    expect(values.preserve_split_text).toBe(false);
    expect(values.append).toBe(false);
  });

  it("carries the documented defaults through", () => {
    const def = RECIPE_DEFS.find((d) => d.op === "extract_text")!;
    const values = initialParams(def, "title");
    expect(values.start).toBe("(");
    expect(values.end).toBe(")");
  });

  it("an unknown special-character mode falls back to 'special'", () => {
    const recipe = buildRecipe("remove_special_characters", {
      field: "title",
      mode: "nonsense",
    });
    expect(recipe).toMatchObject({ mode: "special" });
  });

  it("describes a recipe with its target field", () => {
    expect(
      describeRecipe({ op: "to_title_case", field: "artist", ignore_words: [] }),
    ).toBe("To title case — artist");
    expect(describeRecipe({ op: "copy_field", from: "title", to: "comment" })).toBe(
      "Copy field — title",
    );
  });

  it("covers all four categories", () => {
    const categories = new Set(RECIPE_DEFS.map((d) => d.category));
    expect([...categories].sort()).toEqual(["Casing", "Field", "Number", "Text"]);
  });
});
