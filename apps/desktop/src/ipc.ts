import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  Track,
  HotCue,
  KeyMixingMode,
  MixableOptions,
  MixableResult,
  MixableTemplate,
  MergePreview,
  SortPreview,
  CrossReferencePreview,
  CrossReferenceMode,
  PlaylistSortMode,
  PrefixSpec,
  PlaylistRenamePlan,
  RewriteOrderPlan,
  Playlist,
  PlaylistDetail,
  DuplicateGroup,
  BrokenMetadataReport,
  LibraryAnalytics,
  TrackTags,
  TagWriteFields,
  AnalysisResult,
  AnlzWaveform,
  RelocateCandidate,
  GenreCount,
  ArtistCount,
  TagCategory,
  Tag,
  Smartlist,
  SmartlistClause,
  SmartlistCombinator,
  SmartlistCompatibility,
  SmartlistGeneratorSpec,
  BeatGridEntry,
  QuantizeResolution,
  CueField,
  CueInput,
  CueTemplate,
  CustomAnchorRule,
  GeneratePreview,
  OrganizeRequest,
  OrganizeResult,
  OrganizeRow,
  PatternField,
  ExtensionFilter,
  UnusedScan,
  DeleteReport,
  TagFieldSelection,
  WriteTagsResult,
  PathMappingRow,
  QuickMoveFolder,
  WatchFolderRow,
  WatchScan,
  ImportResult,
  AutomaticAction,
  FieldMappingRow,
  MappingSource,
  Recipe,
  RecipePreview,
  RecipeProposal,
  TagRecipe,
  TagProposal,
  TagApplyResult,
  OtherRecipe,
  OtherRecipeResult,
  CueRecipe,
  CueRecipeTrack,
  UndoEntry,
  UndoResult,
  UndoRun,
  CsvImportColumns,
  CsvImportPreview,
  CsvPlannedRow,
  MultiEdit,
  MultiEditFormData,
  BrokenScan,
  BrokenTrack,
  CheckDepth,
  ArchiveCriterion,
  ArchiveResult,
  BackupSummary,
  RestoreReport,
  DuplicateCandidate,
  PreferRule,
  ResolutionPlan,
  ResolveResult,
  PathRewrite,
  RewritePreview,
  RewriteSpec,
} from "./types";
import type {
  ChatMessage,
  ConversationSummary,
  NewStagedChange,
  PersistedConversation,
  PersistedConversationMessage,
  StagedChange,
} from "./agent/types";

export async function pickLibraryPath(): Promise<string | null> {
  const result = await open({
    title: "Locate master.db",
    filters: [{ name: "SQLite Database", extensions: ["db"] }],
    multiple: false,
    directory: false,
  });
  if (result === null || result === undefined) return null;
  return typeof result === "string" ? result : null;
}

export async function validateLibraryPath(path: string): Promise<number> {
  return invoke<number>("validate_library_path", { path });
}

export async function listTracks(path: string): Promise<Track[]> {
  return invoke<Track[]>("list_tracks", { path });
}

export async function getTrack(
  path: string,
  trackId: string,
): Promise<Track | null> {
  return invoke<Track | null>("get_track", { path, trackId });
}

export async function getTrackCues(
  path: string,
  trackId: string,
): Promise<HotCue[]> {
  return invoke<HotCue[]>("get_track_cues", { path, trackId });
}

export async function getLibraryPath(): Promise<string | null> {
  return invoke<string | null>("get_library_path");
}

export async function setLibraryPath(path: string): Promise<void> {
  return invoke<void>("set_library_path", { path });
}

export async function playTrack(path: string): Promise<void> {
  return invoke<void>("play_track", { path });
}

export async function pauseAudio(): Promise<void> {
  return invoke<void>("pause_audio");
}

export async function resumeAudio(): Promise<void> {
  return invoke<void>("resume_audio");
}

export async function stopAudio(): Promise<void> {
  return invoke<void>("stop_audio");
}

export interface PlaybackState {
  is_playing: boolean;
  path: string | null;
}

export interface PlaybackStatus {
  is_playing: boolean;
  path: string | null;
  /** Seconds since playback started, 0 if no track loaded. */
  time: number;
  /** Total track duration in seconds, 0 if unknown. */
  duration: number;
}

export async function getPlaybackState(): Promise<PlaybackState> {
  return invoke<PlaybackState>("get_playback_state");
}

export async function getPlaybackStatus(): Promise<PlaybackStatus> {
  return invoke<PlaybackStatus>("get_playback_status");
}

export async function seekAudio(timeSecs: number): Promise<void> {
  return invoke<void>("seek_audio", { timeSecs });
}

export async function revealInFinder(path: string): Promise<void> {
  return invoke<void>("reveal_in_finder", { path });
}

// ── Settings ──────────────────────────────────────────────────────────────────

export async function getTheme(): Promise<string | null> {
  return invoke<string | null>("get_theme");
}

export async function setTheme(theme: string): Promise<void> {
  return invoke<void>("set_theme", { theme });
}

export type AgentModel =
  | "claude-opus-4-7"
  | "claude-sonnet-4-6"
  | "claude-haiku-4-5-20251001";

export async function getAgentModel(): Promise<AgentModel> {
  return invoke<AgentModel>("get_agent_model");
}

export async function setAgentModel(model: AgentModel): Promise<void> {
  return invoke<void>("set_agent_model", { model });
}

export async function getApiKey(service: string): Promise<string | null> {
  return invoke<string | null>("get_api_key", { service });
}

export async function setApiKey(service: string, key: string): Promise<void> {
  return invoke<void>("set_api_key", { service, key });
}

