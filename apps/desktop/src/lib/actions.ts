/**
 * The action registry.
 *
 * One idea, reused everywhere: **every global capability is a named, bindable
 * command**. The Action Center searches this list, hotkeys bind to it, the
 * inline shortcut hints read from it, and a future plugin host registers into
 * it. Lexicon works the same way — its command palette explicitly offers
 * "anything that can be bound to a hotkey".
 *
 * Component-local key handling (arrow-key navigation inside the track table,
 * for instance) deliberately stays in `useKeyboardShortcuts`. The registry is
 * for *application* actions, not for widget-internal keyboard behaviour.
 */

export interface Binding {
  /** Lower-case `KeyboardEvent.key`, e.g. "k", "1", "arrowleft", " ". */
  key: string;
  /** Cmd on macOS, Ctrl elsewhere — matched interchangeably. */
  meta?: boolean;
  shift?: boolean;
  alt?: boolean;
}

export interface ActionDef {
  /** Stable identifier, e.g. `player.playPause`. Used as the persistence key
   *  for rebinding, so it must not change once shipped. */
  id: string;
  label: string;
  /** Grouping for the Action Center. */
  group: string;
  defaultBinding?: Binding | null;
  /** Fire even while the user is typing in an input. Rare. */
  whenEditable?: boolean;
  /** Hidden from the palette and inert as a binding when false. Defaults true. */
  enabled?: boolean;
  /** Keep out of the Action Center while still bindable — for actions that only
   *  make sense as a keystroke, like "delete cue 3". */
  hidden?: boolean;
  run: (event?: KeyboardEvent) => void;
}

const STORAGE_KEY = "decks.keybindings.v1";

const IS_MAC =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/** Serialised binding form used both in storage and for display lookup. */
export function bindingToString(b: Binding): string {
  const parts: string[] = [];
  if (b.meta) parts.push("mod");
  if (b.shift) parts.push("shift");
  if (b.alt) parts.push("alt");
  parts.push(b.key.toLowerCase());
  return parts.join("+");
}

export function bindingFromString(s: string): Binding | null {
  const parts = s.split("+").map((p) => p.trim().toLowerCase()).filter(Boolean);
  if (parts.length === 0) return null;
  const key = parts[parts.length - 1];
  if (!key) return null;
  return {
    key,
    meta: parts.includes("mod"),
    shift: parts.includes("shift"),
    alt: parts.includes("alt"),
  };
}

const KEY_DISPLAY: Record<string, string> = {
  " ": "Space",
  arrowleft: "←",
  arrowright: "→",
  arrowup: "↑",
  arrowdown: "↓",
  escape: "Esc",
  enter: "↵",
};

/** Human-readable form for inline hints and the palette. */
export function formatBinding(b: Binding | null | undefined): string {
  if (!b) return "";
  const parts: string[] = [];
  if (b.meta) parts.push(IS_MAC ? "⌘" : "Ctrl");
  if (b.shift) parts.push(IS_MAC ? "⇧" : "Shift");
  if (b.alt) parts.push(IS_MAC ? "⌥" : "Alt");
  const key = KEY_DISPLAY[b.key] ?? b.key.toUpperCase();
  parts.push(key);
  return IS_MAC ? parts.join("") : parts.join("+");
}

/**
 * Whether a keyboard event satisfies a binding.
 *
 * Cmd and Ctrl are treated as the same modifier so one binding table works on
 * both platforms — matching how the rest of the app already reads modifiers.
 */
export function bindingMatches(b: Binding, event: KeyboardEvent): boolean {
  if (b.key.toLowerCase() !== event.key.toLowerCase()) return false;
  if (!!b.meta !== (event.metaKey || event.ctrlKey)) return false;
  if (!!b.shift !== event.shiftKey) return false;
  if (!!b.alt !== event.altKey) return false;
  return true;
}

export type BindingOverrides = Record<string, string | null>;

/** User rebindings, keyed by action id. `null` means "unbound deliberately". */
export function loadBindingOverrides(): BindingOverrides {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return {};
    return parsed as BindingOverrides;
  } catch {
    return {};
  }
}

export function saveBindingOverrides(overrides: BindingOverrides): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(overrides));
  } catch {
    // Storage full or disabled — rebinding silently reverts to defaults.
  }
}

/** The binding actually in force for an action. */
export function resolveBinding(
  action: ActionDef,
  overrides: BindingOverrides,
): Binding | null {
  if (Object.prototype.hasOwnProperty.call(overrides, action.id)) {
    const raw = overrides[action.id];
    return raw === null ? null : bindingFromString(raw);
  }
  return action.defaultBinding ?? null;
}

/**
 * The first action whose binding matches, respecting enablement and the
 * editable-target rule.
 *
 * Registration order is precedence order: an action registered earlier wins a
 * conflicting binding. Callers that care should surface conflicts in settings
 * rather than relying on this, but a deterministic rule beats an arbitrary one.
 */
export function findMatchingAction(
  actions: ActionDef[],
  overrides: BindingOverrides,
  event: KeyboardEvent,
  targetIsEditable: boolean,
): ActionDef | null {
  for (const action of actions) {
    if (action.enabled === false) continue;
    const binding = resolveBinding(action, overrides);
    if (!binding) continue;
    if (!bindingMatches(binding, event)) continue;
    // Escape always fires so dialogs can always be dismissed.
    if (targetIsEditable && !action.whenEditable && binding.key !== "escape") {
      continue;
    }
    return action;
  }
  return null;
}

/** Detect binding collisions so settings can warn rather than silently shadow. */
export function findConflicts(
  actions: ActionDef[],
  overrides: BindingOverrides,
): Array<{ binding: string; actionIds: string[] }> {
  const byBinding = new Map<string, string[]>();
  for (const action of actions) {
    const b = resolveBinding(action, overrides);
    if (!b) continue;
    const key = bindingToString(b);
    byBinding.set(key, [...(byBinding.get(key) ?? []), action.id]);
  }
  return [...byBinding.entries()]
    .filter(([, ids]) => ids.length > 1)
    .map(([binding, actionIds]) => ({ binding, actionIds }));
}

/** Case-insensitive subsequence match, so "pp" finds "Play / pause". */
export function fuzzyMatch(query: string, text: string): boolean {
  const q = query.trim().toLowerCase();
  if (q === "") return true;
  const t = text.toLowerCase();
  let i = 0;
  for (const ch of t) {
    if (ch === q[i]) i++;
    if (i === q.length) return true;
  }
  return false;
}

/** Actions visible in the Action Center for a query. */
export function searchActions(actions: ActionDef[], query: string): ActionDef[] {
  return actions.filter(
    (a) =>
      a.enabled !== false &&
      !a.hidden &&
      (fuzzyMatch(query, a.label) || fuzzyMatch(query, `${a.group} ${a.label}`)),
  );
}
