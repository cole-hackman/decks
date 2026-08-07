import { useCallback, useEffect, useState } from "react";
import {
  createPathMapping,
  deletePathMapping,
  listPathMappings,
  previewPathMapping,
} from "../ipc";
import { useToast } from "./Toast";
import type { PathMappingRow } from "../types";

interface Props {
  /** Matches the padding the surrounding settings sections use. */
  className?: string;
}

/**
 * Local Path Mappings.
 *
 * Per-computer prefix rewrites, so a library restored on a second machine finds
 * its music without a bulk relocate. Read-side only: the library keeps saying
 * `D:\Music\…`, which is what lets the same database work on both machines at
 * once. Nothing here is staged, exported or synced.
 */
export function PathMappingsSection({ className }: Props) {
  const { toast } = useToast();
  const [rows, setRows] = useState<PathMappingRow[]>([]);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [testPath, setTestPath] = useState("");
  const [testResult, setTestResult] = useState<[string, boolean] | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await listPathMappings();
      setRows(Array.isArray(list) ? list : []);
    } catch {
      setRows([]);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const add = useCallback(async () => {
    try {
      await createPathMapping(from, to);
      setFrom("");
      setTo("");
      await refresh();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  }, [from, to, refresh, toast]);

  const remove = useCallback(
    async (id: string) => {
      try {
        await deletePathMapping(id);
        await refresh();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [refresh, toast],
  );

  const test = useCallback(async () => {
    try {
      setTestResult(await previewPathMapping(testPath));
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    }
  }, [testPath, toast]);

  return (
    <section className={className} aria-label="Local path mappings">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
        Local Path Mappings
      </h3>
      <p className="mb-3 text-[11px] text-ink-faint">
        Rewrites a stored path prefix for this computer, so a library from another
        machine finds its music. The library itself is not changed — the same
        database keeps working on both machines. The longest matching prefix wins.
      </p>

      {rows.length === 0 ? (
        <p className="mb-3 text-xs text-ink-secondary" data-testid="no-mappings">
          No mappings. Paths are used exactly as the library stores them.
        </p>
      ) : (
        <ul className="mb-3 space-y-1 text-xs">
          {rows.map((r) => (
            <li key={r.id} className="flex items-center gap-2">
              <span className="font-mono">{r.from}</span>
              <span className="text-ink-faint">→</span>
              <span className="font-mono">{r.to}</span>
              <button
                type="button"
                aria-label={`Remove mapping ${r.from}`}
                className="ml-auto text-ink-muted hover:text-red-400"
                onClick={() => void remove(r.id)}
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="mb-3 flex flex-wrap items-end gap-2 text-xs">
        <label className="flex-1">
          <span className="mb-1 block text-ink-secondary">Stored prefix</span>
          <input
            className="w-full rounded-md border border-edge-strong bg-surface px-2 py-1 font-mono text-xs"
            placeholder="D:\Music"
            value={from}
            onChange={(e) => setFrom(e.target.value)}
          />
        </label>
        <label className="flex-1">
          <span className="mb-1 block text-ink-secondary">On this computer</span>
          <input
            className="w-full rounded-md border border-edge-strong bg-surface px-2 py-1 font-mono text-xs"
            placeholder="/Users/you/Music"
            value={to}
            onChange={(e) => setTo(e.target.value)}
          />
        </label>
        <button
          type="button"
          disabled={from.trim() === "" || to.trim() === ""}
          className="rounded-md border border-edge-strong px-3 py-1 hover:bg-elevated disabled:opacity-50"
          onClick={() => void add()}
        >
          Add
        </button>
      </div>

      <div className="flex flex-wrap items-end gap-2 text-xs">
        <label className="flex-1">
          <span className="mb-1 block text-ink-secondary">Test a stored path</span>
          <input
            className="w-full rounded-md border border-edge-strong bg-surface px-2 py-1 font-mono text-xs"
            placeholder="D:\Music\House\track.mp3"
            value={testPath}
            onChange={(e) => setTestPath(e.target.value)}
          />
        </label>
        <button
          type="button"
          disabled={testPath.trim() === ""}
          className="rounded-md border border-edge-strong px-3 py-1 hover:bg-elevated disabled:opacity-50"
          onClick={() => void test()}
        >
          Test
        </button>
      </div>
      {testResult && (
        <p
          className={`mt-2 font-mono text-[11px] ${testResult[1] ? "text-emerald-400" : "text-amber-500"}`}
          data-testid="mapping-test-result"
        >
          {testResult[0]} — {testResult[1] ? "file found" : "no file there"}
        </p>
      )}
    </section>
  );
}
