import { useEffect } from "react";

/**
 * Treat a target as "interactive" if a global shortcut should yield to it.
 * Inputs/selects/contenteditable obviously qualify. Buttons and links also
 * legitimately rely on Space/Enter for activation, so a global Space binding
 * that doesn't exclude them would steal their click.
 */
function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (tag === "BUTTON" || tag === "A") return true;
  if (target.getAttribute("role") === "button") return true;
  if (target.isContentEditable) return true;
  // A focused grid owns the arrows, Home/End, the page keys and printable
  // characters for the same reason an input does — it is a spreadsheet, and a
  // global binding firing alongside its cell cursor would move twice.
  if (target.closest('[role="grid"]')) return true;
  return false;
}

export interface Shortcut {
  /** Lower-case key, e.g. "j", "/", "escape". */
  key: string;
  /** Required modifier — leave undefined for no modifier. */
  meta?: boolean;
  shift?: boolean;
  /** When true, the handler fires even while typing in an input/textarea.
   *  Default false. Escape is special-cased separately. */
  whenEditable?: boolean;
  handler: (event: KeyboardEvent) => void;
}

/**
 * Register a set of global keyboard shortcuts. Each shortcut is matched
 * against the event's `key` (case-insensitive) plus optional modifiers.
 *
 * Shortcuts are ignored when the user is typing in an editable element,
 * unless `whenEditable` is true. Escape always fires regardless.
 */
export function useKeyboardShortcuts(shortcuts: Shortcut[]) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const key = event.key.toLowerCase();
      const editable = isEditable(event.target);

      for (const s of shortcuts) {
        if (s.key.toLowerCase() !== key) continue;
        if (!!s.meta !== (event.metaKey || event.ctrlKey)) continue;
        if (!!s.shift !== event.shiftKey) continue;
        // Escape always fires; otherwise respect editable
        if (editable && !s.whenEditable && key !== "escape") continue;
        s.handler(event);
        return;
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [shortcuts]);
}
