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
  /** `#rrggbb`, or null for no colour. Absent is the normal state. */
  color: string | null;
}

export interface Tag {
  id: string;
  category_id: string;
  name: string;
  seq: number;
  /** Number of track ↔ tag bindings across all libraries. Surfaced as a "(N)"
   *  badge in the Custom Tags panel. */
  usage_count: number;
  /** 1–9, bound to the number row in the tag popup. Global across the whole
   *  tag tree, so no two tags share one. */
  hotkey: number | null;
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
  /** Record label, from `djmdLabel`. */
  label: string | null;
  /** Remixer, from `djmdArtist` via `RemixerID`. */
  remixer: string | null;
  /** Mix name — "Extended Mix", "Radio Edit" — from `djmdContent.Subtitle`. */
  mix: string | null;
  /** Rekordbox's own colour label, by name rather than id. */
  color: string | null;
  /** ISO-8601 as stored; format varies by library, so it is not reformatted. */
  date_added: string | null;
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
  | "label"
  | "remixer"
  | "mix"
  | "color"
  | "date_added"
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

export type SmartlistFieldKind =
  | "text"
  | "key"
  | "number"
  | "bool"
  | "tags"
  | "date";

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
  | { type: "text_range"; value: [string, string] }
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

/** One edit the Rekordbox field mappings would make. */
export interface MappingProposal {
  id: string;
  track_id: string;
  track_title: string;
  /** `djmdContent` column name, in the applier's vocabulary. */
  target: string;
  before: string | null;
  after: string;
}

export interface MappingPreview {
  proposals: MappingProposal[];
  /** Targets the applier will not write, named rather than dropped. */
  unwritable_targets: string[];
  /** Tracks a mapping produced nothing for, so "0 proposals" reads as
   *  "nothing to change" rather than as a broken configuration. */
  unchanged: number;
}

export interface FieldMappingRow {
  id: string;
  source: MappingSource;
  target: string;
  overwrite: boolean;
}

// ── Recipes (Epic 5) ─────────────────────────────────────────────────────────

export type DelimiterPair =
  | "parentheses"
  | "brackets"
  | "braces"
  | "angles"
  | "double_quotes"
  | "single_quotes";

export type SpecialCharacterMode = "special" | "emojis";

/** Mirrors `recipes::Recipe`. */
export type Recipe =
  | { op: "to_upper_case"; field: string; ignore_words: string[] }
  | { op: "to_lower_case"; field: string; ignore_words: string[] }
  | { op: "to_title_case"; field: string; ignore_words: string[] }
  | { op: "to_sentence_case"; field: string }
  | { op: "copy_field"; from: string; to: string }
  | { op: "move_field"; from: string; to: string }
  | {
      op: "merge_fields";
      first: string;
      second: string;
      target: string;
      separator: string;
    }
  | { op: "prefix_field"; field: string; text: string }
  | { op: "suffix_field"; field: string; text: string }
  | { op: "swap_fields"; first: string; second: string }
  | {
      op: "split_field";
      field: string;
      delimiter: string;
      first_target: string;
      second_target: string;
      preserve_split_text: boolean;
      append: boolean;
    }
  | { op: "remove_text"; field: string; text: string; case_insensitive: boolean }
  | {
      op: "replace_text";
      field: string;
      find: string;
      replace: string;
      case_insensitive: boolean;
    }
  | {
      op: "extract_text";
      field: string;
      start: string;
      end: string;
      target: string;
      include_delimiters: boolean;
      delete_from_source: boolean;
      append: boolean;
    }
  | { op: "shorten_text"; field: string; chars_per_word: number }
  | { op: "remove_special_characters"; field: string; mode: SpecialCharacterMode }
  | { op: "remove_between"; field: string; pair: DelimiterPair }
  | { op: "adjust_number"; field: string; amount: number };

export interface RecipeProposal {
  id: string;
  track_id: string;
  track_title: string;
  field: string;
  before: string | null;
  after: string | null;
}

export interface RecipePreview {
  proposals: RecipeProposal[];
  /** `[track_id, reason]` for tracks a recipe could not act on. */
  skipped: [string, string][];
}

export type TagRecipe =
  | { op: "import_from_text"; field: string; separator: string }
  | { op: "add_tags"; tags: string[] }
  | { op: "remove_tags"; tags: string[] }
  | { op: "replace_tag"; from: string; to: string }
  | { op: "clear_tags" };

export interface TagProposal {
  track_id: string;
  track_title: string;
  added: string[];
  removed: string[];
}

export interface TagApplyResult {
  tracks_changed: number;
  tags_added: number;
  tags_removed: number;
  tags_created: string[];
}

export type OtherRecipe =
  | "mark_as_incoming"
  | "remove_from_all_playlists"
  | "import_date_from_filesystem";

export interface OtherRecipeResult {
  changed: string[];
  staged: string[];
  skipped: [string, string][];
}

export type CueDeleteMode =
  | "all"
  | "first"
  | "last"
  | "keep_first"
  | "keep_last"
  | "loops_only"
  | "without_colour"
  | "without_text"
  | "memory_cues";

export type CueColourScheme =
  | "basic"
  | "grayscale"
  | "cold"
  | "warm"
  | "cycle"
  | "none"
  | "first_cue_colour";

export type CueSortOrder =
  | "time_asc"
  | "time_desc"
  | "label_asc"
  | "label_desc"
  | "empty_labels_first"
  | "empty_labels_last"
  | "cues_before_loops"
  | "loops_before_cues";

export type CueRecipe =
  | { op: "delete_cues"; mode: CueDeleteMode }
  | { op: "change_colours"; scheme: CueColourScheme }
  | {
      op: "find_and_replace";
      match_text: string | null;
      match_colour: number | null;
      new_text: string | null;
      new_colour: number | null;
    }
  | { op: "sort_cues"; order: CueSortOrder }
  | {
      op: "replace_cue_text";
      find: string;
      replace: string;
      case_insensitive: boolean;
    }
  | { op: "remove_cue_text" }
  | { op: "remove_cues_by_label"; text: string }
  | { op: "shift_cues"; offset_ms: number }
  | { op: "quantize_cues"; resolution_beats: number };

/** One `djmdCue` column edit, ready to stage. Values are typed, not stringified. */
export interface CueChange {
  cue_id: string;
  cue_label: string;
  field: string;
  before: unknown;
  after: unknown;
}

export interface CueDeletion {
  cue_id: string;
  cue_label: string;
}

export interface CueRecipeTrack {
  track_id: string;
  track_title: string;
  edits: CueChange[];
  deletions: CueDeletion[];
  skipped: string | null;
}

/**
 * One Sync run, as the undo list shows it.
 *
 * `reversible` and `blocked` are counts over the run's entries — a run is
 * rarely all-or-nothing, and the list says so up front rather than after the
 * user has clicked Undo.
 */
export interface UndoRun {
  id: string;
  library_path: string;
  applied_at: number;
  /** Set once the run's inverses have been staged. */
  undone_at: number | null;
  reversible: number;
  blocked: number;
}

export interface UndoEntry {
  id: string;
  source_change_id: string;
  /** `null` when the entry cannot be reversed; `blocked_reason` says why. */
  kind: string | null;
  target_id: string | null;
  field: string | null;
  old_value: unknown;
  new_value: unknown;
  description: string;
  blocked_reason: string | null;
}

export interface UndoResult {
  staged: string[];
  /** `[description, reason]` for entries that could not be reversed. */
  blocked: [string, string][];
}

// ── Import Tags From CSV (Epic 5) ────────────────────────────────────────────
// Mirrors `track_matcher::csv_import`. See docs/lexicon/10-recipes.md.

export interface CsvImportColumns {
  /** File-path column. Matched against the track's folder path. */
  location: string | null;
  artist: string | null;
  title: string | null;
  /** `[csv header, track field]` — the values to write. */
  fields: [string, string][];
}

export interface CsvImportRow {
  /** 1-based, counting the header as row 1 — what the spreadsheet shows. */
  line: number;
  location: string | null;
  artist: string | null;
  title: string | null;
  values: Record<string, string>;
}

export type CsvRowOutcome =
  | {
      kind: "matched";
      track_id: string;
      track_title: string;
      /** `[field, before, after]`. */
      changes: [string, string | null, string][];
    }
  | { kind: "already_current"; track_id: string }
  | { kind: "unmatched" }
  | { kind: "ambiguous"; count: number };

export interface CsvPlannedRow {
  row: CsvImportRow;
  outcome: CsvRowOutcome;
}

export interface CsvImportReport {
  rows: number;
  matched: number;
  already_current: number;
  unmatched: number;
  ambiguous: number;
  changes: number;
}

export interface CsvImportPreview {
  rows: CsvPlannedRow[];
  report: CsvImportReport;
}

// ── Manual multi-track editing (Epic 5) ──────────────────────────────────────
// Mirrors `changes::multi_edit`. See docs/lexicon/02-library.md §Manual Editing.

/** `multiple` when the selection disagrees — deliberately carries no value. */
export type MultiEditFieldValue =
  | { kind: "same"; value: string | null }
  | { kind: "multiple" };

export interface MultiEditFormData {
  fields: [string, MultiEditFieldValue][];
  track_count: number;
}

/** A field the user actually changed. `null` clears it. */
export interface MultiEdit {
  field: string;
  value: string | null;
}

// ── Find Broken Tracks (Epic 5) ──────────────────────────────────────────────
// Mirrors `audio_analysis::playable`. See docs/lexicon/07-health.md.

/** `header` is fast and misses late corruption; `full` decodes everything. */
export type CheckDepth = "header" | "full";

export type PlaybackStatus =
  | { kind: "ok" }
  | { kind: "missing" }
  | { kind: "unreadable"; detail: string }
  | { kind: "undecodable"; detail: string }
  | { kind: "truncated"; detail: string }
  | { kind: "damaged"; detail: { bad_packets: number } };

export interface BrokenTrack {
  track_id: string;
  title: string;
  artist: string | null;
  path: string;
  status: PlaybackStatus;
  /** Playlists holding this track — what makes sourcing a replacement possible. */
  playlists: string[];
}

export interface BrokenScan {
  broken: BrokenTrack[];
  checked: number;
  /** Tracks with no file path at all: nothing to check, and not a failure. */
  no_path: number;
}

// ── Archive (Epic 5) ─────────────────────────────────────────────────────────
// See docs/lexicon/02-library.md §Archive.

export type ArchiveCriterion =
  | { kind: "older_than_days"; value: number }
  | { kind: "without_cues" }
  | { kind: "in_no_playlist" };

export interface ArchiveResult {
  archived: string[];
  /** Staged `PlaylistRemoveTrack` ids — only when archiving from a playlist. */
  staged: string[];
}

// ── Database backup (Epic 5) ─────────────────────────────────────────────────
// Mirrors `cache::backup`. See docs/lexicon/09-history-backup.md.

export interface BackupSummary {
  path: string;
  rows: number;
  /** `[table, rows]` — what the backup holds, rather than a byte count. */
  tables: [string, number][];
}

export interface RestoreReport {
  restored: [string, number][];
  /** Tables the backup named that this build does not have. */
  unknown_tables: string[];
  /** `[table, column]` the backup had and this build does not. */
  dropped_columns: [string, string][];
}

// ── Duplicate resolution (Epic 5) ────────────────────────────────────────────
// See docs/lexicon/07-health.md §Find Duplicates.

export interface DuplicateCandidate {
  track_id: string;
  bit_rate: number | null;
  duration_secs: number | null;
  has_cues: boolean;
  rating: number | null;
  play_count: number | null;
  in_playlists: number;
}

/** `best` is the default heuristic: cues, then bitrate, then playlists, then plays. */
export type PreferRule =
  | "best"
  | "highest_bitrate"
  | "has_cues"
  | "most_playlists"
  | "longest";

export interface ResolutionPlan {
  keeper_id: string;
  loser_ids: string[];
  /** `[playlist_id, playlist_name, loser_id]` — memberships to re-point. */
  repoint: [string, string, string][];
  /** Playlists already holding the keeper, so the loser is removed not swapped. */
  already_present: string[];
}

export interface ResolveResult {
  archived: string[];
  staged: string[];
}

// ── Path rewriting (Epic 5) ──────────────────────────────────────────────────
// Mirrors `relocate::rewrite`. See docs/lexicon/07-health.md.

export interface RewriteSpec {
  from_prefix: string;
  to_prefix: string;
  /** The WAV→MP3 re-encode case. */
  new_extension: string | null;
  /** Rewrite working paths too, not only missing ones. Off by default. */
  all_tracks: boolean;
}

export type RewriteSkipReason =
  | { kind: "no_match" }
  | { kind: "not_missing" }
  | { kind: "unchanged" }
  | { kind: "taken"; detail: string };

export interface PathRewrite {
  track_id: string;
  from: string;
  to: string;
}

export interface RewritePlan {
  rewrites: PathRewrite[];
  /** `[track_id, path, reason]` for every track passed over. */
  skipped: [string, string, RewriteSkipReason][];
}

export interface RewritePreview {
  plan: RewritePlan;
  considered: number;
}

// ── Mixable Tracks (Epic 6) ──────────────────────────────────────────────────

/** The global setting, shared with the browser's compatible-key indicator. */
export type KeyMixingMode = "harmonically_compatible" | "fuzzy";

export type KeyRelation =
  | "same"
  | "relative_major_minor"
  | "adjacent_same_mode"
  | "adjacent_opposite_mode";

export type BpmRelation = "direct" | "half" | "double";

/** A rule over Energy or Rating. `near_source` is the spec's "input ±1". */
export type NumericRule =
  | { kind: "off" }
  | { kind: "near_source" }
  | { kind: "range"; min: number; max: number };

export type YearRule =
  | { kind: "off" }
  | { kind: "same_as_source" }
  | { kind: "range"; min: number; max: number };

export interface MixableOptions {
  /** Percentage of the source tempo. `null` accepts any tempo. */
  bpm_tolerance_pct: number | null;
  match_key: boolean;
  key_mixing_mode: KeyMixingMode;
  include_half_double: boolean;
  must_have_cues: boolean;
  genres: string[];
  year: YearRule;
  energy: NumericRule;
  rating: NumericRule;
  /** Tag ids. */
  must_have_tags: string[];
  must_not_have_tags: string[];
  /** Only candidates carrying the source track's colour. */
  match_color: boolean;
  /** ISO-8601 date; only candidates added on or after it. `null` is off. */
  added_since: string | null;
  limit: number;
}

export interface MixableMatch {
  track: Track;
  score: number;
  reasons: string[];
  bpm_relation: BpmRelation;
  key_relation: KeyRelation | null;
}

export interface MixableResult {
  source: Track;
  matches: MixableMatch[];
  /** Tracks weighed before the rules ran, so an empty list can say "0 of N". */
  considered: number;
  compatible_keys: string[];
}

export interface MixableTemplate {
  id: string;
  name: string;
  options: MixableOptions;
  created_at: number;
}

// ── Playlist Tools (Epic 6) ──────────────────────────────────────────────────

export type PlaylistSortMode = "name_asc" | "name_desc" | "track_count_desc";
export type CrossReferenceMode = "in_all" | "in_none";

export interface MergePreview {
  track_ids: string[];
  /** Rows across the sources before duplicates were dropped. */
  source_rows: number;
}

export interface SortPreview {
  /** `[playlist_id, name]` in the new order. */
  order: [string, string][];
  unchanged: boolean;
}

export interface CrossReferencePreview {
  track_ids: string[];
  considered: number;
}

export interface Numbering {
  start: number;
  /** Zero-pad width. 2 gives 01, 02, … */
  pad: number;
  /** Strip a number already at the front, so prefixes do not stack. */
  replace_existing: boolean;
}

export interface PrefixSpec {
  text: string;
  numbering: Numbering | null;
}

export interface PlaylistRenamePlan {
  id: string;
  from: string;
  to: string;
}

export interface OccurrenceReport {
  tracks: Track[];
  /** `[playlist_count, how_many_tracks]`, ascending. */
  distribution: [number, number][];
}

export interface RewriteOrderPlan {
  playlist_id: string;
  order: string[];
  /** Requested ids that are not in the playlist. */
  unknown: string[];
  /** Playlist members the visible order left out; appended, never dropped. */
  appended: string[];
  unchanged: boolean;
}

// ── Share / export (Epic 6) ──────────────────────────────────────────────────

export type ShareFormat =
  | "quick_copy"
  | "quick_copy_numbered"
  | "csv"
  | "m3u"
  | "html";

/** Mirrors `share::Column`. A closed set: an unknown name fails to parse
 *  rather than becoming a blank column that looks like missing data. */
export type ShareColumn =
  | "title"
  | "artist"
  | "album"
  | "genre"
  | "key"
  | "bpm"
  | "duration"
  | "rating"
  | "year"
  | "comment"
  | "bitrate"
  | "play_count"
  | "energy"
  | "path";

export interface ShareExport {
  content: string;
  /** Sanitised suggested filename. */
  filename: string;
  track_count: number;
  /** Titles the format could not carry — M3U tracks with no file path. */
  skipped: string[];
}

// ── Favourite playlists (Epic 6) ─────────────────────────────────────────────

export interface FavouritePlaylist {
  playlist_id: string;
  name: string;
  /** 1-based hotkey position. Favourite n is bound to key n. */
  seq: number;
  track_count: number;
}

/** Hotkeys stop at 9, so there is no point storing a tenth. */
export const MAX_FAVOURITE_PLAYLISTS = 9;

// ── Play history (Epic 6) ────────────────────────────────────────────────────

export interface HistorySet {
  id: string;
  /** The `djmdHistory.ID` this came from — what makes re-import idempotent. */
  source_id: string;
  name: string;
  played_at: string | null;
  rating: number | null;
  location: string | null;
  track_count: number;
}

/** A track as it was at play time. A snapshot, not a join. */
export interface HistoryTrack {
  id: string;
  seq: number;
  content_id: string | null;
  title: string | null;
  artist: string | null;
  album: string | null;
  genre: string | null;
  musical_key: string | null;
  bpm: number | null;
  duration_secs: number | null;
  folder_path: string | null;
}

export interface HistoryImportReport {
  imported: number;
  already_known: number;
  previously_deleted: number;
}

/** How a snapshot row was matched back to a live track. */
export type MatchKind = "content_id" | "path" | "filename" | "none";

export interface HistoryMatch {
  history_track_id: string;
  title: string | null;
  artist: string | null;
  track_id: string | null;
  kind: MatchKind;
}

export interface HistoryMatchReport {
  matches: HistoryMatch[];
  matched: number;
  unmatched: number;
}

// ── M3U import ───────────────────────────────────────────────────────────────

export interface M3uImportRow {
  path: string;
  /** The `#EXTINF` label — the only identifier left for an unmatched row. */
  label: string | null;
  track_id: string | null;
}

export interface M3uImportPreview {
  rows: M3uImportRow[];
  matched: number;
  unmatched: number;
  suggested_name: string;
}

// ── Delete from disk ─────────────────────────────────────────────────────────

/** Why a track will not be deleted. Discriminated on `kind`; the backend
 *  resolves the sentence into `Refused.message`, so the renderer never has to
 *  reimplement the match. */
export type Refusal =
  | { kind: "no_path" }
  | { kind: "missing" }
  | { kind: "not_a_regular_file" }
  | { kind: "symlink" }
  | { kind: "outside_music_roots" }
  | { kind: "shared_with_tracks"; track_ids: string[] }
  | { kind: "still_in_playlists"; playlists: string[] }
  | { kind: "already_quarantined" };

export interface PlannedDelete {
  track_id: string;
  source: string;
  bytes: number;
}

export interface RefusedDelete {
  track_id: string;
  path: string;
  reason: Refusal;
  message: string;
}

export interface DeletePlanView {
  deletable: PlannedDelete[];
  refused: RefusedDelete[];
  /** Bytes the quarantine will hold — not bytes freed. */
  total_bytes: number;
  /** Track id → "Artist — Title". */
  labels: Record<string, string>;
  /** Everything was refused because Settings has no music folders yet. */
  no_roots_configured: boolean;
}

export interface DeleteManifestEntry {
  track_id: string;
  original_path: string;
  stored_as: string;
  bytes: number;
}

export interface DeleteManifest {
  batch_id: string;
  created_at: number;
  library_path: string;
  reason: string;
  entries: DeleteManifestEntry[];
}

export interface DeleteBatch {
  manifest: DeleteManifest;
  total_bytes: number;
  file_count: number;
}

export interface MoveFailure {
  track_id: string;
  path: string;
  error: string;
}

export interface DeleteReceipt {
  manifest: DeleteManifest;
  failed: MoveFailure[];
}

export type QuarantineRestoreOutcome =
  | { outcome: "restored"; path: string }
  | { outcome: "occupied"; path: string }
  | { outcome: "missing_from_quarantine" }
  | { outcome: "failed"; error: string };

export interface QuarantineRestoreResult {
  track_id: string;
  original_path: string;
  outcome: QuarantineRestoreOutcome;
}

export interface QuarantineRestoreReport {
  batch_id: string;
  results: QuarantineRestoreResult[];
  restored: number;
  batch_emptied: boolean;
}

export interface MusicRootSuggestion {
  path: string;
  track_count: number;
}

// ── Cue presets (Epic 2) ─────────────────────────────────────────────────────

/**
 * A saved name+colour pair for the cue editor.
 *
 * Distinct from `CueTemplate`, which is the Cue Point Generator's bulk rule
 * set. Two things called "template" in one player would be unreadable, so the
 * spec's `Cue templates` ship here as presets.
 */
export interface CuePreset {
  id: string;
  name: string;
  color: number | null;
  /** 1–8 for the presets that carry a number key; null beyond that. */
  hotkey: number | null;
}

// ── MyTag import (Epic 5) ────────────────────────────────────────────────────

export interface MyTagImportPreview {
  new_categories: string[];
  /** `[category, tag]` pairs. */
  new_tags: [string, string][];
  existing_tags: number;
  new_links: number;
  /** Links whose track is not in this library — a large number means the
   *  MyTag data came from a different collection. */
  unmatched_links: number;
}

export interface MyTagImportResult {
  categories_created: number;
  tags_created: number;
  links_created: number;
}
