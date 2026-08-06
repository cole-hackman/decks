import { useCallback, useEffect, useState } from "react";
import {
  commonTextBlocklistAdd,
  commonTextBlocklistList,
  commonTextBlocklistRemove,
} from "../ipc";
import { useToast } from "./Toast";

interface Props {
  /** Matches the padding the surrounding settings sections use. */
  className?: string;
}

/**
 * The two presets the manual calls out by name.
 *
 * Offered rather than seeded: a blocklist that arrives pre-populated will
 * eventually strip something a user wanted, and they will not know where it
 * came from. One click each, and the button says exactly what it adds.
 */
const PRESETS: { label: string; patterns: string[] }[] = [
  { label: "(Original Mix)", patterns: ["(Original Mix)"] },
  {
    label: "Camelot keys",
    patterns: [
      ...Array.from({ length: 12 }, (_, i) => `${i + 1}A`),
      ...Array.from({ length: 12 }, (_, i) => `${i + 1}B`),
    ],
  },
];

/**
 * Remove Common Text blocklist.
 *
 * The one Smart Fix that needs configuring: it strips whatever is on this list
 * from Title, Artist, Album and Comment. Matching is case-insensitive, and the
 * fix still previews every proposal as a deselectable row — nothing here writes
 * anything on its own.
 */
export function CommonTextSection({ className }: Props) {
  const { toast } = useToast();
  const [patterns, setPatterns] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const list = await commonTextBlocklistList();
      setPatterns(Array.isArray(list) ? list : []);
    } catch {
      setPatterns([]);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const add = useCallback(
    async (values: string[]) => {
      // Adding one that is already there would give the fix a duplicate to
      // apply twice, and the list a row the user cannot tell apart.
      const fresh = values
        .map((v) => v.trim())
        .filter((v) => v !== "")
        .filter(
          (v) => !patterns.some((p) => p.toLowerCase() === v.toLowerCase()),
        );
      if (fresh.length === 0) return;
      setBusy(true);
      try {
        for (const value of fresh) await commonTextBlocklistAdd(value);
        setDraft("");
        await refresh();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [patterns, refresh, toast],
  );

  const remove = useCallback(
    async (pattern: string) => {
      setBusy(true);
      try {
        await commonTextBlocklistRemove(pattern);
        await refresh();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [refresh, toast],
  );

  return (
    <section className={className} aria-label="Remove common text">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
        Remove Common Text
      </h3>
      <p className="mb-3 text-[11px] text-ink-faint">
        The Remove Common Text smart fix strips these from Title, Artist, Album
        and Comment. Matching ignores case, and every proposal is still
        reviewable before it is staged.
      </p>

      {patterns.length === 0 ? (
        <p className="mb-3 text-xs text-ink-secondary" data-testid="no-common-text">
          Nothing on the list, so Remove Common Text proposes nothing.
        </p>
      ) : (
        <ul className="mb-3 space-y-1 text-xs">
          {patterns.map((p) => (
            <li key={p} className="flex items-center gap-2">
              <code className="rounded bg-elevated px-1.5 py-0.5">{p}</code>
              <button
                type="button"
                disabled={busy}
                aria-label={`Remove ${p}`}
                className="ml-auto text-ink-muted hover:text-red-400 disabled:opacity-50"
                onClick={() => void remove(p)}
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="mb-2 flex flex-wrap items-end gap-2 text-xs">
        <label className="flex-1">
          <span className="mb-1 block text-ink-secondary">Text to remove</span>
          <input
            aria-label="Text to remove"
            className="w-full rounded-md border border-edge-strong bg-surface px-2 py-1 text-xs"
            placeholder="(Original Mix)"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void add([draft]);
            }}
          />
        </label>
        <button
          type="button"
          disabled={busy || draft.trim() === ""}
          className="rounded-md border border-edge-strong px-3 py-1 hover:bg-elevated disabled:opacity-50"
          onClick={() => void add([draft])}
        >
          Add
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-2 text-[11px]">
        <span className="text-ink-faint">Presets:</span>
        {PRESETS.map((preset) => (
          <button
            key={preset.label}
            type="button"
            disabled={busy}
            className="rounded-md border border-edge-strong px-2 py-0.5 hover:bg-elevated disabled:opacity-50"
            onClick={() => void add(preset.patterns)}
          >
            Add {preset.label}
          </button>
        ))}
      </div>
    </section>
  );
}