export async function deleteApiKey(service: string): Promise<void> {
  return invoke<void>("delete_api_key", { service });
}

export interface ClaudeCodeStatus {
  installed: boolean;
  version: string | null;
  logged_in: boolean | null;
  auth_method: string | null;
  subscription_type: string | null;
  email: string | null;
  error: string | null;
}

export async function getClaudeCodeStatus(): Promise<ClaudeCodeStatus> {
  return invoke<ClaudeCodeStatus>("get_claude_code_status");
}

// ── Conversations ────────────────────────────────────────────────────────────

export async function listConversations(
  libraryPath?: string | null,
): Promise<ConversationSummary[]> {
  return invoke<ConversationSummary[]>("list_conversations", {
    libraryPath: libraryPath ?? null,
  });
}

export async function createConversation(
  libraryPath: string | null,
  title: string,
): Promise<ConversationSummary> {
  return invoke<ConversationSummary>("create_conversation", {
    libraryPath,
    title,
  });
}

export async function loadConversation(
  id: string,
): Promise<PersistedConversation | null> {
  return invoke<PersistedConversation | null>("load_conversation", { id });
}

export async function appendConversationMessage(
  conversationId: string,
  role: string,
  content: ChatMessage,
): Promise<PersistedConversationMessage> {
  return invoke<PersistedConversationMessage>("append_conversation_message", {
    conversationId,
    role,
    content,
  });
}

export async function renameConversation(
  id: string,
  title: string,
): Promise<void> {
  return invoke<void>("rename_conversation", { id, title });
}

export async function deleteConversation(id: string): Promise<void> {
  return invoke<void>("delete_conversation", { id });
}

// ── Staged changes ───────────────────────────────────────────────────────────

export async function stageChange(
  change: NewStagedChange,
): Promise<StagedChange> {
  return invoke<StagedChange>("stage_change", { change });
}

export async function listChanges(
  libraryPath?: string | null,
): Promise<StagedChange[]> {
  return invoke<StagedChange[]>("list_changes", {
    libraryPath: libraryPath ?? null,
  });
}

export async function acceptChange(id: string): Promise<StagedChange> {
  return invoke<StagedChange>("accept_change", { id });
}

export async function rejectChange(id: string): Promise<StagedChange> {
  return invoke<StagedChange>("reject_change", { id });
}

export async function acceptAllSafe(
  libraryPath?: string | null,
): Promise<StagedChange[]> {
  return invoke<StagedChange[]>("accept_all_safe", {
    libraryPath: libraryPath ?? null,
  });
}

export async function rejectAll(
  libraryPath?: string | null,
): Promise<StagedChange[]> {
  return invoke<StagedChange[]>("reject_all", {
    libraryPath: libraryPath ?? null,
  });
}

export interface ExportResult {
  output_path: string;
  exported_count: number;
}

export async function exportAcceptedChanges(
  libraryPath: string,
  outputPath?: string | null,
): Promise<ExportResult | null> {
  const resolvedPath =
    outputPath ??
    (await save({
      title: "Export Rekordbox XML",
      defaultPath: "rekordagent-export.xml",
      filters: [{ name: "Rekordbox XML", extensions: ["xml"] }],
    }));
  if (!resolvedPath) return null;
  return invoke<ExportResult>("export_accepted_changes", {
    libraryPath,
    outputPath: resolvedPath,
  });
}

// ── Agent tools ───────────────────────────────────────────────────────────────

export async function librarySearch(
  path: string,
  query: string,
  limit?: number,
): Promise<Track[]> {
  return invoke<Track[]>("library_search", { path, query, limit });
}

export async function suggestNextTracks(
  path: string,
  trackId: string,
  limit?: number,
): Promise<[Track, import("./types").TransitionScore][]> {
  return invoke<[Track, import("./types").TransitionScore][]>("suggest_next_tracks", { path, trackId, limit });
}

export async function libraryStageIntroCues(
  libraryPath: string,
  trackIds: string[],
): Promise<StagedChange[]> {
  return invoke<StagedChange[]>("library_stage_intro_cues", {
    libraryPath,
    trackIds,
  });
}

export async function libraryStagePlaylistRemoveTrack(
  libraryPath: string,
  playlistId: string,
  trackId: string,
): Promise<StagedChange> {
  return invoke<StagedChange>("library_stage_playlist_remove_track", {
    libraryPath,
    playlistId,
    trackId,
  });
}

export async function listPlaylists(path: string): Promise<Playlist[]> {
  return invoke<Playlist[]>("list_playlists", { path });
}

export async function getPlaylist(
  path: string,
  playlistId: string,
): Promise<PlaylistDetail | null> {
  return invoke<PlaylistDetail | null>("get_playlist", { path, playlistId });
}

export async function listTracksWithCues(path: string): Promise<string[]> {
  return invoke<string[]>("list_tracks_with_cues", { path });
}

export async function listTracksInAnyPlaylist(path: string): Promise<string[]> {
  return invoke<string[]>("list_tracks_in_any_playlist", { path });
}

export async function listTracksWithMissingFiles(
  path: string,
): Promise<string[]> {
  return invoke<string[]>("list_tracks_with_missing_files", { path });
}

export async function healthOrphanScan(path: string): Promise<Track[]> {
  return invoke<Track[]>("health_orphan_scan", { path });
}

export async function healthDuplicateScan(path: string): Promise<DuplicateGroup[]> {
  return invoke<DuplicateGroup[]>("health_duplicate_scan", { path });
}

export async function healthFuzzyDuplicateScan(path: string): Promise<DuplicateGroup[]> {
  return invoke<DuplicateGroup[]>("health_fuzzy_duplicate_scan", { path });
}

