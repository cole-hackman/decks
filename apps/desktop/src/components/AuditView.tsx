import type { ReactNode } from "react";
import { useStagedChanges } from "../hooks/useStagedChanges";
import { BrokenTracksPanel } from "./BrokenTracksPanel";

interface Props {
  libraryPath: string;
  /** Open the agent inspector and run the given prompt. */
  onRunAudit: (prompt: string) => void;
  /** Navigate to the Changes workspace view. */
  onOpenChanges: () => void;
  trackCount: number | null;
}

interface Module {
  id: string;
  title: string;
  description: string;
  prompt: string;
  icon: ReactNode;
}

const MODULES: Module[] = [
  {
    id: "metadata",
    title: "Metadata health",
    description:
      "Find tracks with missing artist, BPM, key, or genre. Stage safe fills where confident.",
    prompt:
      "Audit metadata health using the health.broken_link_scan tool. Summarize missing-artist, missing-bpm, missing-key, missing-genre, and suspicious tracks. Stage only safe proposed fixes for review — do not claim anything was applied.",
    icon: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
        <path d="M14.5 3a.5.5 0 01.5.5v9a.5.5 0 01-.5.5h-13a.5.5 0 01-.5-.5v-9a.5.5 0 01.5-.5h13zm-13-1A1.5 1.5 0 000 3.5v9A1.5 1.5 0 001.5 14h13a1.5 1.5 0 001.5-1.5v-9A1.5 1.5 0 0014.5 2h-13z" />
        <path d="M3 5.5a.5.5 0 01.5-.5h9a.5.5 0 010 1h-9a.5.5 0 01-.5-.5zM3 8a.5.5 0 01.5-.5h9a.5.5 0 010 1h-9A.5.5 0 013 8zm0 2.5a.5.5 0 01.5-.5h6a.5.5 0 010 1h-6a.5.5 0 01-.5-.5z" />
      </svg>
    ),
  },
  {
    id: "orphans",
    title: "Broken file paths",
    description:
      "Scan for tracks whose audio files are missing on disk. Critical before any DJ set.",
    prompt:
      "Run health.orphan_scan and summarize all tracks whose underlying audio files are missing. Group by folder if useful. Do not stage changes for these — orphans require manual relinking.",
    icon: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
        <path d="M4.715 6.542L3.343 7.914a3 3 0 104.243 4.243l1.828-1.829A3 3 0 008.586 5.5L8 6.086a1.002 1.002 0 00-.154.199 2 2 0 01.861 3.337L6.88 11.45a2 2 0 11-2.83-2.83l.793-.792a4.018 4.018 0 01-.128-1.287z" />
        <path d="M6.586 4.672A3 3 0 007.414 9.5l.775-.776a2 2 0 01-.896-3.346L9.12 3.55a2 2 0 012.83 2.83l-.793.792c.112.42.155.855.128 1.287l1.372-1.372a3 3 0 10-4.243-4.243L6.586 4.672z" />
      </svg>
    ),
  },
  {
    id: "duplicates",
    title: "Duplicate candidates",
    description:
      "Surface likely duplicates by title/artist + bpm. Review and stage merges as needed.",
    prompt:
      "Run health.duplicate_scan and summarize likely-duplicate groups by title and artist. List the top groups with track counts. Do not stage changes — surface them for manual review.",
    icon: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
        <path d="M4 4a2 2 0 012-2h6a2 2 0 012 2v6a2 2 0 01-2 2h-6a2 2 0 01-2-2V4zm2-1a1 1 0 00-1 1v6a1 1 0 001 1h6a1 1 0 001-1V4a1 1 0 00-1-1H6z" />
        <path d="M2 6a1 1 0 011-1v8h8a1 1 0 11-2 0H4a2 2 0 01-2-2V6z" />
      </svg>
    ),
  },
  {
    id: "playlists",
    title: "Playlist coverage",
    description:
      "Check folder structure and find tracks not in any playlist. Useful before a gig.",
    prompt:
      "Use library.list_playlists and library.search to find tracks that aren't in any playlist. Summarize coverage by genre if possible. Do not stage changes.",
    icon: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
        <path d="M1 2.5A.5.5 0 011.5 2h9a.5.5 0 010 1h-9a.5.5 0 01-.5-.5zm0 3A.5.5 0 011.5 5h9a.5.5 0 010 1h-9a.5.5 0 01-.5-.5zm0 3A.5.5 0 011.5 8h6a.5.5 0 010 1h-6a.5.5 0 01-.5-.5zM14 12a2 2 0 11-1.732-1.984V4.5a.5.5 0 011 0v7.5z" />
      </svg>
    ),
  },
];

