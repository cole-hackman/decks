import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyCuePreset,
  beatJumpPosition,
  createCuePreset,
  deleteCuePreset,
  listCuePresets,
  stageCueAdd,
  stageCueDelete,
  stageCueEdit,
  stageGridShift,
} from "../ipc";
import { useToast } from "./Toast";
import type { CueKind, CuePreset, HotCue, QuantizeResolution, Track } from "../types";

const HOT_SLOTS = [1, 2, 3, 4, 5, 6, 7, 8] as const;

const RESOLUTIONS: Array<{ value: QuantizeResolution; label: string }> = [
  { value: "beat", label: "1 beat" },
  { value: "two_beats", label: "2 beats" },
  { value: "bar", label: "1 bar" },
  { value: "four_bars", label: "4 bars" },
  { value: "sixteen_bars", label: "16 bars" },
];

/** Rekordbox colour IDs, in the order the picker shows them. `-1` is unset. */
const COLORS: Array<{ id: number; label: string; className: string }> = [
  { id: -1, label: "None", className: "bg-transparent border border-border" },
  { id: 0, label: "Pink", className: "bg-pink-500" },
  { id: 1, label: "Red", className: "bg-red-500" },
  { id: 2, label: "Orange", className: "bg-orange-500" },
  { id: 3, label: "Yellow", className: "bg-yellow-400" },
  { id: 4, label: "Green", className: "bg-green-500" },
  { id: 5, label: "Blue", className: "bg-blue-500" },
  { id: 6, label: "Violet", className: "bg-violet-500" },
];

function slotOf(kind: CueKind): number {
  return kind === "MemoryCue" ? 0 : kind.HotCue;
}

function formatMs(ms: number | null | undefined): string {
  if (ms == null) return "—";
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = String(total % 60).padStart(2, "0");
  const cs = String(Math.floor((ms % 1000) / 10)).padStart(2, "0");
  return `${m}:${s}.${cs}`;
}

interface Props {
  libraryPath: string;
  track: Track;
  cues: HotCue[];
  /** Current playhead position in ms — where new cues land. */
  positionMs: number;
  onSeek?: (ms: number) => void;
  /** Refetch cues after a change is staged. */
  onChanged?: () => void;
}

/**
 * Cue and loop editing.
 *
 * Every operation stages a change rather than writing `master.db` — cue edits
 * on a library you perform from are exactly what the staged-change pipeline
 * exists for, so they land in the Changes review like everything else.
 */