/**
 * Library-wide duplicate scan. Returns groups across all three strategies
 * (exact title+artist, fuzzy title, audio fingerprint) tagged with `kind`.
 */
export async function listLibraryDuplicateGroups(
  libraryPath: string,
): Promise<DuplicateGroup[]> {
  return invoke<DuplicateGroup[]>("library_duplicate_groups", {
    path: libraryPath,
  });
}

export async function healthBrokenLinkScan(
  path: string,
): Promise<BrokenMetadataReport> {
  return invoke<BrokenMetadataReport>("health_broken_link_scan", { path });
}

export async function getLibraryAnalytics(
  path: string,
): Promise<LibraryAnalytics> {
  return invoke<LibraryAnalytics>("library_analytics", { path });
}

export async function readAudioTags(filePath: string): Promise<TrackTags> {
  return invoke<TrackTags>("read_audio_tags", { filePath });
}

export async function analyzeTrack(
  libraryPath: string,
  trackId: string,
): Promise<AnalysisResult> {
  return invoke<AnalysisResult>("analyze_track", {
    libraryPath,
    trackId,
  });
}

export async function getAnlzWaveform(
  libraryPath: string,
  trackId: string,
): Promise<AnlzWaveform> {
  return invoke<AnlzWaveform>("get_anlz_waveform", { libraryPath, trackId });
}

export async function getAudioWaveform(
  filePath: string,
  bars?: number,
): Promise<number[]> {
  return invoke<number[]>("get_audio_waveform", { filePath, bars: bars ?? null });
}

export async function writeAudioTags(
  filePath: string,
  fields: TagWriteFields,
): Promise<void> {
  return invoke<void>("write_audio_tags", { filePath, fields });
}

export async function relocateScan(
  libraryPath: string,
  searchRoots: string[],
): Promise<RelocateCandidate[]> {
  return invoke<RelocateCandidate[]>("relocate_scan", {
    libraryPath,
    searchRoots,
  });
}

// ── Sync (master.db write-back) ────────────────────────────────────────────

export interface SyncCheckResult {
  locked: boolean;
  pending_changes: number;
}

export interface ApplyResult {
  applied: string[];
  failed: [string, string][];
  /** Non-fatal warnings (e.g. unconvertible key values). Optional so older
   *  Tauri builds that pre-date the field still deserialize cleanly. */
  warnings?: string[];
}

export async function syncCheck(libraryPath: string): Promise<SyncCheckResult> {
  return invoke<SyncCheckResult>("sync_check", { libraryPath });
}

export async function syncExecuteAccepted(libraryPath: string): Promise<ApplyResult> {
  return invoke<ApplyResult>("sync_execute_accepted", { libraryPath });
}

export type SyncMode = "full" | "playlist" | "modified";

export type CueDestination = "hot" | "memory" | "both";
export type KeyFormat = "original" | "camelot" | "open_key";

export interface SyncOptions {
  playlist_id?: string | null;
  since_ts?: number | null;
  // Writer-side options — forwarded into the Rust `changes::applier`.
  cue_destination?: CueDestination;
  keep_grids?: boolean;
  convert_keys?: KeyFormat;
  change_to_nearest_color?: boolean;
  all_smartlists_to_playlists?: boolean;
}

export interface PendingChange {
  change_id: string;
  kind: string;
  track_id: string | null;
  track_title: string | null;
  field: string | null;
  old_value: unknown;
  new_value: unknown;
  reason: string | null;
  updated_at: number;
}

export async function syncPreview(
  libraryPath: string,
  mode: SyncMode = "full",
  options: SyncOptions = {},
): Promise<PendingChange[]> {
  return invoke<PendingChange[]>("sync_preview", { libraryPath, mode, options });
}

export async function syncExecute(
  libraryPath: string,
  mode: SyncMode,
  options: SyncOptions,
  changeIds: string[],
): Promise<ApplyResult> {
  return invoke<ApplyResult>("sync_execute", {
    libraryPath,
    mode,
    options,
    changeIds,
  });
}

// ── Custom Tags & Cleanup ──────────────────────────────────────────────────

export interface CleanupResult {
  affected_tracks: number;
  staged_change_ids: string[];
}

export async function listGenres(path: string): Promise<GenreCount[]> {
  return invoke<GenreCount[]>("list_genres", { path });
}

export async function listArtists(path: string): Promise<ArtistCount[]> {
  return invoke<ArtistCount[]>("list_artists", { path });
}

export async function renameGenre(
  libraryPath: string,
  oldGenre: string,
  newGenre: string,
): Promise<CleanupResult> {
  return invoke<CleanupResult>("rename_genre", { libraryPath, oldGenre, newGenre });
}

export async function renameArtist(
  libraryPath: string,
  oldArtist: string,
  newArtist: string,
): Promise<CleanupResult> {
  return invoke<CleanupResult>("rename_artist", { libraryPath, oldArtist, newArtist });
}

export async function deleteGenre(
  libraryPath: string,
  genre: string,
): Promise<CleanupResult> {
  return invoke<CleanupResult>("delete_genre", { libraryPath, genre });
}

export async function deleteArtist(
  libraryPath: string,
  artist: string,
): Promise<CleanupResult> {
  return invoke<CleanupResult>("delete_artist", { libraryPath, artist });
}

// ── Incoming / Archive ─────────────────────────────────────────────────────

export async function listIncomingTracks(libraryPath: string): Promise<Track[]> {
  return invoke<Track[]>("list_incoming_tracks", { libraryPath });
}

export async function clearIncoming(libraryPath: string): Promise<void> {
  return invoke<void>("clear_incoming", { libraryPath });
}

