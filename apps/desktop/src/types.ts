/**
 * Mirrors `rekordbox_db::types::CueKind` with serde external tagging:
 *   MemoryCue  → "MemoryCue"
 *   HotCue(n)  → { HotCue: n }
 */
export type CueKind = "MemoryCue" | { HotCue: number };

/** Mirrors `rekordbox_db::types::HotCue`. */
export interface HotCue {
  id: string;
  content_id: string;
  in_msec: number | null;
  out_msec: number | null;
  kind: CueKind;
  color: number | null;
  comment: string | null;
}

/** Mirrors `rekordbox_db::types::Playlist`. */
export interface Playlist {
  id: string;
  name: string;
  kind: "Playlist" | "Folder" | "SmartPlaylist" | { Unknown: number };
  parent_id: string | null;
  seq: number | null;
}

export interface PlaylistDetail {
  playlist: Playlist;
  tracks: Track[];
}

/** Mirrors `rekordbox_db::types::DuplicateKind`. */
export type DuplicateKind =
  | "ExactTitleArtist"
  | "FuzzyTitle"
  | "AudioFingerprint";

export interface DuplicateGroup {
  title: string;
  artist: string | null;
  tracks: Track[];
  /** Detection strategy; defaults to ExactTitleArtist for legacy responses. */
  kind?: DuplicateKind;
  /** Confidence in 0.0..=1.0. */
  confidence?: number;
}

export interface BrokenMetadataReport {
  missing_artist: Track[];
  missing_bpm: Track[];
  missing_key: Track[];
  missing_genre: Track[];
  suspicious: Track[];
}

export interface LibraryAnalytics {
  total_tracks: number;
  genre_distribution: Record<string, number>;
  bpm_histogram: Record<number, number>;
  key_distribution: Record<string, number>;
}

export interface GenreCount {
  genre: string;
  count: number;
}

export interface ArtistCount {
  artist: string;
  count: number;
}

export interface TagCategory {
  id: string;
  name: string;
  seq: number;
}

export interface Tag {
  id: string;
  category_id: string;
  name: string;
  seq: number;
  /** Number of track ↔ tag bindings across all libraries. Surfaced as a "(N)"
   *  badge in the Custom Tags panel. */
  usage_count: number;
}

/** Mirrors `audio_tags::TrackTags`. */
export interface TrackTags {
  title: string | null;
  artist: string | null;
  album: string | null;
  genre: string | null;
  bpm: number | null;
  musical_key: string | null;
  comment: string | null;
  year: number | null;
  rating: number | null;
  duration_secs: number | null;
  file_type: string | null;
}

/** Mirrors `audio_tags::TagWriteFields`. */
export interface TagWriteFields {
  title?: string | null;
  artist?: string | null;
  album?: string | null;
  genre?: string | null;
  bpm?: number | null;
  musical_key?: string | null;
  comment?: string | null;
  year?: number | null;
}

/** Mirrors `audio_analysis::AnalysisResult`. */
export interface AnalysisResult {
  bpm: number;
  musical_key: string;
  confidence: number;
  bpm_confidence: number;
  key_confidence: number;
  cached: boolean;
}

/** Mirrors `rekordbox_db::types::Track` (serde snake_case). */
export interface Track {
  id: string;
  title: string;
  artist: string | null;
  album: string | null;
  genre: string | null;
  musical_key: string | null;
  bpm: number | null;
  duration_secs: number | null;
  rating: number | null;
  comment: string | null;
  folder_path: string | null;
  analysis_data_path: string | null;
  file_type: number | null;
  sample_rate: number | null;
  bit_rate: number | null;
  release_year: number | null;
  dj_play_count: number | null;
  /** 0.0–1.0 audio energy, hydrated from `audio_features` cache. */
  energy: number | null;
}

export interface BeatGridEntry {
  beat_number: number;
  tempo_bpm_x100: number;
  time_ms: number;
}

export type WaveformColor =
  | { type: "Blue"; value: number }
  | { type: "Rgb"; value: [number, number, number] };

export interface PreviewPoint {
  height: number;
  color: WaveformColor;
}

export interface DetailPoint {
  height: number;
  color: WaveformColor;
}

