import { useState, useEffect } from "react";
import {
  pickLibraryPath,
  validateLibraryPath,
  setLibraryPath as setLibraryPathIpc,
  setTheme as setThemeIpc,
  getApiKey,
  setApiKey,
  deleteApiKey,
  getAgentModel,
  setAgentModel,
  getClaudeCodeStatus,
  type AgentModel,
  type ClaudeCodeStatus,
} from "../ipc";

const MODEL_OPTIONS: { value: AgentModel; label: string }[] = [
  { value: "claude-sonnet-4-6", label: "Sonnet 4.6 — recommended" },
  { value: "claude-opus-4-7", label: "Opus 4.7 — most capable" },
  { value: "claude-haiku-4-5-20251001", label: "Haiku 4.5 — fastest" },
];
import { useAppStore } from "../store/appStore";
import { useToast } from "./Toast";
import { PathMappingsSection } from "./PathMappingsSection";
import { AutomaticActionsSection } from "./AutomaticActionsSection";
import { FieldMappingsSection } from "./FieldMappingsSection";
import { CommonTextSection } from "./CommonTextSection";

interface Props {
  /** When provided, the panel renders as a modal overlay with a close affordance.
   *  When omitted, the panel renders inline as a workspace view. */
  onClose?: () => void;
}

export function SettingsPanel({ onClose }: Props) {
  const { libraryPath, theme, setLibraryConfigured, setTheme } = useAppStore();
  const { toast } = useToast();

  const [libraryChanging, setLibraryChanging] = useState(false);
  const [libraryError, setLibraryError] = useState<string | null>(null);

  const [anthropicKey, setAnthropicKey] = useState("");
  const [keyLoaded, setKeyLoaded] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [keySaving, setKeySaving] = useState(false);
  const [keySaved, setKeySaved] = useState(false);
  const [claudeCodeStatus, setClaudeCodeStatus] =
    useState<ClaudeCodeStatus | null>(null);
  const [agentModel, setAgentModelState] = useState<AgentModel>("claude-sonnet-4-6");

  useEffect(() => {
    getApiKey("anthropic_api_key")
      .then((key) => {
        if (key) setAnthropicKey(key);
      })
      .catch(() => {})
      .finally(() => setKeyLoaded(true));

    getAgentModel()
      .then((m) => setAgentModelState(m))
      .catch(() => {});

    getClaudeCodeStatus()
      .then((s) =>
        setClaudeCodeStatus(
          s ?? {
            installed: false,
            version: null,
            logged_in: null,
            auth_method: null,
            subscription_type: null,
            email: null,
            error: null,
          },
        ),
      )
      .catch((e) =>
        setClaudeCodeStatus({
          installed: false,
          version: null,
          logged_in: null,
          auth_method: null,
          subscription_type: null,
          email: null,
          error: e instanceof Error ? e.message : String(e),
        }),
      );
  }, []);

  const handleThemeChange = async (newTheme: "dark" | "light") => {
    setTheme(newTheme);
    try {
      await setThemeIpc(newTheme);
    } catch (e) {
      console.error("set theme error", e);
    }
  };

  const handleChangeLibrary = async () => {
    setLibraryChanging(true);
    setLibraryError(null);
    try {
      const path = await pickLibraryPath();
      if (!path) return;
      const count = await validateLibraryPath(path);
      await setLibraryPathIpc(path);
      setLibraryConfigured(path, count);
      toast({
        variant: "success",
        message: "Library connected",
        detail: `${count.toLocaleString()} tracks`,
      });
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      setLibraryError(detail);
      toast({ variant: "error", message: "Could not open library", detail });
    } finally {
      setLibraryChanging(false);
    }
  };

  const handleSaveKey = async () => {
    setKeySaving(true);
    try {
      await setApiKey("anthropic_api_key", anthropicKey);
      setKeySaved(true);
      setTimeout(() => setKeySaved(false), 2000);
      toast({ variant: "success", message: "API key saved to keychain" });
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      toast({ variant: "error", message: "Failed to save API key", detail });
    } finally {
      setKeySaving(false);
    }
  };

  const handleRemoveKey = async () => {
    try {
      await deleteApiKey("anthropic_api_key");
      setAnthropicKey("");
      toast({ variant: "info", message: "API key removed" });
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      toast({ variant: "error", message: "Failed to remove API key", detail });
    }
  };

  const content = (
    <div
      className={
        onClose
          ? "flex h-full w-96 shrink-0 flex-col overflow-y-auto border-l border-edge bg-base"
          : "mx-auto flex w-full max-w-2xl flex-col gap-0 px-8 py-8"
      }
    >
      {/* Header */}
      <div
        className={
          onClose
            ? "flex shrink-0 items-center justify-between border-b border-edge px-5 py-4"
            : "mb-6 flex items-end justify-between border-b border-edge pb-4"
        }
      >
        <h2
          className={
            onClose
              ? "text-sm font-semibold text-ink"
              : "text-2xl font-semibold tracking-tight text-ink"
          }
        >
          Settings
        </h2>
        {onClose && (
          <button
            onClick={onClose}
            aria-label="Close settings"
            className="text-ink-secondary transition-colors hover:text-ink"
          >
            <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
              <path d="M3.22 3.22a.75.75 0 011.06 0L8 6.94l3.72-3.72a.75.75 0 111.06 1.06L9.06 8l3.72 3.72a.75.75 0 11-1.06 1.06L8 9.06l-3.72 3.72a.75.75 0 01-1.06-1.06L6.94 8 3.22 4.28a.75.75 0 010-1.06z" />
            </svg>
          </button>
        )}
      </div>

      {/* Appearance */}
      <section
        className={
          onClose
            ? "border-b border-edge px-5 py-4"
            : "border-b border-edge/60 py-6"
        }
      >
        <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
          Appearance
        </h3>
        <div className="flex gap-2">
          <button
            onClick={() => void handleThemeChange("dark")}
            className={`flex-1 rounded-md px-3 py-2 text-sm transition-colors duration-150 ${
              theme === "dark"
                ? "bg-accent font-medium text-base"
                : "bg-elevated text-ink-secondary hover:bg-hover"
            }`}
          >
            Dark
          </button>
          <button
            onClick={() => void handleThemeChange("light")}
            className={`flex-1 rounded-md px-3 py-2 text-sm transition-colors duration-150 ${
              theme === "light"
                ? "bg-accent font-medium text-base"
                : "bg-elevated text-ink-secondary hover:bg-hover"
            }`}
          >
            Light
          </button>
        </div>
      </section>

      {/* Library */}
      <section
        className={
          onClose
            ? "border-b border-edge px-5 py-4"
            : "border-b border-edge/60 py-6"
        }
      >
        <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
          Library
        </h3>
        <p
          className="mb-3 truncate font-mono text-xs text-ink-secondary"
          title={libraryPath ?? undefined}
        >
          {libraryPath ?? "—"}
        </p>
        {libraryError && (
          <p className="mb-2 text-xs text-red-400">{libraryError}</p>
        )}
        <button
          onClick={() => void handleChangeLibrary()}
          disabled={libraryChanging}
          className="w-full rounded-md bg-elevated px-3 py-2 text-sm text-ink transition-colors duration-150 hover:bg-hover disabled:cursor-not-allowed disabled:opacity-50"
        >
          {libraryChanging ? "Locating…" : "Change Library…"}
        </button>
      </section>

      {/* Agent runtime */}
      <section
        className={
          onClose
            ? "border-b border-edge px-5 py-4"
            : "border-b border-edge/60 py-6"
        }
      >
        <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
          Agent Runtime
        </h3>
        {claudeCodeStatus === null ? (
          <div className="flex justify-center py-3">
            <div className="h-4 w-4 animate-spin rounded-full border border-edge-strong border-t-accent-hover" />
          </div>
        ) : claudeCodeStatus.installed ? (
          <div className="rounded-md border border-edge bg-elevated p-3 text-xs">
            <div className="flex items-center justify-between gap-3">
              <span className="font-medium text-ink">
                Claude Code detected
              </span>
              <span className="rounded bg-elevated px-1.5 py-0.5 font-mono text-ink-secondary">
                {claudeCodeStatus.version ?? "installed"}
              </span>
            </div>
            {claudeCodeStatus.logged_in ? (
              <p className="mt-2 text-ink-secondary">
                Signed in as {claudeCodeStatus.email ?? "Claude user"}
                {claudeCodeStatus.subscription_type
                  ? ` with ${titleCase(claudeCodeStatus.subscription_type)} subscription`
                  : ""}
                .
              </p>
            ) : (
              <p className="mt-2 text-ink-secondary">Not signed in to Claude Code.</p>
            )}
            {claudeCodeStatus.logged_in ? (
              <p className="mt-2 text-green-400/80 text-[11px]">
                Chat will use your Claude Code subscription — no API key needed.
              </p>
            ) : null}
          </div>
        ) : (
          <div className="rounded-md border border-edge bg-elevated p-3 text-xs text-ink-secondary">
            <p>
              Claude Code was not found on this Mac. Current chat runtime uses
              Anthropic API keys.
            </p>
            {claudeCodeStatus.error && (
              <p className="mt-2 font-mono text-[10px] text-status-warn">
                {claudeCodeStatus.error}
              </p>
            )}
          </div>
        )}
      </section>

      <AutomaticActionsSection className={onClose ? "px-5 py-4" : "py-6"} />

      <FieldMappingsSection className={onClose ? "px-5 py-4" : "py-6"} />

      <CommonTextSection className={onClose ? "px-5 py-4" : "py-6"} />

      <PathMappingsSection className={onClose ? "px-5 py-4" : "py-6"} />

      {/* Agent model */}
      <section className={onClose ? "px-5 py-4" : "py-6"}>
        <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
          Agent Model
        </h3>
        <label className="mb-1.5 block text-xs text-ink-secondary">
          Anthropic model used for chat
        </label>
        <select
          aria-label="Agent model"
          value={agentModel}
          onChange={(e) => {
            const next = e.target.value as AgentModel;
            setAgentModelState(next);
            void setAgentModel(next).catch((err) => {
              const detail = err instanceof Error ? err.message : String(err);
              toast({ variant: "error", message: "Could not save model", detail });
            });
          }}
          className="w-full rounded-md border border-edge-strong bg-surface px-3 py-1.5 text-sm text-ink focus:border-accent focus:outline-none"
        >
          {MODEL_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
        <p className="mt-2 text-[11px] text-ink-faint">
          Applies to API-key chat. Claude Code subscription chat uses your Claude
          Code account's default model.
        </p>
      </section>

      {/* API Keys */}
      <section className={onClose ? "px-5 py-4" : "py-6"}>
        <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
          API Keys
        </h3>
        {!keyLoaded ? (
          <div className="flex justify-center py-3">
            <div className="h-4 w-4 animate-spin rounded-full border border-edge-strong border-t-accent-hover" />
          </div>
        ) : (
          <div>
            <label className="mb-1.5 block text-xs text-ink-secondary">
              Anthropic API Key
            </label>
            <div className="relative mb-2 flex items-center">
              <input
                type={showKey ? "text" : "password"}
                value={anthropicKey}
                onChange={(e) => setAnthropicKey(e.target.value)}
                placeholder="sk-ant-…"
                className="w-full rounded-md border border-edge-strong bg-surface px-3 py-1.5 pr-10 font-mono text-sm text-ink placeholder:text-ink-faint focus:border-accent focus:outline-none"
              />
              <button
                type="button"
                onClick={() => setShowKey((v) => !v)}
                aria-label={showKey ? "Hide key" : "Show key"}
                className="absolute right-2 text-ink-muted transition-colors hover:text-ink-secondary"
              >
                {showKey ? (
                  <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
                    <path d="M13.359 11.238C15.06 9.72 16 8 16 8s-3-5.5-8-5.5a7.028 7.028 0 00-2.79.588l.77.771A5.944 5.944 0 018 3.5c2.12 0 3.879 1.168 5.168 2.457A13.134 13.134 0 0114.828 8c-.058.087-.122.183-.195.288-.335.48-.83 1.12-1.465 1.755-.165.165-.337.328-.517.486l.708.709z" />
                    <path d="M11.297 9.176a3.5 3.5 0 00-4.474-4.474l.823.823a2.5 2.5 0 012.829 2.829l.822.822zm-2.943 1.299l.822.822a3.5 3.5 0 01-4.474-4.474l.823.823a2.5 2.5 0 002.829 2.829z" />
                    <path d="M3.35 5.47c-.18.16-.353.322-.518.487A13.134 13.134 0 001.172 8l.195.288c.335.48.83 1.12 1.465 1.755C4.121 11.332 5.881 12.5 8 12.5c.716 0 1.39-.133 2.02-.36l.77.772A7.029 7.029 0 018 13.5C3 13.5 0 8 0 8s.939-1.721 2.641-3.238l.708.709zm10.296 8.884l-12-12 .708-.708 12 12-.708.708z" />
                  </svg>
                ) : (
                  <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
                    <path d="M16 8s-3-5.5-8-5.5S0 8 0 8s3 5.5 8 5.5S16 8 16 8zM1.173 8a13.133 13.133 0 011.66-2.043C4.12 4.668 5.88 3.5 8 3.5c2.12 0 3.879 1.168 5.168 2.457A13.133 13.133 0 0114.828 8c-.058.087-.122.183-.195.288-.335.48-.83 1.12-1.465 1.755C11.879 11.332 10.119 12.5 8 12.5c-2.12 0-3.879-1.168-5.168-2.457A13.134 13.134 0 011.172 8z" />
                    <path d="M8 5.5a2.5 2.5 0 100 5 2.5 2.5 0 000-5zM4.5 8a3.5 3.5 0 117 0 3.5 3.5 0 01-7 0z" />
                  </svg>
                )}
              </button>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => void handleSaveKey()}
                disabled={keySaving || anthropicKey.trim() === ""}
                className="flex-1 rounded-md bg-accent px-3 py-1.5 text-sm font-medium text-base transition-colors duration-150 hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-50"
              >
                {keySaved ? "Saved!" : keySaving ? "Saving…" : "Save"}
              </button>
              {anthropicKey && (
                <button
                  onClick={() => void handleRemoveKey()}
                  className="rounded-md bg-elevated px-3 py-1.5 text-sm text-ink-secondary transition-colors duration-150 hover:bg-hover"
                >
                  Remove
                </button>
              )}
            </div>
          </div>
        )}
      </section>
    </div>
  );

  if (!onClose) {
    return (
      <div className="flex flex-1 flex-col overflow-y-auto bg-base">
        {content}
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-50 flex">
      <div
        className="flex-1 bg-black/50"
        onClick={onClose}
        aria-hidden="true"
      />
      {content}
    </div>
  );
}

function titleCase(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}