export async function listArchivedTracks(libraryPath: string): Promise<Track[]> {
  return invoke<Track[]>("list_archived_tracks", { libraryPath });
}

export async function listArchivedTrackIds(libraryPath: string): Promise<string[]> {
  return invoke<string[]>("list_archived_track_ids", { libraryPath });
}

export async function archiveTracks(
  libraryPath: string,
  trackIds: string[],
): Promise<void> {
  return invoke<void>("archive_tracks", { libraryPath, trackIds });
}

export async function unarchiveTracks(
  libraryPath: string,
  trackIds: string[],
): Promise<void> {
  return invoke<void>("unarchive_tracks", { libraryPath, trackIds });
}

export async function stageTrackDelete(
  libraryPath: string,
  trackIds: string[],
): Promise<number> {
  return invoke<number>("stage_track_delete", { libraryPath, trackIds });
}

// ── Smart Fixes ─────────────────────────────────────────────────────────────

export interface FixProposal {
  id: string;
  track_id: string;
  track_title: string;
  field: string;
  old_value: string;
  new_value: string;
}

export const SMART_FIX_NAMES = [
  "fix_casing",
  "replace_with_space",
  "fix_encoded_chars",
  "extract_artist",
  "extract_remixer",
  "remove_garbage",
  "remove_promo",
  "remove_number_prefix",
  "remove_urls",
  "add_mix_parens",
  "remove_common_text",
] as const;

export type SmartFixName = (typeof SMART_FIX_NAMES)[number];

export async function smartFixPreview(
  libraryPath: string,
  fixName: SmartFixName,
): Promise<FixProposal[]> {
  return invoke<FixProposal[]>("smart_fix_preview", { libraryPath, fixName });
}

export async function smartFixApply(
  libraryPath: string,
  fixName: SmartFixName,
  proposalIds: string[],
): Promise<number> {
  return invoke<number>("smart_fix_apply", {
    libraryPath,
    fixName,
    proposalIds,
  });
}

export async function commonTextBlocklistList(): Promise<string[]> {
  return invoke<string[]>("common_text_blocklist_list");
}

export async function commonTextBlocklistAdd(pattern: string): Promise<void> {
  return invoke<void>("common_text_blocklist_add", { pattern });
}

export async function commonTextBlocklistRemove(pattern: string): Promise<void> {
  return invoke<void>("common_text_blocklist_remove", { pattern });
}

// ── Track Matcher ──────────────────────────────────────────────────────────

export interface MatchInput {
  title: string;
  artist?: string;
}

export interface MatchedTrack {
  id: string;
  title: string;
  artist: string | null;
}

export type MatchStatus = "Exact" | "Fuzzy" | "Unmatched";

export interface MatchResult {
  input_title: string;
  input_artist: string | null;
  track: MatchedTrack | null;
  score: number;
  status: MatchStatus;
}

export async function matchTracks(
  libraryPath: string,
  candidates: MatchInput[],
): Promise<MatchResult[]> {
  return invoke<MatchResult[]>("match_tracks", { libraryPath, candidates });
}

export async function parseCsvForMatcher(
  content: string,
  titleCol: string,
  artistCol?: string,
): Promise<MatchInput[]> {
  return invoke<MatchInput[]>("parse_csv_for_matcher", {
    content,
    titleCol,
    artistCol: artistCol ?? null,
  });
}

export async function parseCsvHeadersForMatcher(
  content: string,
): Promise<string[]> {
  return invoke<string[]>("parse_csv_headers_for_matcher", { content });
}

export async function createPlaylistFromTracks(
  libraryPath: string,
  name: string,
  trackIds: string[],
): Promise<string> {
  return invoke<string>("create_playlist_from_tracks", {
    libraryPath,
    name,
    trackIds,
  });
}

export async function listTagCategories(): Promise<TagCategory[]> {
  return invoke<TagCategory[]>("list_tag_categories");
}

export async function createTagCategory(name: string): Promise<TagCategory> {
  return invoke<TagCategory>("create_tag_category", { name });
}

export async function renameTagCategory(id: string, name: string): Promise<void> {
  return invoke<void>("rename_tag_category", { id, name });
}

export async function deleteTagCategory(id: string): Promise<void> {
  return invoke<void>("delete_tag_category", { id });
}

export async function listTags(categoryId?: string): Promise<Tag[]> {
  return invoke<Tag[]>("list_tags", { categoryId: categoryId ?? null });
}

export async function createTag(categoryId: string, name: string): Promise<Tag> {
  return invoke<Tag>("create_tag", { categoryId, name });
}

export async function renameTag(id: string, name: string): Promise<void> {
  return invoke<void>("rename_tag", { id, name });
}

export async function deleteTag(id: string): Promise<void> {
  return invoke<void>("delete_tag", { id });
}

export async function moveTag(id: string, newCategoryId: string): Promise<void> {
  return invoke<void>("move_tag", { id, newCategoryId });
}

export async function getTrackTags(libraryPath: string, trackId: string): Promise<Tag[]> {
  return invoke<Tag[]>("get_track_tags", { libraryPath, trackId });
}

export async function setTrackTags(libraryPath: string, trackId: string, tagIds: string[]): Promise<void> {
  return invoke<void>("set_track_tags", { libraryPath, trackId, tagIds });
}

export async function addTrackTag(libraryPath: string, trackId: string, tagId: string): Promise<void> {
  return invoke<void>("add_track_tag", { libraryPath, trackId, tagId });
}

export async function removeTrackTag(libraryPath: string, trackId: string, tagId: string): Promise<void> {
  return invoke<void>("remove_track_tag", { libraryPath, trackId, tagId });
}

