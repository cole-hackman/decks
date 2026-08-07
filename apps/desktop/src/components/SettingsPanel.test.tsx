import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("../ipc", () => ({
  getApiKey: vi.fn().mockResolvedValue(null),
  setApiKey: vi.fn().mockResolvedValue(undefined),
  deleteApiKey: vi.fn().mockResolvedValue(undefined),
  getClaudeCodeStatus: vi.fn().mockResolvedValue({
    installed: false,
    version: null,
    logged_in: null,
    auth_method: null,
    subscription_type: null,
    email: null,
    error: null,
  }),
  setTheme: vi.fn().mockResolvedValue(undefined),
  pickLibraryPath: vi.fn().mockResolvedValue(null),
  validateLibraryPath: vi.fn().mockResolvedValue(42),
  setLibraryPath: vi.fn().mockResolvedValue(undefined),
  getAgentModel: vi.fn().mockResolvedValue("claude-sonnet-4-6"),
  setAgentModel: vi.fn().mockResolvedValue(undefined),
  // Sections mounted inside the panel.
  listAutomaticActions: vi.fn().mockResolvedValue([]),
  setAutomaticAction: vi.fn().mockResolvedValue(undefined),
  listFieldMappings: vi.fn().mockResolvedValue([]),
  createFieldMapping: vi.fn().mockResolvedValue("r1"),
  deleteFieldMapping: vi.fn().mockResolvedValue(true),
  mappableTagTargets: vi.fn().mockResolvedValue([]),
  listPathMappings: vi.fn().mockResolvedValue([]),
  createPathMapping: vi.fn().mockResolvedValue("m1"),
  deletePathMapping: vi.fn().mockResolvedValue(true),
  previewPathMapping: vi.fn().mockResolvedValue(["/x", false]),
  commonTextBlocklistList: vi.fn().mockResolvedValue([]),
  commonTextBlocklistAdd: vi.fn().mockResolvedValue(undefined),
  commonTextBlocklistRemove: vi.fn().mockResolvedValue(undefined),
}));

import {
  getApiKey,
  setApiKey,
  deleteApiKey,
  getClaudeCodeStatus,
  setTheme as setThemeIpc,
  pickLibraryPath,
  validateLibraryPath,
  setLibraryPath as setLibraryPathIpc,
  setAgentModel,
} from "../ipc";

const mockStore = {
  libraryPath: "/tmp/master.db" as string | null,
  theme: "dark" as "dark" | "light",
  trackCount: 42 as number | null,
  setLibraryConfigured: vi.fn(),
  setTheme: vi.fn(),
  clearLibrary: vi.fn(),
};

vi.mock("../store/appStore", () => ({
  useAppStore: vi.fn().mockImplementation(
    (selector?: (s: typeof mockStore) => unknown) =>
      selector ? selector(mockStore) : mockStore,
  ),
}));

beforeEach(() => {
  vi.clearAllMocks();
  mockStore.libraryPath = "/tmp/master.db";
  mockStore.theme = "dark";
  vi.mocked(getApiKey).mockResolvedValue(null);
  vi.mocked(getClaudeCodeStatus).mockResolvedValue({
    installed: false,
    version: null,
    logged_in: null,
    auth_method: null,
    subscription_type: null,
    email: null,
    error: null,
  });
});