export interface AnlzWaveform {
  preview: PreviewPoint[];
  detail: DetailPoint[];
  beat_grid: BeatGridEntry[];
  peaks: number[] | null;
}

export interface RelocateMatch {
  path: string;
  score: number;
  reasons: string[];
}

export interface RelocateCandidate {
  track_id: string;
  original_path: string;
  matches: RelocateMatch[];
}

export interface TransitionScore {
  score: number;
  reasons: string[];
}

// ── Smartlists (Epic 1) ──────────────────────────────────────────────────────
// Mirrors `crates/smartlists`. See docs/lexicon/03-smartlists.md.

export type SmartlistCombinator = "all" | "any";

export type SmartlistField =
  | "title"
  | "artist"
  | "album"
  | "genre"
  | "comment"
  | "file_path"
  | "musical_key"
  | "bpm"
  | "rating"
  | "year"
  | "duration_secs"
  | "bit_rate"
  | "sample_rate"
  | "play_count"
  | "energy"
  | "has_cues"
  | "in_any_playlist"
  | "is_file_missing"
  | "is_archived"
  | "tags";

export type SmartlistFieldKind = "text" | "key" | "number" | "bool" | "tags";

export type SmartlistOperator =
  | "contains"
  | "not_contains"
  | "equals"
  | "not_equals"
  | "is_none"
  | "is_not_none"
  | "greater_than"
  | "less_than"
  | "greater_or_equal"
  | "less_or_equal"
  | "between"
  | "is_true"
  | "is_false"
  | "has_all"
  | "has_any"
  | "has_none";

/** Serde-tagged union — matches `smartlists::Value`. */
export type SmartlistValue =
  | { type: "text"; value: string }
  | { type: "number"; value: number }
  | { type: "range"; value: [number, number] }
  | { type: "tags"; value: string[] }
  | { type: "none" };

export interface SmartlistRule {
  field: SmartlistField;
  op: SmartlistOperator;
  value: SmartlistValue;
}

/** Rules within a clause are OR-ed. Clauses are AND-ed when combinator is
 *  "all" — the two-level structure from ADR-0013. */
export interface SmartlistClause {
  rules: SmartlistRule[];
}

export interface Smartlist {
  id: string;
  name: string;
  parent_folder_id: string | null;
  combinator: SmartlistCombinator;
  clauses: SmartlistClause[];
  created_at: number;
  updated_at: number;
}

export type SmartlistCompatibility =
  | { native: Record<string, never> }
  | "native"
  | { materialised: { reason: string } };

export type SmartlistGeneratorSpec =
  | { kind: "by_field"; field: SmartlistField }
  | {
      kind: "by_tag_category";
      category_id: string;
      category_name: string;
      tags: [string, string][];
    }
  | { kind: "by_decade" }
  | { kind: "by_bpm_range"; width: number }
  | { kind: "by_play_count"; threshold: number };

// ── Cue editing (Epic 2) ─────────────────────────────────────────────────────

/** Resolutions the quantiser offers. Mirrors `rekordbox_db::quantize`. */
export type QuantizeResolution =
  | "beat"
  | "two_beats"
  | "bar"
  | "four_bars"
  | "sixteen_bars";

/** Columns `changes::applier::cues` allows editing. */
export type CueField = "InMsec" | "OutMsec" | "Kind" | "Color" | "Commnt";

export interface CueInput {
  in_msec: number;
  /** Set to make the cue a loop. */
  out_msec?: number | null;
  /** 0 = memory cue, 1–8 = hot cue slot. */
  kind: number;
  color?: number | null;
  comment?: string | null;
}

// ── Cue Point Generator (Epic 3) ─────────────────────────────────────────────

export type CueAnchor =
  | { kind: "start" }
  | { kind: "drop"; ordinal: number }
  | { kind: "breakdown"; ordinal: number }
  | { kind: "fade_out" }
  | { kind: "end" };

/** `Certain` for human-placed anchors; `Detected` carries a 0–1 score. */
export type CueConfidence = "certain" | { detected: number };

export interface ResolvedAnchor {
  anchor: CueAnchor;
  position_ms: number;
  confidence: CueConfidence;
}

export interface CustomAnchorRule {
  anchor: CueAnchor;
  name?: string | null;
  color?: number | null;
}

