import { useCallback, useEffect, useState } from "react";
import { listAutomaticActions, setAutomaticAction } from "../ipc";
import { useToast } from "./Toast";
import type { AutomaticAction } from "../types";

interface Props {
  className?: string;
}

/**
 * Automatic Actions — background behaviours the user opts into once.
 *
 * Actions decks cannot honour yet are shown **disabled with the reason**,
 * rather than hidden or offered as toggles that quietly do nothing. A switch
 * that does not switch anything is worse than a switch that says why it is off,
 * and hiding them would make the gap invisible.
 */
export function AutomaticActionsSection({ className }: Props) {
  const { toast } = useToast();
  const [actions, setActions] = useState<AutomaticAction[]>([]);

  const refresh = useCallback(async () => {
    try {
      const list = await listAutomaticActions();
      setActions(Array.isArray(list) ? list : []);
    } catch {
      setActions([]);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const toggle = useCallback(
    async (action: AutomaticAction, enabled: boolean) => {
      try {
        await setAutomaticAction(action.key, enabled);
        await refresh();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [refresh, toast],
  );

  return (
    <section className={className} aria-label="Automatic actions">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
        Automatic Actions
      </h3>
      <p className="mb-3 text-[11px] text-ink-faint">
        These apply to tracks you bring in — never to tracks that came from
        Rekordbox.
      </p>

      <ul className="space-y-3">
        {actions.map((a) => (
          <li key={a.key}>
            <label className="flex items-start gap-2 text-xs">
              <input
                type="checkbox"
                className="mt-0.5"
                checked={a.enabled}
                disabled={a.unavailable != null}
                aria-label={a.label}
                onChange={(e) => void toggle(a, e.target.checked)}
              />
              <span className="min-w-0">
                <span
                  className={a.unavailable != null ? "text-ink-muted" : "text-ink"}
                >
                  {a.label}
                </span>
                <span className="block text-[11px] text-ink-faint">
                  {a.description}
                </span>
                {a.unavailable != null && (
                  <span
                    className="block text-[11px] text-amber-500"
                    data-testid={`unavailable-${a.key}`}
                  >
                    {a.unavailable}
                  </span>
                )}
              </span>
            </label>
          </li>
        ))}
      </ul>
    </section>
  );
}
