use serde::{Deserialize, Serialize};

/// A track from `djmdContent` with denormalised artist/album/genre/key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    /// Camelot-wheel key notation, e.g. "8A", "11B".
    pub musical_key: Option<String>,
    /// Actual BPM (DB stores integer × 100; we convert at read time).
    pub bpm: Option<f64>,
    /// Track duration in seconds.
    pub duration_secs: Option<i64>,
    pub rating: Option<i64>,
    pub comment: Option<String>,
    /// Full path to the audio file on disk.
    pub folder_path: Option<String>,
    /// Relative path to the ANLZ analysis directory (used to locate beat-grid files).
    pub analysis_data_path: Option<String>,
    pub file_type: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_rate: Option<i64>,
    pub release_year: Option<i64>,
    pub dj_play_count: Option<i64>,
    /// Record label, from `djmdLabel` via `djmdContent.LabelID`.
    ///
    /// Writing this already worked — `changes::applier` has handled `Label` as
    /// a foreign-key edit since Epic 5. Only the read side was missing, which
    /// meant the browser could set a label it could never show back.
    #[serde(default)]
    pub label: Option<String>,
    /// Remixer, from `djmdArtist` via `djmdContent.RemixerID`.
    #[serde(default)]
    pub remixer: Option<String>,
    /// Mix name — `djmdContent.Subtitle`, which is where Rekordbox keeps
    /// "Extended Mix", "Radio Edit" and friends.
    #[serde(default)]
    pub mix: Option<String>,
    /// Rekordbox's own colour label for the track.
    ///
    /// The *name*, not the id: an id is meaningless outside the database it
    /// came from, and "Red" is the thing the user sorted by.
    #[serde(default)]
    pub color: Option<String>,
    /// When Rekordbox first saw the track — `djmdContent.DateCreated`, as the
    /// ISO-8601 string the column stores. Not parsed here: the column's format
    /// varies between libraries, and pretending otherwise loses information.
    #[serde(default)]
    pub date_added: Option<String>,
    /// Audio energy 0.0–1.0, hydrated from the local cache's `audio_features`
    /// table at the Tauri layer. `None` when no analysis has been cached.
    #[serde(default)]
    pub energy: Option<f32>,
}

/// A playlist or folder from `djmdPlaylist`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub seq: Option<i64>,
    pub kind: PlaylistKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlaylistKind {
    Playlist,
    Folder,
    SmartPlaylist,
    Unknown(i64),
}

impl PlaylistKind {
    pub fn from_attribute(attr: i64) -> Self {
        match attr {
            0 => Self::Playlist,
            1 => Self::Folder,
            4 => Self::SmartPlaylist,
            n => Self::Unknown(n),
        }
    }
}

/// A single track entry inside a playlist, from `djmdSongPlaylist`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistEntry {
    pub playlist_id: String,
    pub content_id: String,
    pub track_no: Option<i64>,
}

/// Why a `DuplicateGroup` was flagged. Tags every group so the UI can render a
/// per-row header explaining the match strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuplicateKind {
    /// Identical lowercased+trimmed title AND artist.
    ExactTitleArtist,
    /// Fuzzy title signature (strips remix annotations, "feat." markers, etc.)
    /// plus primary-artist normalisation.
    FuzzyTitle,
    /// Chromagram fingerprint within a Hamming-distance threshold.
    AudioFingerprint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub title: String,
    pub artist: Option<String>,
    pub tracks: Vec<Track>,
    /// Detection strategy. Defaults to `ExactTitleArtist` for back-compat.
    #[serde(default = "default_dup_kind")]
    pub kind: DuplicateKind,
    /// Confidence in 0.0..=1.0. Exact matches are 1.0; fuzzy/fingerprint vary.
    #[serde(default = "default_dup_confidence")]
    pub confidence: f32,
}

fn default_dup_kind() -> DuplicateKind {
    DuplicateKind::ExactTitleArtist
}

fn default_dup_confidence() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BrokenMetadataReport {
    pub missing_artist: Vec<Track>,
    pub missing_bpm: Vec<Track>,
    pub missing_key: Vec<Track>,
    pub missing_genre: Vec<Track>,
    pub suspicious: Vec<Track>,
}

/// A hot cue or memory cue from `djmdCue`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotCue {
    pub id: String,
    pub content_id: String,
    pub in_msec: Option<i64>,
    pub out_msec: Option<i64>,
    pub kind: CueKind,
    /// Rekordbox color ID; -1 means unset.
    pub color: Option<i64>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CueKind {
    /// `djmdCue.Kind = 0`
    MemoryCue,
    /// `djmdCue.Kind = 1–8`
    HotCue(u8),
}

impl CueKind {
    pub fn from_db(kind: i64) -> Self {
        match kind {
            0 => Self::MemoryCue,
            n if (1..=8).contains(&n) => Self::HotCue(n as u8),
            n => Self::HotCue(n.clamp(1, 8) as u8),
        }
    }
}

/// One beat entry from an ANLZ `.DAT` file's PQTZ section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeatGridEntry {
    /// Beat position within the bar: 1–4.
    pub beat_number: u16,
    /// BPM × 100 (e.g. 12800 = 128.00 bpm).
    pub tempo_bpm_x100: u16,
    /// Absolute position in the track, milliseconds.
    pub time_ms: u32,
}

impl BeatGridEntry {
    pub fn bpm(&self) -> f64 {
        self.tempo_bpm_x100 as f64 / 100.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LibraryAnalytics {
    pub total_tracks: usize,
    /// Genre name -> Count
    pub genre_distribution: std::collections::HashMap<String, usize>,
    /// Floor(BPM) -> Count
    pub bpm_histogram: std::collections::HashMap<u16, usize>,
    /// Musical Key -> Count
    pub key_distribution: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenreCount {
    pub genre: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistCount {
    pub artist: String,
    pub count: u32,
}