export async function searchTracksByTags(libraryPath: string, tagIds: string[], matchAll: boolean): Promise<Track[]> {
  return invoke<Track[]>("search_tracks_by_tags", { libraryPath, tagIds, matchAll });
}

export async function listTrackTagsMap(libraryPath: string): Promise<Record<string, string[]>> {
  return invoke<Record<string, string[]>>("list_track_tags_map", { libraryPath });
}


// ── Smartlists (Epic 1) ──────────────────────────────────────────────────────

export async function listSmartlists(libraryPath: string): Promise<Smartlist[]> {
  return invoke<Smartlist[]>("list_smartlists", { libraryPath });
}

export async function createSmartlist(
  libraryPath: string,
  name: string,
  combinator: SmartlistCombinator,
  clauses: SmartlistClause[],
  parentFolderId: string | null = null,
): Promise<Smartlist> {
  return invoke<Smartlist>("create_smartlist", {
    libraryPath,
    name,
    parentFolderId,
    combinator,
    clauses,
  });
}

export async function updateSmartlist(
  libraryPath: string,
  id: string,
  name: string,
  combinator: SmartlistCombinator,
  clauses: SmartlistClause[],
  parentFolderId: string | null = null,
): Promise<Smartlist> {
  return invoke<Smartlist>("update_smartlist", {
    libraryPath,
    id,
    name,
    parentFolderId,
    combinator,
    clauses,
  });
}

export async function deleteSmartlist(libraryPath: string, id: string): Promise<void> {
  return invoke<void>("delete_smartlist", { libraryPath, id });
}

export async function evaluateSmartlist(libraryPath: string, id: string): Promise<Track[]> {
  return invoke<Track[]>("evaluate_smartlist", { libraryPath, id });
}

/** Evaluate an unsaved rule set — powers the editor's live match count. */
export async function previewSmartlist(
  libraryPath: string,
  combinator: SmartlistCombinator,
  clauses: SmartlistClause[],
): Promise<Track[]> {
  return invoke<Track[]>("preview_smartlist", { libraryPath, combinator, clauses });
}

export async function smartlistCounts(
  libraryPath: string,
): Promise<Record<string, number>> {
  return invoke<Record<string, number>>("smartlist_counts", { libraryPath });
}

export async function smartlistCompatibility(
  libraryPath: string,
): Promise<Record<string, SmartlistCompatibility>> {
  return invoke<Record<string, SmartlistCompatibility>>("smartlist_compatibility", {
    libraryPath,
  });
}

export async function generateSmartlists(
  libraryPath: string,
  spec: SmartlistGeneratorSpec,
): Promise<Smartlist[]> {
  return invoke<Smartlist[]>("generate_smartlists", { libraryPath, spec });
}

// ── Cue editing & beat grid (Epic 2) ─────────────────────────────────────────

export async function getBeatGrid(
  libraryPath: string,
  trackId: string,
): Promise<BeatGridEntry[]> {
  return invoke<BeatGridEntry[]>("get_beat_grid", { libraryPath, trackId });
}

export async function quantizePosition(
  libraryPath: string,
  trackId: string,
  positionMs: number,
  resolution: QuantizeResolution,
): Promise<number> {
  return invoke<number>("quantize_position", {
    libraryPath,
    trackId,
    positionMs,
    resolution,
  });
}

export async function beatJumpPosition(
  libraryPath: string,
  trackId: string,
  positionMs: number,
  beats: number,
): Promise<number> {
  return invoke<number>("beat_jump_position", {
    libraryPath,
    trackId,
    positionMs,
    beats,
  });
}

export async function stageCueAdd(
  libraryPath: string,
  trackId: string,
  cue: CueInput,
  quantizeTo?: QuantizeResolution | null,
): Promise<string> {
  return invoke<string>("stage_cue_add", {
    libraryPath,
    trackId,
    cue,
    quantizeTo: quantizeTo ?? null,
  });
}

export async function stageCueDelete(
  libraryPath: string,
  cueId: string,
): Promise<string> {
  return invoke<string>("stage_cue_delete", { libraryPath, cueId });
}

export async function stageCueEdit(
  libraryPath: string,
  cueId: string,
  field: CueField,
  newValue: unknown,
  oldValue?: unknown,
): Promise<string> {
  return invoke<string>("stage_cue_edit", {
    libraryPath,
    cueId,
    field,
    newValue,
    oldValue: oldValue ?? null,
  });
}

export async function stageGridShift(
  libraryPath: string,
  trackId: string,
  offsetMs: number,
  toleranceMs?: number,
): Promise<string[]> {
  return invoke<string[]>("stage_grid_shift", {
    libraryPath,
    trackId,
    offsetMs,
    toleranceMs: toleranceMs ?? null,
  });
}

// ── Cue Point Generator (Epic 3) ─────────────────────────────────────────────

export async function previewGeneratedCues(
  libraryPath: string,
  trackId: string,
  template: CueTemplate,
  anchorRules: CustomAnchorRule[],
): Promise<GeneratePreview> {
  return invoke<GeneratePreview>("preview_generated_cues", {
    libraryPath,
    trackId,
    template,
    anchorRules,
  });
}

export async function applyGeneratedCues(
  libraryPath: string,
  trackId: string,
  template: CueTemplate,
  anchorRules: CustomAnchorRule[],
): Promise<string[]> {
  return invoke<string[]>("apply_generated_cues", {
    libraryPath,
    trackId,
    template,
    anchorRules,
  });
}

export async function suggestAnchorRules(
  libraryPath: string,
  trackId: string,
): Promise<CustomAnchorRule[]> {
  return invoke<CustomAnchorRule[]>("suggest_anchor_rules", { libraryPath, trackId });
}

