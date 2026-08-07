import { createContext, useContext } from "react";
import type { ActionDef, Binding, BindingOverrides } from "./actions";

export interface ActionRegistry {
  actions: ActionDef[];
  overrides: BindingOverrides;
  bindingFor: (id: string) => Binding | null;
  rebind: (id: string, binding: Binding | null) => void;
  resetBinding: (id: string) => void;
  run: (id: string) => void;
}

export const ActionContext = createContext<ActionRegistry | null>(null);

/** Access the registry. Returns null outside a provider, so components rendered
 *  standalone in tests don't have to wrap themselves. */
export function useActions(): ActionRegistry | null {
  return useContext(ActionContext);
}

/** The binding in force for an action, for inline hints. Null when unbound. */
export function useBindingHint(id: string): Binding | null {
  const reg = useActions();
  return reg?.bindingFor(id) ?? null;
}
