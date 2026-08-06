import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ActionContext, type ActionRegistry } from "../lib/action-context";
import {
  findMatchingAction,
  loadBindingOverrides,
  resolveBinding,
  saveBindingOverrides,
  type ActionDef,
  type Binding,
  type BindingOverrides,
} from "../lib/actions";

/**
 * Same rule as `useKeyboardShortcuts`: yield to anything the user might be
 * typing into, and to buttons/links that rely on Space and Enter themselves.
 */
function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (tag === "BUTTON" || tag === "A") return true;
  if (target.getAttribute("role") === "button") return true;
  if (target.isContentEditable) return true;
  return false;
}



/**
 * Owns the global key handler and the binding overrides.
 *
 * Actions are supplied by the app root rather than registered imperatively from
 * arbitrary components: a single declarative list keeps precedence
 * deterministic and makes the whole surface enumerable for the Action Center.
 */
export function ActionProvider({
  actions,
  children,
}: {
  actions: ActionDef[];
  children: ReactNode;
}) {
  const [overrides, setOverrides] = useState<BindingOverrides>(() =>
    loadBindingOverrides(),
  );

  // Keep the latest actions in a ref so the key listener is installed once
  // rather than being torn down and rebuilt whenever a handler closure changes.
  const actionsRef = useRef(actions);
  actionsRef.current = actions;
  const overridesRef = useRef(overrides);
  overridesRef.current = overrides;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const match = findMatchingAction(
        actionsRef.current,
        overridesRef.current,
        event,
        isEditable(event.target),
      );
      if (!match) return;
      event.preventDefault();
      match.run(event);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const rebind = useCallback((id: string, binding: Binding | null) => {
    setOverrides((prev) => {
      const next: BindingOverrides = {
        ...prev,
        [id]: binding === null ? null : bindingKey(binding),
      };
      saveBindingOverrides(next);
      return next;
    });
  }, []);

  const resetBinding = useCallback((id: string) => {
    setOverrides((prev) => {
      const next = { ...prev };
      delete next[id];
      saveBindingOverrides(next);
      return next;
    });
  }, []);

  const value = useMemo<ActionRegistry>(
    () => ({
      actions,
      overrides,
      bindingFor: (id) => {
        const a = actions.find((x) => x.id === id);
        return a ? resolveBinding(a, overrides) : null;
      },
      rebind,
      resetBinding,
      run: (id) => {
        const a = actions.find((x) => x.id === id);
        if (a && a.enabled !== false) a.run();
      },
    }),
    [actions, overrides, rebind, resetBinding],
  );

  return <ActionContext.Provider value={value}>{children}</ActionContext.Provider>;
}

function bindingKey(b: Binding): string {
  const parts: string[] = [];
  if (b.meta) parts.push("mod");
  if (b.shift) parts.push("shift");
  if (b.alt) parts.push("alt");
  parts.push(b.key.toLowerCase());
  return parts.join("+");
}