// ── File organiser (Epic 4) ──────────────────────────────────────────────────

export async function patternFields(): Promise<PatternField[]> {
  return invoke<PatternField[]>("pattern_fields");
}

export async function validatePattern(pattern: string): Promise<string[]> {
  return invoke<string[]>("validate_pattern", { pattern });
}

export async function previewOrganize(
  libraryPath: string,
  trackIds: string[],
  request: OrganizeRequest,
): Promise<OrganizeRow[]> {
  return invoke<OrganizeRow[]>("preview_organize", {
    libraryPath,
    trackIds,
    request,
  });
}

export async function applyOrganize(
  libraryPath: string,
  rows: OrganizeRow[],
): Promise<OrganizeResult> {
  return invoke<OrganizeResult>("apply_organize", { libraryPath, rows });
}

export async function scanUnusedFiles(
  libraryPath: string,
  roots: string[],
  filter: ExtensionFilter,
): Promise<UnusedScan> {
  return invoke<UnusedScan>("scan_unused_files", { libraryPath, roots, filter });
}

export async function deleteUnusedFiles(
  libraryPath: string,
  paths: string[],
): Promise<DeleteReport> {
  return invoke<DeleteReport>("delete_unused_files", { libraryPath, paths });
}

export async function writeTagsBulk(
  libraryPath: string,
  trackIds: string[],
  selection: TagFieldSelection,
): Promise<WriteTagsResult> {
  return invoke<WriteTagsResult>("write_tags_bulk", {
    libraryPath,
    trackIds,
    selection,
  });
}

export async function listPathMappings(): Promise<PathMappingRow[]> {
  return invoke<PathMappingRow[]>("list_path_mappings");
}

export async function createPathMapping(
  from: string,
  to: string,
): Promise<string> {
  return invoke<string>("create_path_mapping", { from, to });
}

export async function deletePathMapping(id: string): Promise<boolean> {
  return invoke<boolean>("delete_path_mapping", { id });
}

/** Returns `[resolvedPath, existsOnDisk]`. */
export async function previewPathMapping(
  storedPath: string,
): Promise<[string, boolean]> {
  return invoke<[string, boolean]>("preview_path_mapping", { storedPath });
}

export async function listQuickMoveFolders(): Promise<QuickMoveFolder[]> {
  return invoke<QuickMoveFolder[]>("list_quick_move_folders");
}

export async function recordQuickMoveFolder(path: string): Promise<string> {
  return invoke<string>("record_quick_move_folder", { path });
}

export async function toggleQuickMoveFavourite(id: string): Promise<boolean> {
  return invoke<boolean>("toggle_quick_move_favourite", { id });
}

export async function deleteQuickMoveFolder(id: string): Promise<boolean> {
  return invoke<boolean>("delete_quick_move_folder", { id });
}

export async function listWatchFolders(): Promise<WatchFolderRow[]> {
  return invoke<WatchFolderRow[]>("list_watch_folders");
}

export async function addWatchFolder(path: string): Promise<string> {
  return invoke<string>("add_watch_folder", { path });
}

export async function removeWatchFolder(id: string): Promise<boolean> {
  return invoke<boolean>("remove_watch_folder", { id });
}

export async function scanArrivals(libraryPath: string): Promise<WatchScan> {
  return invoke<WatchScan>("scan_arrivals", { libraryPath });
}

export async function stageArrivalImports(
  libraryPath: string,
  paths: string[],
): Promise<ImportResult> {
  return invoke<ImportResult>("stage_arrival_imports", { libraryPath, paths });
}

export async function dismissArrivals(paths: string[]): Promise<number> {
  return invoke<number>("dismiss_arrivals", { paths });
}

export async function clearDismissedArrivals(): Promise<number> {
  return invoke<number>("clear_dismissed_arrivals");
}

export async function listAutomaticActions(): Promise<AutomaticAction[]> {
  return invoke<AutomaticAction[]>("list_automatic_actions");
}

export async function setAutomaticAction(
  key: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("set_automatic_action", { key, enabled });
}

export async function mappableTagTargets(): Promise<string[]> {
  return invoke<string[]>("mappable_tag_targets");
}

export async function listFieldMappings(): Promise<FieldMappingRow[]> {
  return invoke<FieldMappingRow[]>("list_field_mappings");
}

export async function createFieldMapping(
  source: MappingSource,
  target: string,
  overwrite: boolean,
): Promise<string> {
  return invoke<string>("create_field_mapping", { source, target, overwrite });
}

export async function deleteFieldMapping(id: string): Promise<boolean> {
  return invoke<boolean>("delete_field_mapping", { id });
}

export async function markIncomingReviewed(
  libraryPath: string,
  trackIds: string[],
): Promise<number> {
  return invoke<number>("mark_incoming_reviewed", { libraryPath, trackIds });
}

// ── Recipes (Epic 5) ─────────────────────────────────────────────────────────

export async function recipeFields(): Promise<string[]> {
  return invoke<string[]>("recipe_fields");
}

export async function recipePreview(
  libraryPath: string,
  trackIds: string[],
  recipes: Recipe[],
): Promise<RecipePreview> {
  return invoke<RecipePreview>("recipe_preview", {
    libraryPath,
    trackIds,
    recipes,
  });
}

export async function recipeApply(
  libraryPath: string,
  proposals: RecipeProposal[],
): Promise<string[]> {
  return invoke<string[]>("recipe_apply", { libraryPath, proposals });
}

