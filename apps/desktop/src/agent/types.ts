import type {
  Track,
  Playlist,
  HotCue,
  PlaylistDetail,
  DuplicateGroup,
  BrokenMetadataReport,
} from "../types";

export type ChangeStatus = "Proposed" | "Accepted" | "Rejected" | "Exported";

/** Mirrors `changes::ChangeKind`. */
export type ChangeKind =
  | "TrackMetadataEdit"
  | "TrackDelete"
  | "TrackRelocate"
  | "CueMetadataEdit"
  | "TrackAddCue"
  | "TrackDeleteCue"
  | "PlaylistCreate"
  | "PlaylistRename"
  | "PlaylistDelete"
  | "PlaylistAddTrack"
  | "PlaylistRemoveTrack"
  | "PlaylistReorderTrack";

export interface StagedChange {
  id: string;
  library_path: string | null;
  kind: ChangeKind;
  target_id: string | null;
  field: string | null;
  old_value: unknown | null;
  new_value: unknown | null;
  reason: string | null;
  confidence: number | null;
  status: ChangeStatus;
  created_at: number;
  updated_at: number;
}

export type NewStagedChange = Omit<
  StagedChange,
  "id" | "status" | "created_at" | "updated_at"
>;

// ── Conversation message types ────────────────────────────────────────────────

export interface TextBlock {
  type: "text";
  text: string;
}

export interface ToolCallBlock {
  type: "tool_call";
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface ToolResultBlock {
  type: "tool_result";
  tool_use_id: string;
  content: string;
}

export type ContentBlock = TextBlock | ToolCallBlock | ToolResultBlock;

export interface UserMessage {
  role: "user";
  text: string;
}

export interface AssistantMessage {
  role: "assistant";
  /** Blocks in order of arrival: text and tool-call blocks. */
  blocks: ContentBlock[];
}

export interface ToolResultMessage {
  role: "tool_results";
  results: ToolResultBlock[];
}

export type ChatMessage = UserMessage | AssistantMessage | ToolResultMessage;

export interface ConversationSummary {
  id: string;
  library_path: string | null;
  title: string;
  created_at: number;
  updated_at: number;
}

export interface PersistedConversationMessage {
  id: string;
  conversation_id: string;
  role: string;
  content: ChatMessage;
  created_at: number;
}

export interface PersistedConversation {
  conversation: ConversationSummary;
  messages: PersistedConversationMessage[];
}

// ── Tool result payload types ─────────────────────────────────────────────────

export interface SearchResult {
  tool: "library.search";
  tracks: Track[];
  query: string;
}

export interface TrackResult {
  tool: "library.get_track";
  track: Track | null;
  id: string;
}

export interface PlaylistsResult {
  tool: "library.list_playlists";
  playlists: Playlist[];
}

export interface PlaylistResult {
  tool: "library.get_playlist";
  detail: PlaylistDetail | null;
  id: string;
}

export interface CuesResult {
  tool: "library.list_cues";
  cues: HotCue[];
  track_id: string;
}

export interface OrphanResult {
  tool: "health.orphan_scan";
  orphans: Track[];
}

export interface DuplicateResult {
  tool: "health.duplicate_scan";
  groups: DuplicateGroup[];
}

export interface FuzzyDuplicateResult {
  tool: "health.fuzzy_duplicate_scan";
  groups: DuplicateGroup[];
}

export interface BrokenLinkResult {
  tool: "health.broken_link_scan";
  report: BrokenMetadataReport;
}

export interface StageChangeResult {
  tool: "staging.stage_change";
  change: StagedChange;
}

export interface LibraryStageIntroCuesResult {
  tool: "library.stage_intro_cues";
  changes: StagedChange[];
}

export interface ListChangesResult {
  tool: "staging.list_changes";
  changes: StagedChange[];
}

export interface RelocateScanResult {
  tool: "relocate.scan";
  candidates: unknown[]; // from types.ts RelocateCandidate
}

export interface RelocateApplyResult {
  tool: "relocate.apply";
  change: StagedChange;
}

export type ToolPayload =
  | SearchResult
  | TrackResult
  | PlaylistsResult
  | PlaylistResult
  | CuesResult
  | OrphanResult
  | DuplicateResult
  | FuzzyDuplicateResult
  | BrokenLinkResult
  | StageChangeResult
  | LibraryStageIntroCuesResult
  | ListChangesResult
  | RelocateScanResult
  | RelocateApplyResult;