const FULL_AUDIT_PROMPT =
  "Run a full library audit using all available health tools (broken_link_scan, orphan_scan, duplicate_scan). Summarize the top issues by category. Stage only safe proposed fixes for review and clearly mark anything that requires human judgement.";

export function AuditView({
  libraryPath,
  onRunAudit,
  onOpenChanges,
  trackCount,
}: Props) {
  const { data: changes = [] } = useStagedChanges(libraryPath);
  const proposed = changes.filter((c) => c.status === "Proposed").length;
  const accepted = changes.filter((c) => c.status === "Accepted").length;

  return (
    <div className="flex flex-1 flex-col overflow-y-auto bg-base">
      <div className="mx-auto w-full max-w-4xl px-8 py-10">
        {/* Header */}
        <div className="mb-6 flex items-end justify-between border-b border-edge pb-5">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight text-ink">
              Library Audit
            </h1>
            <p className="mt-2 max-w-xl text-[13px] leading-relaxed text-ink-secondary">
              Scan your library for missing metadata, broken paths, duplicates,
              and playlist gaps. The agent stages safe fixes for review —
              nothing is applied to Rekordbox until you export.
            </p>
          </div>
          <button
            onClick={() => onRunAudit(FULL_AUDIT_PROMPT)}
            className="shrink-0 rounded-md bg-accent px-4 py-2 text-sm font-medium text-base transition-colors duration-150 hover:bg-accent-hover"
          >
            Run full audit
          </button>
        </div>

        {/* Library summary strip */}
        <div className="mb-8 grid grid-cols-3 gap-3">
          <SummaryStat label="Tracks" value={trackCount ?? "—"} />
          <SummaryStat
            label="Proposed"
            value={proposed}
            accent={proposed > 0}
            onClick={proposed > 0 ? onOpenChanges : undefined}
          />
          <SummaryStat
            label="Accepted"
            value={accepted}
            status={accepted > 0 ? "ok" : undefined}
            onClick={accepted > 0 ? onOpenChanges : undefined}
          />
        </div>

        {/* Modules */}
        <h2 className="mb-3 text-[11px] font-semibold uppercase tracking-[0.1em] text-ink-muted">
          Targeted scans
        </h2>
        <div className="grid grid-cols-2 gap-3">
          {MODULES.map((m) => (
            <article
              key={m.id}
              className="group flex flex-col gap-2 rounded-lg border border-edge bg-surface p-4 transition-colors duration-150 hover:border-edge-strong"
            >
              <div className="flex items-center gap-2.5">
                <span className="flex h-7 w-7 items-center justify-center rounded-md bg-elevated text-ink-secondary group-hover:text-accent-hover">
                  {m.icon}
                </span>
                <h3 className="text-[14px] font-medium tracking-tight text-ink">
                  {m.title}
                </h3>
              </div>
              <p className="flex-1 text-[12px] leading-relaxed text-ink-muted">
                {m.description}
              </p>
              <button
                onClick={() => onRunAudit(m.prompt)}
                className="self-start rounded-md border border-edge px-2.5 py-1 text-[11px] font-medium uppercase tracking-wider text-ink-secondary transition-colors duration-150 hover:border-accent/50 hover:text-accent-hover"
              >
                Run scan
              </button>
            </article>
          ))}
        </div>

        <div className="mt-8">
          <BrokenTracksPanel libraryPath={libraryPath} />
        </div>

        <p className="mt-8 text-[11px] uppercase tracking-wider text-ink-faint">
          Workflow dashboard · early preview
        </p>
      </div>
    </div>
  );
}

function SummaryStat({
  label,
  value,
  accent = false,
  status,
  onClick,
}: {
  label: string;
  value: number | string;
  accent?: boolean;
  status?: "ok";
  onClick?: () => void;
}) {
  const valueClass = accent
    ? "text-accent-hover"
    : status === "ok"
      ? "text-status-ok"
      : "text-ink";

  const Tag = onClick ? "button" : "div";
  return (
    <Tag
      onClick={onClick}
      className={`flex flex-col gap-1 rounded-lg border border-edge bg-surface px-4 py-3 text-left transition-colors duration-150 ${
        onClick ? "hover:border-edge-strong" : ""
      }`}
    >
      <span className="text-[10px] font-semibold uppercase tracking-[0.1em] text-ink-muted">
        {label}
      </span>
      <span
        className={`font-mono text-2xl font-medium tabular-nums leading-none ${valueClass}`}
      >
        {typeof value === "number" ? value.toLocaleString() : value}
      </span>
    </Tag>
  );
}