export async function tagRecipePreview(
  libraryPath: string,
  trackIds: string[],
  recipe: TagRecipe,
): Promise<TagProposal[]> {
  return invoke<TagProposal[]>("tag_recipe_preview", {
    libraryPath,
    trackIds,
    recipe,
  });
}

export async function tagRecipeApply(
  libraryPath: string,
  proposals: TagProposal[],
): Promise<TagApplyResult> {
  return invoke<TagApplyResult>("tag_recipe_apply", { libraryPath, proposals });
}

export async function otherRecipeApply(
  libraryPath: string,
  trackIds: string[],
  recipe: OtherRecipe,
): Promise<OtherRecipeResult> {
  return invoke<OtherRecipeResult>("other_recipe_apply", {
    libraryPath,
    trackIds,
    recipe,
  });
}

export async function cueRecipePreview(
  libraryPath: string,
  trackIds: string[],
  recipe: CueRecipe,
): Promise<CueRecipeTrack[]> {
  return invoke<CueRecipeTrack[]>("cue_recipe_preview", {
    libraryPath,
    trackIds,
    recipe,
  });
}

export async function cueRecipeApply(
  libraryPath: string,
  tracks: CueRecipeTrack[],
): Promise<string[]> {
  return invoke<string[]>("cue_recipe_apply", { libraryPath, tracks });
}

export async function listUndoRuns(libraryPath: string): Promise<UndoRun[]> {
  return invoke<UndoRun[]>("list_undo_runs", { libraryPath });
}

export async function undoRunEntries(runId: string): Promise<UndoEntry[]> {
  return invoke<UndoEntry[]>("undo_run_entries", { runId });
}

export async function undoRun(
  libraryPath: string,
  runId: string,
): Promise<UndoResult> {
  return invoke<UndoResult>("undo_run", { libraryPath, runId });
}

export async function csvImportHeaders(csv: string): Promise<string[]> {
  return invoke<string[]>("csv_import_headers", { csv });
}

export async function csvImportFields(): Promise<string[]> {
  return invoke<string[]>("csv_import_fields");
}

export async function csvImportPreview(
  libraryPath: string,
  csv: string,
  columns: CsvImportColumns,
): Promise<CsvImportPreview> {
  return invoke<CsvImportPreview>("csv_import_preview", {
    libraryPath,
    csv,
    columns,
  });
}

export async function csvImportApply(
  libraryPath: string,
  rows: CsvPlannedRow[],
): Promise<string[]> {
  return invoke<string[]>("csv_import_apply", { libraryPath, rows });
}

export async function multiEditFields(): Promise<string[]> {
  return invoke<string[]>("multi_edit_fields");
}

export async function multiEditForm(
  libraryPath: string,
  trackIds: string[],
): Promise<MultiEditFormData> {
  return invoke<MultiEditFormData>("multi_edit_form", { libraryPath, trackIds });
}

export async function multiEditApply(
  libraryPath: string,
  trackIds: string[],
  edits: MultiEdit[],
): Promise<string[]> {
  return invoke<string[]>("multi_edit_apply", { libraryPath, trackIds, edits });
}

export async function scanBrokenTracks(
  libraryPath: string,
  trackIds: string[],
  depth: CheckDepth,
): Promise<BrokenScan> {
  return invoke<BrokenScan>("scan_broken_tracks", {
    libraryPath,
    trackIds,
    depth,
  });
}

export async function brokenTracksReport(
  scanBroken: BrokenTrack[],
): Promise<string> {
  return invoke<string>("broken_tracks_report", { scanBroken });
}

/**
 * Save text to a path the user picks.
 *
 * Returns the path, or `null` when the dialog was cancelled — a cancel is a
 * decision, not a failure, so it must not read as an error.
 */
export async function saveTextFile(
  defaultName: string,
  contents: string,
): Promise<string | null> {
  const path = await save({
    title: "Save report",
    defaultPath: defaultName,
    filters: [{ name: "Text", extensions: ["txt"] }],
  });
  if (!path) return null;
  await invoke<void>("save_broken_tracks_report", { path, contents });
  return path;
}

export async function archiveTracksFrom(
  libraryPath: string,
  trackIds: string[],
  fromPlaylistId: string | null,
): Promise<ArchiveResult> {
  return invoke<ArchiveResult>("archive_tracks_from", {
    libraryPath,
    trackIds,
    fromPlaylistId,
  });
}

export async function selectArchived(
  libraryPath: string,
  criterion: ArchiveCriterion,
): Promise<string[]> {
  return invoke<string[]>("select_archived", { libraryPath, criterion });
}

export async function cleanupArchived(
  libraryPath: string,
  trackIds: string[],
): Promise<string[]> {
  return invoke<string[]>("cleanup_archived", { libraryPath, trackIds });
}

/**
 * Write a backup of the local cache's derived state.
 *
 * Returns `null` when the save dialog was cancelled — a decision, not a
 * failure.
 */
export async function createBackup(): Promise<BackupSummary | null> {
  const path = await save({
    title: "Save decks backup",
    defaultPath: "decks-backup.json",
    filters: [{ name: "decks backup", extensions: ["json"] }],
  });
  if (!path) return null;
  return invoke<BackupSummary>("create_backup", { path });
}

/** What a backup file holds. Returns `null` when the picker was cancelled. */
export async function pickAndInspectBackup(): Promise<BackupSummary | null> {
  const path = await open({
    title: "Open decks backup",
    filters: [{ name: "decks backup", extensions: ["json"] }],
    multiple: false,
    directory: false,
  });
  if (typeof path !== "string") return null;
  return invoke<BackupSummary>("inspect_backup", { path });
}

export async function restoreBackup(path: string): Promise<RestoreReport> {
  return invoke<RestoreReport>("restore_backup", { path });
}

