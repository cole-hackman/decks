import { useCallback, useEffect, useState } from "react";
import {
  applyGeneratedCues,
  previewGeneratedCues,
  suggestAnchorRules,
} from "../ipc";
import { useToast } from "./Toast";
import type {
  CueAnchor,
  CueConfidence,
  CueTemplate,
  CustomAnchorRule,
  GeneratePreview,
  SkippedCue,
  Track,
} from "../types";

function anchorLabel(a: CueAnchor): string {
  switch (a.kind) {
    case "start":
      return "Start";
    case "drop":
      return a.ordinal === 1 ? "Drop" : `Drop ${a.ordinal}`;
    case "breakdown":
      return a.ordinal === 1 ? "Breakdown" : `Breakdown ${a.ordinal}`;
    case "fade_out":
      return "Fade-out";
    case "end":
      return "End";
  }
}

function confidenceScore(c: CueConfidence): number {
  return c === "certain" ? 1 : c.detected;
}

function isProvisional(c: CueConfidence): boolean {
  return c !== "certain" && c.detected < 0.6;
}

function skippedLabel(s: SkippedCue): string {
  switch (s.reason) {
    case "anchor_missing":
      return `${s.name} — no ${anchorLabel(s.anchor).toLowerCase()} found`;
    case "out_of_range":
      return `${s.name} — lands outside the track`;
    case "overflow":
      return `${s.name} — more cues than Rekordbox can hold`;
    case "duplicate_memory_cue":
      return `${s.name} — Rekordbox rejects two memory cues at the same position`;
  }
}

