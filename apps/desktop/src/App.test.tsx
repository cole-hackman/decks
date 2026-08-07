import { render, screen } from "@testing-library/react";
import { describe, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { ToastProvider } from "./components/Toast";
import { DialogHost } from "./components/ui/Dialog";

vi.mock("./ipc", () => ({
  getLibraryPath: vi.fn().mockResolvedValue(null),
  validateLibraryPath: vi.fn(),
  getTheme: vi.fn().mockResolvedValue(null),
  listChanges: vi.fn().mockResolvedValue([]),
  acceptChange: vi.fn(),
  rejectChange: vi.fn(),
  acceptAllSafe: vi.fn(),
  rejectAll: vi.fn(),
  exportAcceptedChanges: vi.fn(),
  listTagCategories: vi.fn().mockResolvedValue([]),
  listTags: vi.fn().mockResolvedValue([]),
  listTrackTagsMap: vi.fn().mockResolvedValue({}),
  keyCompatibility: vi.fn().mockResolvedValue([]),
  applyPlaylistMerge: vi.fn().mockResolvedValue([]),
}));

const mockState = {
  libraryPath: null as string | null,
  trackCount: null as number | null,
  theme: "dark" as "dark" | "light",
  setLibraryConfigured: vi.fn(),
  setTheme: vi.fn(),
};

vi.mock("./store/appStore", () => ({
  useAppStore: vi.fn().mockImplementation((selector?: (s: typeof mockState) => unknown) =>
    selector ? selector(mockState) : mockState
  ),
}));

function renderApp() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    // App is always rendered inside these in main.tsx, and now depends on
    // both — the context menu's "new playlist from selection" prompts for a
    // name and toasts the result.
    <QueryClientProvider client={client}>
      <ToastProvider>
        <DialogHost>
          <App />
        </DialogHost>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

describe("App", () => {
  it("shows first-run wizard when no library is configured", async () => {
    renderApp();
    await screen.findByText("Welcome to decks");
  });
});
