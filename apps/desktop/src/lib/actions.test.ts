import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  bindingFromString,
  bindingMatches,
  bindingToString,
  findConflicts,
  findMatchingAction,
  formatBinding,
  fuzzyMatch,
  loadBindingOverrides,
  resolveBinding,
  saveBindingOverrides,
  searchActions,
  type ActionDef,
} from "./actions";

function action(over: Partial<ActionDef> & { id: string }): ActionDef {
  return {
    label: over.id,
    group: "Test",
    run: vi.fn(),
    ...over,
  };
}

function keyEvent(init: Partial<KeyboardEvent> & { key: string }): KeyboardEvent {
  return {
    key: init.key,
    metaKey: init.metaKey ?? false,
    ctrlKey: init.ctrlKey ?? false,
    shiftKey: init.shiftKey ?? false,
    altKey: init.altKey ?? false,
  } as KeyboardEvent;
}

beforeEach(() => {
  window.localStorage.clear();
});

describe("binding serialisation", () => {
  it("round-trips through string form", () => {
    const b = { key: "k", meta: true, shift: true, alt: false };
    expect(bindingFromString(bindingToString(b))).toEqual({
      key: "k",
      meta: true,
      shift: true,
      alt: false,
    });
  });

  it("rejects empty input", () => {
    expect(bindingFromString("")).toBeNull();
    expect(bindingFromString("   ")).toBeNull();
  });

  it("formats for display", () => {
    // Platform-dependent modifier glyphs, so assert on the key portion only.
    expect(formatBinding({ key: " " })).toBe("Space");
    expect(formatBinding({ key: "arrowleft" })).toBe("←");
    expect(formatBinding({ key: "escape" })).toBe("Esc");
    expect(formatBinding({ key: "k" })).toBe("K");
    expect(formatBinding(null)).toBe("");
  });
});

describe("bindingMatches", () => {
  it("treats Cmd and Ctrl as the same modifier", () => {
    const b = { key: "k", meta: true };
    expect(bindingMatches(b, keyEvent({ key: "k", metaKey: true }))).toBe(true);
    expect(bindingMatches(b, keyEvent({ key: "k", ctrlKey: true }))).toBe(true);
    expect(bindingMatches(b, keyEvent({ key: "k" }))).toBe(false);
  });

  it("requires every declared modifier and rejects extra ones", () => {
    const b = { key: "k" };
    expect(bindingMatches(b, keyEvent({ key: "k" }))).toBe(true);
    expect(bindingMatches(b, keyEvent({ key: "k", shiftKey: true }))).toBe(false);
    expect(bindingMatches(b, keyEvent({ key: "k", altKey: true }))).toBe(false);
  });

  it("is case-insensitive on the key", () => {
    expect(bindingMatches({ key: "k" }, keyEvent({ key: "K" }))).toBe(true);
  });
});

describe("overrides", () => {
  it("falls back to the default binding", () => {
    const a = action({ id: "a", defaultBinding: { key: "j" } });
    expect(resolveBinding(a, {})).toEqual({ key: "j" });
  });

  it("prefers a user override", () => {
    const a = action({ id: "a", defaultBinding: { key: "j" } });
    expect(resolveBinding(a, { a: "mod+k" })).toEqual({
      key: "k",
      meta: true,
      shift: false,
      alt: false,
    });
  });

  it("treats an explicit null override as deliberately unbound", () => {
    const a = action({ id: "a", defaultBinding: { key: "j" } });
    expect(resolveBinding(a, { a: null })).toBeNull();
  });

  it("persists and reloads", () => {
    saveBindingOverrides({ a: "mod+k" });
    expect(loadBindingOverrides()).toEqual({ a: "mod+k" });
  });

  it("recovers from malformed storage", () => {
    window.localStorage.setItem("decks.keybindings.v1", "not json");
    expect(loadBindingOverrides()).toEqual({});
  });
});

describe("findMatchingAction", () => {
  it("returns the first matching enabled action", () => {
    const actions = [
      action({ id: "a", defaultBinding: { key: "k" } }),
      action({ id: "b", defaultBinding: { key: "k" } }),
    ];
    expect(findMatchingAction(actions, {}, keyEvent({ key: "k" }), false)?.id).toBe("a");
  });

  it("skips disabled actions", () => {
    const actions = [
      action({ id: "a", defaultBinding: { key: "k" }, enabled: false }),
      action({ id: "b", defaultBinding: { key: "k" } }),
    ];
    expect(findMatchingAction(actions, {}, keyEvent({ key: "k" }), false)?.id).toBe("b");
  });

  it("skips unbound actions", () => {
    const actions = [action({ id: "a", defaultBinding: null })];
    expect(findMatchingAction(actions, {}, keyEvent({ key: "k" }), false)).toBeNull();
  });

  it("yields to editable targets unless the action opts in", () => {
    const actions = [
      action({ id: "a", defaultBinding: { key: "k" } }),
      action({ id: "b", defaultBinding: { key: "j" }, whenEditable: true }),
    ];
    expect(findMatchingAction(actions, {}, keyEvent({ key: "k" }), true)).toBeNull();
    expect(findMatchingAction(actions, {}, keyEvent({ key: "j" }), true)?.id).toBe("b");
  });

  it("always lets Escape through so dialogs can be dismissed", () => {
    const actions = [action({ id: "a", defaultBinding: { key: "escape" } })];
    expect(findMatchingAction(actions, {}, keyEvent({ key: "Escape" }), true)?.id).toBe(
      "a",
    );
  });

  it("honours a rebinding", () => {
    const actions = [action({ id: "a", defaultBinding: { key: "k" } })];
    expect(findMatchingAction(actions, { a: "j" }, keyEvent({ key: "k" }), false)).toBeNull();
    expect(findMatchingAction(actions, { a: "j" }, keyEvent({ key: "j" }), false)?.id).toBe(
      "a",
    );
  });
});

describe("findConflicts", () => {
  it("reports bindings claimed by more than one action", () => {
    const actions = [
      action({ id: "a", defaultBinding: { key: "k" } }),
      action({ id: "b", defaultBinding: { key: "k" } }),
      action({ id: "c", defaultBinding: { key: "j" } }),
    ];
    const conflicts = findConflicts(actions, {});
    expect(conflicts).toHaveLength(1);
    expect(conflicts[0].actionIds).toEqual(["a", "b"]);
  });

  it("is clean when a rebinding resolves the clash", () => {
    const actions = [
      action({ id: "a", defaultBinding: { key: "k" } }),
      action({ id: "b", defaultBinding: { key: "k" } }),
    ];
    expect(findConflicts(actions, { b: "j" })).toEqual([]);
  });
});

describe("search", () => {
  it("matches subsequences, not just prefixes", () => {
    expect(fuzzyMatch("pp", "Play / pause")).toBe(true);
    expect(fuzzyMatch("beatj", "Beat jump forward")).toBe(true);
    expect(fuzzyMatch("zzz", "Play / pause")).toBe(false);
    expect(fuzzyMatch("", "anything")).toBe(true);
  });

  it("excludes disabled and hidden actions from the palette", () => {
    const actions = [
      action({ id: "a", label: "Visible" }),
      action({ id: "b", label: "Disabled", enabled: false }),
      action({ id: "c", label: "Hidden", hidden: true }),
    ];
    expect(searchActions(actions, "").map((a) => a.id)).toEqual(["a"]);
  });

  it("searches the group name too", () => {
    const actions = [action({ id: "a", label: "Play", group: "Player" })];
    expect(searchActions(actions, "player")).toHaveLength(1);
  });
});
