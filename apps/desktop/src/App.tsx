import { useEffect, useCallback, useMemo, useRef, useState } from "react";
import { FirstRunWizard } from "./components/FirstRunWizard";
import { TrackTable } from "./components/TrackTable";
import { TrackDetailPanel } from "./components/TrackDetailPanel";
import { MixableTracksPanel } from "./components/MixableTracksPanel";
import { PlaylistToolsView } from "./components/PlaylistToolsView";
import { FavouritePlaylistsBar } from "./components/FavouritePlaylistsBar";
import { SettingsPanel } from "./components/SettingsPanel";
import { ChatPanel } from "./components/ChatPanel";
import { PlaylistPanel } from "./components/PlaylistPanel";
import { DiffReviewPanel } from "./components/DiffReviewPanel";
import { AnalyticsView } from "./components/AnalyticsView";
import { InboxView } from "./components/InboxView";
import { SidebarNav, type WorkspaceView } from "./components/SidebarNav";
import { CustomTagsPanel } from "./components/CustomTagsPanel";
import { CleanupPanel } from "./components/CleanupPanel";
import { SyncPanel } from "./components/SyncPanel";
import { SmartFixesPanel } from "./components/SmartFixesPanel";
import { TrackMatcherView } from "./components/TrackMatcherView";
import { SmartlistsView } from "./components/SmartlistsView";
import { OrganizeFilesView } from "./components/OrganizeFilesView";
import { RecipesPanel } from "./components/RecipesPanel";
import { DuplicatesView } from "./components/DuplicatesView";
import { IncomingView } from "./components/IncomingView";
import { ArchiveView } from "./components/ArchiveView";
import { StatusBar } from "./components/StatusBar";
import { AuditView } from "./components/AuditView";
import { FilterDrawer } from "./components/FilterDrawer";
import { FilterChips } from "./components/FilterChips";
import { RelocateBanner } from "./components/RelocateBanner";
import { ResizablePanel } from "./components/ui/ResizablePanel";
import { useAppStore } from "./store/appStore";
import { useAudioPlayer } from "./hooks/useAudioPlayer";
import { useStagedChanges } from "./hooks/useStagedChanges";
import { ActionProvider } from "./hooks/useActions";
import { ActionCenter } from "./components/ActionCenter";
import type { ActionDef } from "./lib/actions";
import { useFilterContext } from "./hooks/useFilterContext";
import { useLibrary } from "./hooks/useLibrary";
import { useTrackContextActions } from "./hooks/useTrackContextActions";
import { TrackContextMenu } from "./components/TrackContextMenu";
import { MultiTrackEditor } from "./components/MultiTrackEditor";
import { TagPickerModal } from "./components/TagPickerModal";
import {
  activeFilterCount,
  distinctValues,
  loadPersistedFilters,
  persistFilters,
  type Filters,
} from "./lib/filters";
import {
  getLibraryPath,
  validateLibraryPath,
  getTheme,
  listTagCategories,
  listTags,
} from "./ipc";
import { useQuery } from "@tanstack/react-query";
import type { Track } from "./types";

const IS_MAC =
  typeof navigator !== "undefined" &&
  /Mac/i.test(navigator.platform ?? navigator.userAgent ?? "");
// Reserve space for macOS traffic lights when titleBarStyle: Overlay.
const TRAFFIC_LIGHT_INSET = IS_MAC ? 72 : 0;