// ── Genre / Artist Cleanup state ─────────────────────────────────────────────

export async function listCleanupLocks(kind: string): Promise<string[]> {
  return invoke<string[]>("list_cleanup_locks", { kind });
}

/** Returns the new lock state, so the caller need not guess what a toggle did. */
export async function toggleCleanupLock(
  kind: string,
  value: string,
): Promise<boolean> {
  return invoke<boolean>("toggle_cleanup_lock", { kind, value });
}

export async function listPinnedLetters(kind: string): Promise<string[]> {
  return invoke<string[]>("list_pinned_letters", { kind });
}

export async function togglePinnedLetter(
  kind: string,
  letter: string,
): Promise<boolean> {
  return invoke<boolean>("toggle_pinned_letter", { kind, letter });
}

// ── Duplicate resolution ─────────────────────────────────────────────────────

export async function preselectKeepers(
  groups: DuplicateCandidate[][],
  rule: PreferRule,
): Promise<(string | null)[]> {
  return invoke<(string | null)[]>("preselect_keepers", { groups, rule });
}

export async function planDuplicateResolution(
  libraryPath: string,
  keeperId: string,
  loserIds: string[],
): Promise<ResolutionPlan> {
  return invoke<ResolutionPlan>("plan_duplicate_resolution", {
    libraryPath,
    keeperId,
    loserIds,
  });
}

export async function resolveDuplicates(
  libraryPath: string,
  plan: ResolutionPlan,
): Promise<ResolveResult> {
  return invoke<ResolveResult>("resolve_duplicates", { libraryPath, plan });
}

// ── Path rewriting ───────────────────────────────────────────────────────────

export async function previewPathRewrite(
  libraryPath: string,
  spec: RewriteSpec,
): Promise<RewritePreview> {
  return invoke<RewritePreview>("preview_path_rewrite", { libraryPath, spec });
}

export async function applyPathRewrite(
  libraryPath: string,
  rewrites: PathRewrite[],
): Promise<string[]> {
  return invoke<string[]>("apply_path_rewrite", { libraryPath, rewrites });
}

// ── Mixable Tracks (Epic 6) ──────────────────────────────────────────────────

export async function findMixableTracks(
  path: string,
  trackId: string,
  options: MixableOptions | null,
): Promise<MixableResult> {
  return invoke<MixableResult>("find_mixable_tracks", {
    path,
    trackId,
    options,
  });
}

export async function mixableDefaultOptions(): Promise<MixableOptions> {
  return invoke<MixableOptions>("mixable_default_options");
}

export async function getKeyMixingMode(): Promise<KeyMixingMode> {
  return invoke<KeyMixingMode>("get_key_mixing_mode");
}

export async function setKeyMixingMode(mode: KeyMixingMode): Promise<void> {
  return invoke<void>("set_key_mixing_mode", { mode });
}

export async function listMixableTemplates(): Promise<MixableTemplate[]> {
  return invoke<MixableTemplate[]>("list_mixable_templates");
}

export async function saveMixableTemplate(
  name: string,
  options: MixableOptions,
): Promise<string> {
  return invoke<string>("save_mixable_template", { name, options });
}

export async function deleteMixableTemplate(id: string): Promise<boolean> {
  return invoke<boolean>("delete_mixable_template", { id });
}

// ── Playlist Tools (Epic 6) ──────────────────────────────────────────────────

export async function previewPlaylistMerge(
  path: string,
  playlistIds: string[],
): Promise<MergePreview> {
  return invoke<MergePreview>("preview_playlist_merge", { path, playlistIds });
}

export async function applyPlaylistMerge(
  libraryPath: string,
  name: string,
  parentId: string | null,
  trackIds: string[],
): Promise<string[]> {
  return invoke<string[]>("apply_playlist_merge", {
    libraryPath,
    name,
    parentId,
    trackIds,
  });
}

export async function previewPlaylistSort(
  path: string,
  parentId: string | null,
  mode: PlaylistSortMode,
): Promise<SortPreview> {
  return invoke<SortPreview>("preview_playlist_sort", { path, parentId, mode });
}

export async function applyPlaylistSort(
  libraryPath: string,
  parentId: string | null,
  order: string[],
): Promise<string> {
  return invoke<string>("apply_playlist_sort", { libraryPath, parentId, order });
}

export async function previewCrossReference(
  path: string,
  playlistIds: string[],
  mode: CrossReferenceMode,
): Promise<CrossReferencePreview> {
  return invoke<CrossReferencePreview>("preview_cross_reference", {
    path,
    playlistIds,
    mode,
  });
}

export async function previewPlaylistPrefix(
  path: string,
  playlistIds: string[],
  spec: PrefixSpec,
): Promise<PlaylistRenamePlan[]> {
  return invoke<PlaylistRenamePlan[]>("preview_playlist_prefix", {
    path,
    playlistIds,
    spec,
  });
}

export async function applyPlaylistPrefix(
  libraryPath: string,
  renames: PlaylistRenamePlan[],
): Promise<string[]> {
  return invoke<string[]>("apply_playlist_prefix", { libraryPath, renames });
}

export async function previewRewriteOrder(
  path: string,
  playlistId: string,
  visibleOrder: string[],
): Promise<RewriteOrderPlan> {
  return invoke<RewriteOrderPlan>("preview_rewrite_order", {
    path,
    request: { playlist_id: playlistId, visible_order: visibleOrder },
  });
}

export async function applyRewriteOrder(
  libraryPath: string,
  plan: RewriteOrderPlan,
): Promise<string | null> {
  return invoke<string | null>("apply_rewrite_order", { libraryPath, plan });
}
