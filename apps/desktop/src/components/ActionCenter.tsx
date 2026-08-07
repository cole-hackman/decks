import { useEffect, useMemo, useRef, useState } from "react";
import { formatBinding, resolveBinding, searchActions } from "../lib/actions";
import { useActions } from "../lib/action-context";

interface Props {
  open: boolean;
  onClose: () => void;
}

/**
 * The command palette. Anything bindable is reachable from here — that
 * equivalence is the point of the action registry, and it means new features
 * become discoverable without anyone adding a menu item.
 */
export function ActionCenter({ open, onClose }: Props) {
  const registry = useActions();
  const [query, setQuery] = useState("");
  const [highlight, setHighlight] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const results = useMemo(
    () => (registry ? searchActions(registry.actions, query) : []),
    [registry, query],
  );

  useEffect(() => {
    if (open) {
      setQuery("");
      setHighlight(0);
      // The ref is populated by the time this effect runs, so focus directly —
      // deferring to rAF would leave the palette unfocused in environments that
      // don't drive animation frames.
      inputRef.current?.focus();
    }
  }, [open]);

  // Keep the highlight inside the result set as the query narrows it.
  useEffect(() => {
    setHighlight((h) => (h >= results.length ? 0 : h));
  }, [results.length]);

  if (!open || !registry) return null;

  const runAt = (index: number) => {
    // Enter with nothing explicitly highlighted runs the first result, which is
    // what makes type-and-Enter feel instant.
    const action = results[index] ?? results[0];
    if (!action) return;
    onClose();
    action.run();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 pt-[15vh]"
      onClick={onClose}
      data-testid="action-center-backdrop"
    >
      <div
        role="dialog"
        aria-label="Action Center"
        className="w-full max-w-xl overflow-hidden rounded-lg border border-border bg-surface shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          aria-label="Run a command"
          placeholder="Run a command…"
          className="w-full border-b border-border bg-transparent px-4 py-3 text-sm outline-none"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setHighlight((h) => Math.min(h + 1, results.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setHighlight((h) => Math.max(h - 1, 0));
            } else if (e.key === "Enter") {
              e.preventDefault();
              runAt(highlight);
            } else if (e.key === "Escape") {
              e.preventDefault();
              onClose();
            }
          }}
        />

        <ul className="max-h-80 overflow-auto" role="listbox">
          {results.length === 0 && (
            <li className="px-4 py-6 text-center text-sm text-muted">
              No matching command.
            </li>
          )}
          {results.map((a, i) => {
            const binding = resolveBinding(a, registry.overrides);
            return (
              <li key={a.id}>
                <button
                  type="button"
                  role="option"
                  aria-selected={i === highlight}
                  className={`flex w-full items-center justify-between px-4 py-2 text-left text-sm ${
                    i === highlight ? "bg-surface-hover" : ""
                  }`}
                  onMouseEnter={() => setHighlight(i)}
                  onClick={() => runAt(i)}
                >
                  <span>
                    <span className="text-muted">{a.group}</span>
                    <span className="mx-1.5 text-muted">›</span>
                    <span>{a.label}</span>
                  </span>
                  {binding && (
                    <kbd className="rounded border border-border px-1.5 py-0.5 font-mono text-[11px] text-muted">
                      {formatBinding(binding)}
                    </kbd>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
}