export type StartCueBehavior = "first_beat" | "existing_cue" | "zero";

export interface CueTemplateEntry {
  anchor: CueAnchor;
  offset_beats: number;
  name: string;
  color?: number | null;
  enabled: boolean;
  memory_cue: boolean;
  loop_beats?: number | null;
}

export interface CueTemplate {
  name: string;
  entries: CueTemplateEntry[];
  start_behavior: StartCueBehavior;
  keep_cue_position: boolean;
}

export interface GeneratedCue {
  position_ms: number;
  name: string;
  color: number | null;
  slot: number;
  memory_cue: boolean;
  loop_end_ms: number | null;
  confidence: CueConfidence;
  template_index: number;
}

export type SkippedCue =
  | { reason: "anchor_missing"; name: string; anchor: CueAnchor }
  | { reason: "out_of_range"; name: string; position_ms: number }
  | { reason: "overflow"; name: string }
  | { reason: "duplicate_memory_cue"; name: string; position_ms: number };

export interface GeneratePreview {
  cues: GeneratedCue[];
  skipped: SkippedCue[];
  anchors: ResolvedAnchor[];
}

// ── File organiser (Epic 4) ──────────────────────────────────────────────────

export type SubfolderPattern =
  | { kind: "field"; name: string }
  | { kind: "bitrate_bucket" }
  | { kind: "first_tag" }
  | { kind: "current_year" }
  | { kind: "current_month" }
  | { kind: "current_decade" }
  | { kind: "release_decade" };

export interface SubfolderSpec {
  levels: SubfolderPattern[];
}

export interface OrganizeRequest {
  /** Absent renames in place. */
  target_folder?: string | null;
  /** Absent keeps the existing filename. */
  filename_pattern?: string | null;
  subfolders: SubfolderSpec;
}

export interface OrganizeRow {
  track_id: string;
  source: string;
  /** Null when the file is already where it belongs. */
  destination: string | null;
  title: string;
  artist: string | null;
}

export interface OrganizeResult {
  moved: string[];
  failed: [string, string][];
  staged: string[];
}

export interface PatternField {
  name: string;
  supported: boolean;
}

export type ExtensionMode = "include" | "exclude";

export interface ExtensionFilter {
  mode: ExtensionMode;
  extensions: string[];
}

export interface UnusedFile {
  path: string;
  size_bytes: number;
}

export interface UnusedScan {
  files: UnusedFile[];
  total_bytes: number;
  skipped_directories: string[];
  errors: string[];
}

export interface DeleteReport {
  deleted: string[];
  failed: [string, string][];
  report_path: string | null;
}

export interface TagFieldSelection {
  title: boolean;
  artist: boolean;
  album: boolean;
  genre: boolean;
  bpm: boolean;
  musical_key: boolean;
  comment: boolean;
  year: boolean;
}

export interface WriteTagsResult {
  written: string[];
  failed: [string, string][];
  skipped: string[];
}

export interface PathMappingRow {
  id: string;
  from: string;
  to: string;
}

export interface QuickMoveFolder {
  id: string;
  path: string;
  favourite: boolean;
  last_used_at: number;
}

export interface WatchFolderRow {
  id: string;
  path: string;
}

export interface Arrival {
  path: string;
  size_bytes: number;
  /** Seconds since last modification, as of the scan. */
  age_secs: number;
}

export interface WatchScan {
  arrivals: Arrival[];
  /** Files still being written — reported so the UI can say so. */
  pending: Arrival[];
  errors: string[];
}

export interface ImportResult {
  staged: string[];
  failed: [string, string][];
}

export interface AutomaticAction {
  key: string;
  label: string;
  description: string;
  enabled: boolean;
  /** Non-null when decks cannot honour the action yet; the toggle is disabled. */
  unavailable: string | null;
}

export type MappingSource =
  | { kind: "energy" }
  | { kind: "danceability" }
  | { kind: "popularity" }
  | { kind: "happiness" }
  | { kind: "all_custom_tags" }
  | { kind: "tag_category"; name: string }
  | { kind: "colour" };

export interface FieldMappingRow {
  id: string;
  source: MappingSource;
  target: string;
  overwrite: boolean;
}