function formatMs(ms: number): string {
  const total = Math.floor(ms / 1000);
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

/**
 * A starter template: the placements most DJs want, expressed against anchors.
 * "64 beats before the drop" is the canonical example from the Lexicon manual.
 */
const DEFAULT_TEMPLATE: CueTemplate = {
  name: "Default",
  start_behavior: "first_beat",
  keep_cue_position: false,
  entries: [
    {
      anchor: { kind: "start" },
      offset_beats: 0,
      name: "Start",
      color: 5,
      enabled: true,
      memory_cue: false,
      loop_beats: null,
    },
    {
      anchor: { kind: "drop", ordinal: 1 },
      offset_beats: -64,
      name: "Build",
      color: 3,
      enabled: true,
      memory_cue: false,
      loop_beats: null,
    },
    {
      anchor: { kind: "drop", ordinal: 1 },
      offset_beats: 0,
      name: "Drop",
      color: 1,
      enabled: true,
      memory_cue: false,
      loop_beats: null,
    },
    {
      anchor: { kind: "breakdown", ordinal: 1 },
      offset_beats: 0,
      name: "Breakdown",
      color: 6,
      enabled: true,
      memory_cue: false,
      loop_beats: null,
    },
    {
      anchor: { kind: "fade_out" },
      offset_beats: 0,
      name: "Mix out",
      color: 4,
      enabled: true,
      memory_cue: false,
      loop_beats: null,
    },
  ],
};

interface Props {
  libraryPath: string;
  track: Track;
  onChanged?: () => void;
}

/**
 * The Cue Point Generator, custom-cue-anchors edition.
 *
 * Detection is not implemented yet, so anchors come from the user's own cues:
 * they say which existing cue is the drop, and the template does the rest. The
 * panel is explicit about that rather than implying an analyser is running.
 */
export function CueGeneratorPanel({ libraryPath, track, onChanged }: Props) {
  const { toast } = useToast();
  const [template] = useState<CueTemplate>(DEFAULT_TEMPLATE);
  const [rules, setRules] = useState<CustomAnchorRule[]>([]);
  const [preview, setPreview] = useState<GeneratePreview | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    suggestAnchorRules(libraryPath, track.id)
      .then((r) => {
        // Defensive: a host that returns null for an unknown command must not
        // take the panel down with it.
        if (!cancelled) setRules(Array.isArray(r) ? r : []);
      })
      .catch(() => {
        if (!cancelled) setRules([]);
      });
    return () => {
      cancelled = true;
    };
  }, [libraryPath, track.id]);

  const runPreview = useCallback(async () => {
    setBusy(true);
    try {
      const p = await previewGeneratedCues(libraryPath, track.id, template, rules);
      setPreview(p);
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, track.id, template, rules, toast]);

  const apply = useCallback(async () => {
    setBusy(true);
    try {
      const staged = await applyGeneratedCues(libraryPath, track.id, template, rules);
      toast({
        variant: "success",
        message:
          staged.length === 0
            ? "Nothing to stage — no anchors resolved."
            : `Staged ${staged.length} cue(s) for review.`,
      });
      onChanged?.();
    } catch (e) {
      toast({ variant: "error", message: String(e) });
    } finally {
      setBusy(false);
    }
  }, [libraryPath, track.id, template, rules, onChanged, toast]);

  return (
    <section
      className="border-t border-border px-4 py-3"
      aria-label="Cue point generator"
    >
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-muted">
          Cue Point Generator
        </h3>
        <div className="flex gap-2 text-xs">
          <button
            type="button"
            disabled={busy}
            className="rounded border border-border px-2 py-0.5 hover:bg-surface-hover"
            onClick={() => void runPreview()}
          >
            Preview
          </button>
          <button
            type="button"
            disabled={busy || rules.length === 0}
            className="rounded bg-accent px-2 py-0.5 text-white hover:bg-accent-hover disabled:opacity-50"
            onClick={() => void apply()}
          >
            Stage cues
          </button>
        </div>
      </div>

      <p className="mb-2 text-[11px] text-muted">
        Anchors come from cues you already placed — name one “Drop” and the template
        hangs the rest off it. Automatic drop detection is not implemented yet.
      </p>

      <div className="mb-2">
        <p className="mb-1 text-[11px] font-medium uppercase tracking-wide text-muted">
          Anchors
        </p>
        {rules.length === 0 ? (
          <p className="text-xs text-muted" data-testid="no-anchors">
            No anchors found. Name a cue “Drop”, “Breakdown” or “Outro” and reopen this
            panel.
          </p>
        ) : (
          <ul className="space-y-0.5 text-xs">
            {rules.map((r, i) => (
              <li key={i} className="flex items-center gap-2">
                <span className="w-24 text-muted">{anchorLabel(r.anchor)}</span>
                <span className="font-mono">{r.name ?? "(any name)"}</span>
                <button
                  type="button"
                  className="ml-auto text-muted hover:text-red-400"
                  aria-label={`Remove ${anchorLabel(r.anchor)} anchor`}
                  onClick={() => setRules(rules.filter((_, j) => j !== i))}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {preview && (
        <div data-testid="generator-preview">
          <p className="mb-1 text-[11px] font-medium uppercase tracking-wide text-muted">
            Would create {preview.cues.length} cue(s)
          </p>
          <ul className="mb-2 space-y-0.5 text-xs">
            {preview.cues.map((c, i) => (
              <li key={i} className="flex items-center gap-2">
                <span className="w-6 font-mono">{c.memory_cue ? "M" : c.slot}</span>
                <span className="w-16 font-mono">{formatMs(c.position_ms)}</span>
                <span>{c.name}</span>
                {c.loop_end_ms != null && (
                  <span className="rounded bg-violet-500/15 px-1 text-violet-400">
                    loop
                  </span>
                )}
                {isProvisional(c.confidence) && (
                  <span
                    className="rounded bg-amber-500/15 px-1 text-amber-500"
                    title="Low-confidence anchor — check before applying"
                  >
                    provisional {Math.round(confidenceScore(c.confidence) * 100)}%
                  </span>
                )}
              </li>
            ))}
          </ul>

          {preview.skipped.length > 0 && (
            <>
              <p className="mb-1 text-[11px] font-medium uppercase tracking-wide text-muted">
                Skipped
              </p>
              <ul className="space-y-0.5 text-[11px] text-amber-500">
                {preview.skipped.map((s, i) => (
                  <li key={i}>{skippedLabel(s)}</li>
                ))}
              </ul>
            </>
          )}
        </div>
      )}
    </section>
  );
}