export function CueEditor({
  libraryPath,
  track,
  cues,
  positionMs,
  onSeek,
  onChanged,
}: Props) {
  const { toast } = useToast();
  const [quantizeOn, setQuantizeOn] = useState(true);
  const [resolution, setResolution] = useState<QuantizeResolution>("beat");
  const [busy, setBusy] = useState(false);
  /**
   * Saved name+colour presets — the spec's "Cue templates".
   *
   * Renamed because `crates/cue-generator` already owns `CueTemplate` for its
   * bulk-generation rule sets, and two things called "template" in one player
   * would be unreadable.
   */
  const [presets, setPresets] = useState<CuePreset[]>([]);
  /** The cue a preset click will be stamped onto. */
  const [presetTarget, setPresetTarget] = useState<string | null>(null);

  const refreshPresets = useCallback(async () => {
    try {
      setPresets(await listCuePresets());
    } catch {
      // A preset list that fails to load must not take the cue editor with it.
      setPresets([]);
    }
  }, []);

  useEffect(() => {
    void refreshPresets();
  }, [refreshPresets]);

  const targetCue = cues.find((c) => c.id === presetTarget) ?? null;

  const applyPreset = useCallback(
    async (preset: CuePreset) => {
      if (!targetCue) return;
      setBusy(true);
      try {
        const staged = await applyCuePreset(
          libraryPath,
          targetCue.id,
          preset.id,
          targetCue.comment,
          targetCue.color,
        );
        toast({
          variant: staged.length > 0 ? "success" : "info",
          message:
            staged.length > 0
              ? `Staged ${staged.length} change(s) from “${preset.name}”.`
              : `That cue already matches “${preset.name}”.`,
        });
        onChanged?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [targetCue, libraryPath, onChanged, toast],
  );

  /** Promote a cue's own name and colour into a reusable preset. */
  const promoteCue = useCallback(
    async (cue: HotCue) => {
      const name = cue.comment?.trim();
      if (!name) {
        toast({
          variant: "error",
          message: "Name the cue first",
          detail: "A preset is a name and a colour; there is nothing to save yet.",
        });
        return;
      }
      try {
        await createCuePreset(name, cue.color);
        await refreshPresets();
        toast({ variant: "success", message: `Saved preset “${name}”.` });
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [refreshPresets, toast],
  );

  const removePreset = useCallback(
    async (preset: CuePreset) => {
      try {
        await deleteCuePreset(preset.id);
        await refreshPresets();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [refreshPresets, toast],
  );

  const bySlot = useMemo(() => {
    const map = new Map<number, HotCue>();
    for (const c of cues) {
      const slot = slotOf(c.kind);
      if (slot >= 1 && slot <= 8 && !map.has(slot)) map.set(slot, c);
    }
    return map;
  }, [cues]);

  const quantizeArg = quantizeOn ? resolution : null;

  const setOrPlayCue = useCallback(
    async (slot: number) => {
      const existing = bySlot.get(slot);
      if (existing) {
        // Occupied slot: play it. Matches the documented 1–8 behaviour —
        // set if empty, jump if set.
        if (existing.in_msec != null) onSeek?.(existing.in_msec);
        return;
      }
      setBusy(true);
      try {
        await stageCueAdd(
          libraryPath,
          track.id,
          { in_msec: Math.round(positionMs), kind: slot },
          quantizeArg,
        );
        toast({ variant: "success", message: `Staged cue ${slot}.` });
        onChanged?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [bySlot, libraryPath, track.id, positionMs, quantizeArg, onSeek, onChanged, toast],
  );

  const deleteCue = useCallback(
    async (cue: HotCue) => {
      setBusy(true);
      try {
        await stageCueDelete(libraryPath, cue.id);
        toast({ variant: "success", message: "Staged cue deletion." });
        onChanged?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [libraryPath, onChanged, toast],
  );

  const moveCueToPlayhead = useCallback(
    async (cue: HotCue) => {
      setBusy(true);
      try {
        await stageCueEdit(
          libraryPath,
          cue.id,
          "InMsec",
          Math.round(positionMs),
          cue.in_msec,
        );
        toast({ variant: "success", message: "Staged cue move." });
        onChanged?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [libraryPath, positionMs, onChanged, toast],
  );

  const setColor = useCallback(
    async (cue: HotCue, color: number) => {
      try {
        await stageCueEdit(libraryPath, cue.id, "Color", color, cue.color);
        onChanged?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [libraryPath, onChanged, toast],
  );

  const makeLoop = useCallback(
    async (cue: HotCue, beats: number) => {
      if (cue.in_msec == null) return;
      const bpm = track.bpm ?? 0;
      if (bpm <= 0) {
        toast({
          variant: "error",
          message: "Track has no BPM — analyse it before making loops.",
        });
        return;
      }
      const lengthMs = Math.round((beats * 60_000) / bpm);
      try {
        await stageCueEdit(
          libraryPath,
          cue.id,
          "OutMsec",
          cue.in_msec + lengthMs,
          cue.out_msec,
        );
        toast({ variant: "success", message: `Staged ${beats}-beat loop.` });
        onChanged?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [libraryPath, track.bpm, onChanged, toast],
  );

  const nudgeGrid = useCallback(
    async (offsetMs: number) => {
      setBusy(true);
      try {
        const staged = await stageGridShift(libraryPath, track.id, offsetMs);
        toast({
          variant: "success",
          message:
            staged.length === 0
              ? "No on-grid cues to move."
              : `Staged ${staged.length} cue move(s) following the grid.`,
        });
        onChanged?.();
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      } finally {
        setBusy(false);
      }
    },
    [libraryPath, track.id, onChanged, toast],
  );

  const jump = useCallback(
    async (beats: number) => {
      try {
        const next = await beatJumpPosition(
          libraryPath,
          track.id,
          Math.round(positionMs),
          beats,
        );
        onSeek?.(next);
      } catch (e) {
        toast({ variant: "error", message: String(e) });
      }
    },
    [libraryPath, track.id, positionMs, onSeek, toast],
  );

  // Cue hotkeys are scoped to this panel rather than registered globally:
  // "1" should set a cue only while the editor is on screen.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const target = e.target;
      if (
        target instanceof HTMLElement &&
        ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
      ) {
        return;
      }
      const n = Number(e.key);
      if (Number.isInteger(n) && n >= 1 && n <= 8) {
        e.preventDefault();
        if (e.metaKey || e.ctrlKey) {
          const existing = bySlot.get(n);
          if (existing) void deleteCue(existing);
        } else {
          void setOrPlayCue(n);
        }
        return;
      }
      if (e.key.toLowerCase() === "q" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        setQuantizeOn((v) => !v);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [bySlot, setOrPlayCue, deleteCue]);

  return (
    <section className="border-t border-border px-4 py-3" aria-label="Cue editor">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-muted">
          Cues
        </h3>
        <div className="flex items-center gap-2 text-xs">
          <label className="flex items-center gap-1">
            <input
              type="checkbox"
              checked={quantizeOn}
              onChange={(e) => setQuantizeOn(e.target.checked)}
              aria-label="Quantize"
            />
            Quantize
          </label>
          <select
            aria-label="Quantize resolution"
            className="rounded border border-border bg-surface px-1 py-0.5"
            value={resolution}
            disabled={!quantizeOn}
            onChange={(e) => setResolution(e.target.value as QuantizeResolution)}
          >
            {RESOLUTIONS.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="mb-3 grid grid-cols-8 gap-1">
        {HOT_SLOTS.map((slot) => {
          const cue = bySlot.get(slot);
          return (
            <button
              key={slot}
              type="button"
              disabled={busy}
              aria-label={cue ? `Play cue ${slot}` : `Set cue ${slot}`}
              title={cue ? formatMs(cue.in_msec) : "Empty — click to set"}
              className={`rounded px-1 py-2 text-xs font-mono ${
                cue
                  ? "bg-accent/20 text-accent-hover"
                  : "border border-dashed border-border text-muted"
              }`}
              onClick={() => void setOrPlayCue(slot)}
            >
              {slot}
            </button>
          );
        })}
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2 text-xs">
        <span className="text-muted">Jump</span>
        {[-16, -4, 4, 16].map((b) => (
          <button
            key={b}
            type="button"
            aria-label={`Beat jump ${b}`}
            className="rounded border border-border px-2 py-0.5 hover:bg-surface-hover"
            onClick={() => void jump(b)}
          >
            {b > 0 ? `+${b}` : b}
          </button>
        ))}
        <span className="ml-2 text-muted">Grid</span>
        {[-10, -1, 1, 10].map((ms) => (
          <button
            key={ms}
            type="button"
            aria-label={`Nudge grid ${ms}ms`}
            className="rounded border border-border px-2 py-0.5 hover:bg-surface-hover"
            onClick={() => void nudgeGrid(ms)}
          >
            {ms > 0 ? `+${ms}ms` : `${ms}ms`}
          </button>
        ))}
      </div>

      <section className="mt-3 rounded border border-border p-2" aria-label="Cue presets">
        <div className="flex items-center justify-between text-xs">
          <h3 className="font-medium">Presets</h3>
          <span className="text-muted" data-testid="preset-target">
            {targetCue
              ? `Applying to cue ${slotOf(targetCue.kind) === 0 ? "M" : slotOf(targetCue.kind)}`
              : "Pick a cue below to apply one"}
          </span>
        </div>
        {presets.length === 0 ? (
          <p className="mt-1 text-xs text-muted">
            No presets yet. Name a cue, then press <em>Save preset</em> on it.
          </p>
        ) : (
          <ul className="mt-1 flex flex-wrap gap-1" data-testid="preset-list">
            {presets.map((preset) => (
              <li key={preset.id} className="flex items-center">
                <button
                  type="button"
                  disabled={!targetCue || busy}
                  onClick={() => void applyPreset(preset)}
                  aria-label={`Apply preset ${preset.name}`}
                  title={
                    targetCue
                      ? `Apply “${preset.name}”`
                      : "Pick a cue first"
                  }
                  className="flex items-center gap-1 rounded-l border border-border px-1.5 py-0.5 text-xs hover:bg-surface-hover disabled:opacity-40"
                >
                  <span
                    aria-hidden
                    className={`h-2 w-2 rounded-full ${
                      COLORS.find((c) => c.id === (preset.color ?? -1))
                        ?.className ?? "bg-transparent"
                    }`}
                  />
                  {preset.name}
                  {preset.hotkey != null && (
                    <kbd className="ml-0.5 rounded border border-border px-0.5 text-[9px] text-muted">
                      {preset.hotkey}
                    </kbd>
                  )}
                </button>
                <button
                  type="button"
                  onClick={() => void removePreset(preset)}
                  aria-label={`Delete preset ${preset.name}`}
                  title="Presets are immutable — delete and re-create to change one"
                  className="rounded-r border border-l-0 border-border px-1 py-0.5 text-xs text-muted hover:text-red-400"
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <ul className="space-y-1">
        {cues.length === 0 && (
          <li className="py-2 text-xs text-muted">
            No cues yet. Press 1–8 to set one at the playhead.
          </li>
        )}
        {cues.map((cue) => (
          <li
            key={cue.id}
            className="flex flex-wrap items-center gap-2 rounded px-1 py-1 text-xs hover:bg-surface-hover"
          >
            <span className="w-6 font-mono">
              {slotOf(cue.kind) === 0 ? "M" : slotOf(cue.kind)}
            </span>
            <button
              type="button"
              className="w-20 text-left font-mono text-accent hover:underline"
              onClick={() => cue.in_msec != null && onSeek?.(cue.in_msec)}
              aria-label={`Seek to cue ${slotOf(cue.kind)}`}
            >
              {formatMs(cue.in_msec)}
            </button>
            {cue.out_msec != null && (
              <span className="rounded bg-violet-500/15 px-1 text-violet-400">loop</span>
            )}

            <select
              aria-label={`Colour for cue ${slotOf(cue.kind)}`}
              className="rounded border border-border bg-surface px-1"
              value={cue.color ?? -1}
              onChange={(e) => void setColor(cue, Number(e.target.value))}
            >
              {COLORS.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.label}
                </option>
              ))}
            </select>

            <select
              aria-label={`Loop length for cue ${slotOf(cue.kind)}`}
              className="rounded border border-border bg-surface px-1"
              value=""
              onChange={(e) => {
                const beats = Number(e.target.value);
                if (beats > 0) void makeLoop(cue, beats);
              }}
            >
              <option value="">Loop…</option>
              {[4, 8, 16, 32].map((b) => (
                <option key={b} value={b}>
                  {b} beats
                </option>
              ))}
            </select>

            <button
              type="button"
              className={
                presetTarget === cue.id
                  ? "rounded border border-accent px-1 text-accent"
                  : "text-muted hover:text-fg"
              }
              aria-pressed={presetTarget === cue.id}
              onClick={() =>
                setPresetTarget((id) => (id === cue.id ? null : cue.id))
              }
              aria-label={`Target cue ${slotOf(cue.kind)} for presets`}
            >
              Target
            </button>
            <button
              type="button"
              className="text-muted hover:text-fg"
              onClick={() => void promoteCue(cue)}
              aria-label={`Save cue ${slotOf(cue.kind)} as a preset`}
            >
              Save preset
            </button>
            <button
              type="button"
              className="text-muted hover:text-fg"
              onClick={() => void moveCueToPlayhead(cue)}
              aria-label={`Move cue ${slotOf(cue.kind)} to playhead`}
            >
              Move here
            </button>
            <button
              type="button"
              className="text-muted hover:text-red-400"
              onClick={() => void deleteCue(cue)}
              aria-label={`Delete cue ${slotOf(cue.kind)}`}
            >
              Delete
            </button>
          </li>
        ))}
      </ul>

      <p className="mt-2 text-[11px] text-muted">
        1–8 set or play a cue · ⌘/Ctrl+1–8 delete · Q toggles quantize. Changes are
        staged for review, not written to Rekordbox directly.
      </p>
    </section>
  );
}