export default function App() {
  const { libraryPath, trackCount, theme, setLibraryConfigured, setTheme } =
    useAppStore();
  const [loading, setLoading] = useState(true);
  const [filters, setFilters] = useState<Filters>(() =>
    loadPersistedFilters(null),
  );

  // When the active library changes (including the initial async load), swap
  // in that library's persisted filters. Persist on every change keyed by the
  // current library.
  useEffect(() => {
    setFilters(loadPersistedFilters(libraryPath));
  }, [libraryPath]);

  useEffect(() => {
    persistFilters(filters, libraryPath);
  }, [filters, libraryPath]);
  const [filterDrawerOpen, setFilterDrawerOpen] = useState(false);
  const [selectedTrack, setSelectedTrack] = useState<Track | null>(null);
  const [selectedTrackIds, setSelectedTrackIds] = useState<Set<string>>(new Set());
  const [currentView, setCurrentView] = useState<WorkspaceView>("library");
  const [inspector, setInspector] = useState<
    "details" | "agent" | "mixable" | null
  >(null);
  /** Set when a favourite hotkey asks the playlist panel to jump. */
  const [focusPlaylistId, setFocusPlaylistId] = useState<string | null>(null);
  const [pendingAgentPrompt, setPendingAgentPrompt] = useState<string | null>(
    null,
  );
  const [contextMenu, setContextMenu] = useState<{
    track: Track;
    anchor: { x: number; y: number };
    playlistId?: string;
  } | null>(null);
  const [tagPickerTrackIds, setTagPickerTrackIds] = useState<Set<string> | null>(
    null,
  );
  // The manual editor's selection, frozen when it opens: editing forty tracks
  // while the table's selection changes underneath would save to the wrong set.
  const [editorTrackIds, setEditorTrackIds] = useState<string[] | null>(null);

  const { data: tracks = [] } = useLibrary(libraryPath);
  const { ctx: filterCtx, missingFilesLoading } = useFilterContext(
    libraryPath,
    filters.missingFiles,
  );
  const availableKeys = useMemo(
    () => distinctValues(tracks, (t) => t.musical_key),
    [tracks],
  );
  const availableGenres = useMemo(
    () => distinctValues(tracks, (t) => t.genre),
    [tracks],
  );
  const tagCategoriesQuery = useQuery({
    queryKey: ["tag-categories"],
    queryFn: listTagCategories,
    staleTime: Infinity,
  });
  const allTagsQuery = useQuery({
    queryKey: ["all-tags"],
    queryFn: () => listTags(),
    staleTime: Infinity,
  });

  const { availableTags, tagLabelById } = useMemo(() => {
    const cats = tagCategoriesQuery.data ?? [];
    const tags = allTagsQuery.data ?? [];
    const catName = new Map(cats.map((c) => [c.id, c.name]));
    const options = tags.map((t) => ({
      id: t.id,
      label: `${catName.get(t.category_id) ?? "?"} ▸ ${t.name}`,
    }));
    const byId: Record<string, string> = {};
    for (const opt of options) byId[opt.id] = opt.label;
    return { availableTags: options, tagLabelById: byId };
  }, [tagCategoriesQuery.data, allTagsQuery.data]);

  const activeFilters = activeFilterCount(filters);

  const updateQuery = (q: string) =>
    setFilters((prev) => ({ ...prev, query: q }));

  const runAudit = (prompt: string) => {
    setPendingAgentPrompt(prompt);
    setInspector("agent");
  };

  const audio = useAudioPlayer(selectedTrack);
  const { data: changes = [] } = useStagedChanges(libraryPath);
  const proposedCount = changes.filter((c) => c.status === "Proposed").length;
  const acceptedCount = changes.filter((c) => c.status === "Accepted").length;
  const playingTrack =
    selectedTrack && audio.isCurrentTrack(selectedTrack) ? selectedTrack : null;

  const handleTrackSelect = (track: Track) => {
    setSelectedTrack(track);
    setSelectedTrackIds(new Set([track.id]));
    if (inspector === null) setInspector("details");
  };

  const handleTrackContextMenu = (
    track: Track,
    anchor: { x: number; y: number },
    options?: { playlistId?: string },
  ) => {
    setSelectedTrack(track);
    setSelectedTrackIds(new Set([track.id]));
    setContextMenu({ track, anchor, playlistId: options?.playlistId });
  };

  const trackContextActions = useTrackContextActions({
    libraryPath: libraryPath ?? "",
    playlistId: contextMenu?.playlistId,
    onShowDetails: (track) => {
      setSelectedTrack(track);
      setSelectedTrackIds(new Set([track.id]));
      setInspector("details");
    },
    onEditTags: (track) => {
      // Use the current multi-selection if the right-clicked track is part of
      // it; otherwise scope the picker to just the right-clicked track.
      const ids = selectedTrackIds.has(track.id)
        ? new Set(selectedTrackIds)
        : new Set([track.id]);
      setTagPickerTrackIds(ids);
    },
    onFindMixable: (track) => {
      setSelectedTrack(track);
      setSelectedTrackIds(new Set([track.id]));
      setInspector("mixable");
    },
    onSendToFiles: (track) => {
      // Scope the Files view to the current multi-selection when the
      // right-clicked track is part of it, so "send these twelve" works;
      // otherwise just the one that was clicked.
      if (!selectedTrackIds.has(track.id)) {
        setSelectedTrackIds(new Set([track.id]));
        setSelectedTrack(track);
      }
      setCurrentView("organize");
    },
  });

  const handleSelectionChange = (ids: Set<string>) => {
    setSelectedTrackIds(ids);
    if (ids.size === 1) {
      const id = Array.from(ids)[0];
      const track = tracks.find((t) => t.id === id);
      if (track) setSelectedTrack(track);
    } else if (ids.size === 0) {
      setSelectedTrack(null);
    }
  };

  useEffect(() => {
    setSelectedTrack(null);
    setSelectedTrackIds(new Set());
  }, [libraryPath]);

  // Drop the stale selection when leaving a view that produced it,
  // so the Details panel doesn't carry an orphaned track into a new context.
  useEffect(() => {
    if (currentView !== "library" && currentView !== "playlists" && currentView !== "inbox") {
      setSelectedTrack(null);
      setSelectedTrackIds(new Set());
    }
  }, [currentView]);

  const searchInputRef = useRef<HTMLInputElement>(null);

  const [actionCenterOpen, setActionCenterOpen] = useState(false);

  /**
   * Alt/Option-click a cleanup chip to filter the browser to it.
   *
   * Genre has a real filter dimension. Artist does not, so it goes through the
   * search box — which searches artist among other fields, so the result is a
   * superset rather than an exact match. Close enough to be useful and honest
   * about being a search rather than a filter.
   */
  const filterBrowserTo = useCallback(
    (mode: "genre" | "artist", value: string) => {
      setFilters((prev) =>
        mode === "genre"
          ? { ...prev, genres: [value] }
          : { ...prev, query: value },
      );
      setCurrentView("library");
    },
    [],
  );

  // The global action list. Everything bindable lives here, which is also what
  // makes it all reachable from the Action Center — see lib/actions.ts.
  const actions = useMemo<ActionDef[]>(
    () => [
      {
        id: "app.actionCenter",
        label: "Open Action Center",
        group: "App",
        defaultBinding: { key: " ", meta: true },
        whenEditable: true,
        run: () => setActionCenterOpen((v) => !v),
      },
      {
        id: "library.focusSearch",
        label: "Search the library",
        group: "Library",
        defaultBinding: { key: "/" },
        enabled: currentView === "library",
        run: () => {
          searchInputRef.current?.focus();
          searchInputRef.current?.select();
        },
      },
      {
        id: "player.playPause",
        label: "Play / pause",
        group: "Player",
        defaultBinding: { key: " " },
        run: () => {
          void audio.toggleCurrent();
        },
      },
      {
        id: "tags.openPicker",
        label: "Edit tags on the selection",
        group: "Tags",
        defaultBinding: { key: "t" },
        enabled:
          (currentView === "library" || currentView === "playlists") &&
          selectedTrackIds.size > 0,
        run: () => setTagPickerTrackIds(new Set(selectedTrackIds)),
      },
      {
        id: "library.editTracks",
        label: "Edit the selected tracks",
        group: "Library",
        defaultBinding: { key: "e" },
        enabled:
          (currentView === "library" || currentView === "playlists") &&
          selectedTrackIds.size > 0,
        run: () => setEditorTrackIds([...selectedTrackIds]),
      },
      {
        id: "app.dismiss",
        label: "Close panel or blur input",
        group: "App",
        defaultBinding: { key: "escape" },
        whenEditable: true,
        hidden: true,
        run: () => {
          if (actionCenterOpen) {
            setActionCenterOpen(false);
            return;
          }
          if (
            document.activeElement instanceof HTMLElement &&
            document.activeElement.tagName === "INPUT"
          ) {
            document.activeElement.blur();
            return;
          }
          if (inspector !== null) setInspector(null);
        },
      },
    ],
    [currentView, inspector, audio, selectedTrackIds, actionCenterOpen],
  );

  // Apply theme class to <html>
  useEffect(() => {
    const root = document.documentElement;
    if (theme === "dark") {
      root.classList.add("dark");
      root.classList.remove("light");
    } else {
      root.classList.add("light");
      root.classList.remove("dark");
    }
  }, [theme]);

  useEffect(() => {
    Promise.all([
      getLibraryPath().catch(() => null),
      getTheme().catch(() => null),
    ])
      .then(async ([savedPath, savedTheme]) => {
        if (savedTheme === "dark" || savedTheme === "light") {
          setTheme(savedTheme);
        }
        if (savedPath) {
          try {
            const count = await validateLibraryPath(savedPath);
            setLibraryConfigured(savedPath, count);
          } catch {
            // Saved path is stale — fall through to first-run wizard.
          }
        }
      })
      .finally(() => setLoading(false));
  }, [setLibraryConfigured, setTheme]);

  if (loading) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-base">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-edge-strong border-t-accent-hover" />
      </div>
    );
  }

  if (!libraryPath) {
    return <FirstRunWizard />;
  }

  const showSearch = currentView === "library";
  const showInspectorToggles =
    currentView === "library" || currentView === "playlists";

  return (
    <ActionProvider actions={actions}>
    <div className="flex h-screen w-screen flex-col bg-base text-ink">
      <ActionCenter open={actionCenterOpen} onClose={() => setActionCenterOpen(false)} />
      {/* Top bar — draggable, accounts for macOS traffic lights */}
      <header
        data-tauri-drag-region
        className="relative flex shrink-0 items-center gap-3 border-b border-edge bg-base pr-3"
        style={{ paddingLeft: TRAFFIC_LIGHT_INSET + 12, height: 44 }}
      >
        <div
          data-tauri-drag-region
          className="flex items-baseline gap-2"
        >
          <span
            data-tauri-drag-region
            className="select-none text-[15px] font-semibold tracking-tight text-ink"
          >
            decks
          </span>
          <span
            data-tauri-drag-region
            className="font-mono text-[11px] tabular-nums text-ink-muted"
          >
            {trackCount?.toLocaleString() ?? 0}
          </span>
        </div>

        <div className="ml-auto flex items-center gap-2">
          {showSearch && (
            <>
              <div className="relative">
                <input
                  ref={searchInputRef}
                  type="search"
                  placeholder="Filter library…"
                  value={filters.query}
                  onChange={(e) => updateQuery(e.target.value)}
                  className="w-60 rounded-md border border-edge bg-surface px-3 py-1 pr-7 text-sm text-ink placeholder:text-ink-faint focus:border-accent focus:outline-none"
                />
                <kbd
                  aria-hidden
                  className="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 rounded border border-edge-strong bg-surface px-1 font-mono text-[10px] text-ink-muted"
                >
                  /
                </kbd>
              </div>
              <button
                onClick={() => setFilterDrawerOpen((v) => !v)}
                aria-label="Open filters"
                className={`flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs font-medium transition-colors duration-150 ${
                  activeFilters > 0 || filterDrawerOpen
                    ? "border-accent/60 bg-accent/10 text-accent-hover"
                    : "border-edge text-ink-secondary hover:border-edge-strong hover:text-ink"
                }`}
              >
                <svg viewBox="0 0 16 16" fill="currentColor" className="h-3 w-3">
                  <path d="M1.5 1.5A.5.5 0 012 1h12a.5.5 0 01.39.812l-4.89 6.115V14a.5.5 0 01-.77.42l-3-2A.5.5 0 016 12V7.927L1.11 1.812A.5.5 0 011.5 1.5zm.99.5l4.36 5.451a.5.5 0 01.11.312v3.96l2 1.333V7.763a.5.5 0 01.11-.312L13.43 2H2.49z" />
                </svg>
                <span>Filters</span>
                {activeFilters > 0 && (
                  <span className="ml-0.5 rounded-full bg-accent px-1.5 font-mono text-[10px] font-semibold tabular-nums text-base">
                    {activeFilters}
                  </span>
                )}
              </button>
            </>
          )}
          {showInspectorToggles && (
            <button
              onClick={() =>
                setInspector((v) => (v === "mixable" ? null : "mixable"))
              }
              aria-label={
                inspector === "mixable" ? "Hide mixable tracks" : "Show mixable tracks"
              }
              className={`rounded-md px-2 py-1 text-xs font-medium uppercase tracking-wider transition-colors duration-150 hover:bg-elevated ${
                inspector === "mixable"
                  ? "text-accent-hover"
                  : "text-ink-secondary hover:text-ink"
              }`}
            >
              Mixable
            </button>
          )}
          {showInspectorToggles && (
            <button
              onClick={() =>
                setInspector((v) => (v === "details" ? null : "details"))
              }
              aria-label={inspector === "details" ? "Hide details" : "Show details"}
              className={`rounded-md px-2 py-1 text-xs font-medium uppercase tracking-wider transition-colors duration-150 hover:bg-elevated ${
                inspector === "details"
                  ? "text-accent-hover"
                  : "text-ink-secondary hover:text-ink"
              }`}
            >
              Details
            </button>
          )}
          <button
            onClick={() => setInspector((v) => (v === "agent" ? null : "agent"))}
            aria-label={inspector === "agent" ? "Close agent" : "Open agent"}
            className={`rounded-md p-1.5 transition-colors duration-150 hover:bg-elevated ${
              inspector === "agent"
                ? "text-accent-hover"
                : "text-ink-secondary hover:text-ink"
            }`}
          >
            <svg viewBox="0 0 16 16" fill="currentColor" className="h-4 w-4">
              <path d="M2.678 11.894a1 1 0 01.287.801 10.97 10.97 0 01-.398 2c1.395-.323 2.247-.697 2.634-.893a1 1 0 01.71-.074A8.06 8.06 0 008 14c3.996 0 7-2.807 7-6 0-3.192-3.004-6-7-6S1 4.808 1 8c0 1.468.617 2.83 1.678 3.894zm-.493 3.905a21.682 21.682 0 01-.713.129c-.2.032-.352-.176-.273-.362a9.68 9.68 0 00.244-.637l.003-.01c.248-.72.45-1.548.524-2.319C.743 11.37 0 9.76 0 8c0-3.866 3.582-7 8-7s8 3.134 8 7-3.582 7-8 7a9.06 9.06 0 01-2.347-.306c-.52.263-1.639.742-3.468 1.105z" />
            </svg>
          </button>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        <SidebarNav
          current={currentView}
          onSelect={setCurrentView}
          pendingChangeCount={proposedCount}
        />

        <main className="flex min-w-0 flex-1 flex-col">
          {currentView === "inbox" && (
            <InboxView
              libraryPath={libraryPath}
              selectedTrackIds={selectedTrackIds}
              onSelectionChange={handleSelectionChange}
              onSelect={handleTrackSelect}
              onTrackContextMenu={handleTrackContextMenu}
            />
          )}
          {currentView === "library" && (
            <>
              <FavouritePlaylistsBar
                libraryPath={libraryPath ?? ""}
                selectedTrackIds={selectedTrackIds}
                onOpenPlaylist={(playlistId) => {
                  setFocusPlaylistId(playlistId);
                  setCurrentView("playlists");
                }}
              />
              <FilterChips
                filters={filters}
                onChange={setFilters}
                tagLabelById={tagLabelById}
              />
              {filters.missingFiles && (
                <RelocateBanner libraryPath={libraryPath} />
              )}
              <TrackTable
                libraryPath={libraryPath}
                filters={filters}
                filterCtx={filterCtx}
                selectedTrackIds={selectedTrackIds}
                onSelectionChange={handleSelectionChange}
                onSelect={handleTrackSelect}
                onTrackContextMenu={handleTrackContextMenu}
                tagLabelById={tagLabelById}
              />
            </>
          )}
          {currentView === "incoming" && (
            <IncomingView
              libraryPath={libraryPath}
              selectedTrackIds={selectedTrackIds}
              onSelectionChange={handleSelectionChange}
              onSelect={handleTrackSelect}
              onTrackContextMenu={handleTrackContextMenu}
            />
          )}
          {currentView === "archive" && (
            <ArchiveView
              libraryPath={libraryPath}
              selectedTrackIds={selectedTrackIds}
              onSelectionChange={handleSelectionChange}
              onSelect={handleTrackSelect}
              onTrackContextMenu={handleTrackContextMenu}
              onGoToSync={() => setCurrentView("sync")}
            />
          )}
          {currentView === "playlists" && (
            <PlaylistPanel
              libraryPath={libraryPath}
              selectedTrackId={selectedTrack?.id ?? null}
              onSelectTrack={handleTrackSelect}
              onTrackContextMenu={handleTrackContextMenu}
              focusPlaylistId={focusPlaylistId}
            />
          )}
          {currentView === "tags" && (
            <CustomTagsPanel
              onShowTracks={(tagIds) => {
                setFilters((prev) => ({
                  ...prev,
                  tagIds,
                  tagMatchAll: false,
                }));
                setCurrentView("library");
              }}
            />
          )}
          {currentView === "genre-cleanup" && (
            <CleanupPanel
              mode="genre"
              libraryPath={libraryPath}
              onGoToSync={() => setCurrentView("sync")}
              onFilterTo={filterBrowserTo}
            />
          )}
          {currentView === "artist-cleanup" && (
            <CleanupPanel
              mode="artist"
              libraryPath={libraryPath}
              onGoToSync={() => setCurrentView("sync")}
              onFilterTo={filterBrowserTo}
            />
          )}
          {currentView === "smart-fixes" && (
            <SmartFixesPanel
              libraryPath={libraryPath}
              onGoToSync={() => setCurrentView("sync")}
            />
          )}
          {currentView === "smartlists" && (
            <SmartlistsView
              libraryPath={libraryPath}
              onOpenInspector={(track) => {
                setSelectedTrack(track);
                setSelectedTrackIds(new Set([track.id]));
                setInspector("details");
              }}
            />
          )}
          {currentView === "playlist-tools" && (
            <PlaylistToolsView libraryPath={libraryPath ?? ""} />
          )}
          {currentView === "duplicates" && (
            <DuplicatesView
              libraryPath={libraryPath}
              onOpenInspector={(track) => {
                setSelectedTrack(track);
                setSelectedTrackIds(new Set([track.id]));
                setInspector("details");
              }}
            />
          )}
          {currentView === "matcher" && (
            <TrackMatcherView
              libraryPath={libraryPath}
              onGoToSync={() => setCurrentView("sync")}
            />
          )}
          {currentView === "organize" && (
            <OrganizeFilesView
              libraryPath={libraryPath}
              tracks={tracks}
              selectedTrackIds={selectedTrackIds}
            />
          )}
          {currentView === "recipes" && (
            <RecipesPanel
              libraryPath={libraryPath}
              trackIds={
                selectedTrackIds.size > 0
                  ? tracks.filter((t) => selectedTrackIds.has(t.id)).map((t) => t.id)
                  : tracks.map((t) => t.id)
              }
            />
          )}
          {currentView === "sync" && (
            <SyncPanel libraryPath={libraryPath} />
          )}
          {currentView === "changes" && (
            <DiffReviewPanel libraryPath={libraryPath} />
          )}
          {currentView === "analytics" && (
            <AnalyticsView libraryPath={libraryPath} />
          )}
          {currentView === "audit" && (
            <AuditView
              libraryPath={libraryPath}
              trackCount={trackCount}
              onRunAudit={runAudit}
              onOpenChanges={() => setCurrentView("changes")}
            />
          )}
          {currentView === "settings" && <SettingsPanel />}
        </main>

        {inspector !== null && (
          <ResizablePanel
            side="right"
            className="border-l border-edge bg-base"
            minWidth={260}
            maxWidth={800}
            defaultWidth={280}
          >
            {inspector === "details" && (
              <TrackDetailPanel
                track={selectedTrack}
                libraryPath={libraryPath}
                isPlaying={
                  selectedTrack
                    ? audio.isPlaying && audio.isCurrentTrack(selectedTrack)
                    : false
                }
                onTogglePlay={audio.toggleCurrent}
                currentTime={
                  selectedTrack && audio.isCurrentTrack(selectedTrack)
                    ? audio.currentTime
                    : 0
                }
                playbackDuration={
                  selectedTrack && audio.isCurrentTrack(selectedTrack)
                    ? audio.duration
                    : 0
                }
                onSeek={audio.seek}
              />
            )}
            {inspector === "mixable" && (
              <MixableTracksPanel
                libraryPath={libraryPath ?? ""}
                track={selectedTrack}
                onUseAsNextTrack={(next) => {
                  // The spec's live workflow: the track you just picked becomes
                  // the seed for the next search, so the panel can be driven
                  // through a set without going back to the browser.
                  setSelectedTrack(next);
                  setSelectedTrackIds(new Set([next.id]));
                }}
                onClose={() => setInspector(null)}
              />
            )}
            {inspector === "agent" && (
              <ChatPanel
                libraryPath={libraryPath}
                onClose={() => setInspector(null)}
                pendingPrompt={pendingAgentPrompt}
                onPromptConsumed={() => setPendingAgentPrompt(null)}
              />
            )}
          </ResizablePanel>
        )}
      </div>

      <StatusBar
        libraryPath={libraryPath}
        trackCount={trackCount}
        playingTrack={playingTrack}
        isPlaying={audio.isPlaying}
        pendingChanges={proposedCount}
        acceptedChanges={acceptedCount}
      />

      <FilterDrawer
        open={filterDrawerOpen}
        onClose={() => setFilterDrawerOpen(false)}
        filters={filters}
        onChange={setFilters}
        availableKeys={availableKeys}
        availableGenres={availableGenres}
        availableTags={availableTags}
        missingFilesLoading={missingFilesLoading}
      />

      <TrackContextMenu
        track={contextMenu?.track ?? null}
        anchor={contextMenu?.anchor ?? null}
        onClose={() => setContextMenu(null)}
        actions={trackContextActions}
      />

      {editorTrackIds && (
        <MultiTrackEditor
          libraryPath={libraryPath}
          trackIds={editorTrackIds}
          onClose={() => setEditorTrackIds(null)}
        />
      )}

      {tagPickerTrackIds && (
        <TagPickerModal
          libraryPath={libraryPath}
          selectedTrackIds={tagPickerTrackIds}
          tagsByTrack={filterCtx.tagsByTrack}
          onClose={() => setTagPickerTrackIds(null)}
        />
      )}
    </div>
    </ActionProvider>
  );
}