describe("SettingsPanel", () => {
  it("renders the settings heading", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByText("Settings")).toBeInTheDocument();
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
  });

  it("calls onClose when backdrop is clicked", async () => {
    const onClose = vi.fn();
    render(<SettingsPanel onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Close settings" }));
    expect(onClose).toHaveBeenCalledOnce();
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
  });

  it("shows dark and light theme buttons", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Dark" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Light" })).toBeInTheDocument();
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
  });

  it("calls setTheme and IPC when Dark is clicked", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Dark" }));
    expect(mockStore.setTheme).toHaveBeenCalledWith("dark");
    await waitFor(() =>
      expect(vi.mocked(setThemeIpc)).toHaveBeenCalledWith("dark"),
    );
  });

  it("calls setTheme and IPC when Light is clicked", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Light" }));
    expect(mockStore.setTheme).toHaveBeenCalledWith("light");
    await waitFor(() =>
      expect(vi.mocked(setThemeIpc)).toHaveBeenCalledWith("light"),
    );
  });

  it("shows current library path", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByText("/tmp/master.db")).toBeInTheDocument();
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
  });

  it("shows '—' when no library path", async () => {
    mockStore.libraryPath = null;
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByText("—")).toBeInTheDocument();
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
  });

  it("shows Change Library button", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(
      screen.getByRole("button", { name: "Change Library…" }),
    ).toBeInTheDocument();
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
  });

  it("calls pickLibraryPath when Change Library is clicked", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Change Library…" }));
    await waitFor(() =>
      expect(vi.mocked(pickLibraryPath)).toHaveBeenCalled(),
    );
  });

  it("validates and saves library path when picker returns a path", async () => {
    vi.mocked(pickLibraryPath).mockResolvedValue("/new/master.db");
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Change Library…" }));
    await waitFor(() =>
      expect(vi.mocked(validateLibraryPath)).toHaveBeenCalledWith(
        "/new/master.db",
      ),
    );
    await waitFor(() =>
      expect(vi.mocked(setLibraryPathIpc)).toHaveBeenCalledWith(
        "/new/master.db",
      ),
    );
    expect(mockStore.setLibraryConfigured).toHaveBeenCalledWith(
      "/new/master.db",
      42,
    );
  });

  it("persists the chosen agent model", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    const select = (await screen.findByLabelText("Agent model")) as HTMLSelectElement;
    expect(select.value).toBe("claude-sonnet-4-6");
    fireEvent.change(select, { target: { value: "claude-opus-4-7" } });
    await waitFor(() =>
      expect(vi.mocked(setAgentModel)).toHaveBeenCalledWith("claude-opus-4-7"),
    );
  });

  it("loads Anthropic key from keychain on mount", async () => {
    vi.mocked(getApiKey).mockResolvedValue("sk-ant-existing");
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() =>
      expect(
        (screen.getByPlaceholderText("sk-ant-…") as HTMLInputElement).value,
      ).toBe("sk-ant-existing"),
    );
  });

  it("shows Claude Code subscription status separately from API key support", async () => {
    vi.mocked(getClaudeCodeStatus).mockResolvedValue({
      installed: true,
      version: "2.1.126 (Claude Code)",
      logged_in: true,
      auth_method: "claude.ai",
      subscription_type: "pro",
      email: "dj@example.com",
      error: null,
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(await screen.findByText("Agent Runtime")).toBeInTheDocument();
    expect(screen.getByText(/Claude Code detected/i)).toBeInTheDocument();
    expect(screen.getByText(/Pro subscription/i)).toBeInTheDocument();
    expect(screen.getByText(/Chat will use your Claude Code subscription/i)).toBeInTheDocument();
  });

  it("saves Anthropic key when Save is clicked", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
    fireEvent.change(screen.getByPlaceholderText("sk-ant-…"), {
      target: { value: "sk-ant-newkey" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(vi.mocked(setApiKey)).toHaveBeenCalledWith(
        "anthropic_api_key",
        "sk-ant-newkey",
      ),
    );
  });

  it("shows Remove button and clears key when clicked", async () => {
    vi.mocked(getApiKey).mockResolvedValue("sk-ant-existing");
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Remove" })).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() =>
      expect(vi.mocked(deleteApiKey)).toHaveBeenCalledWith("anthropic_api_key"),
    );
    await waitFor(() =>
      expect(
        (screen.getByPlaceholderText("sk-ant-…") as HTMLInputElement).value,
      ).toBe(""),
    );
  });

  it("toggles key visibility via show/hide button", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => expect(vi.mocked(getApiKey)).toHaveBeenCalled());
    const input = screen.getByPlaceholderText("sk-ant-…");
    expect(input).toHaveAttribute("type", "password");
    fireEvent.click(screen.getByRole("button", { name: "Show key" }));
    expect(input).toHaveAttribute("type", "text");
    fireEvent.click(screen.getByRole("button", { name: "Hide key" }));
    expect(input).toHaveAttribute("type", "password");
  });
});

import { SettingsPanel } from "./SettingsPanel";
