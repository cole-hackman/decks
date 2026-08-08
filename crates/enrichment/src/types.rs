//! What a provider can tell us about a track, and how sure it is.

use serde::{Deserialize, Serialize};

/// One provider's answer for one track.
///
/// Every field is optional because every provider is partial — MusicBrainz
/// knows labels and years well and genres unevenly; Cover Art Archive knows
/// only images. Merging is [`crate::merge`]'s job, not a provider's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// Which provider said this. Shown in the UI, per ADR-0008 — a user
    /// accepting a genre is entitled to know who claimed it.
    pub source: String,
    /// The provider's own id for this recording, so a later lookup (cover art,
    /// a second fetch) does not have to search again.
    pub id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// The **main** genre only. Per `docs/lexicon/07-health.md §Find Tags`,
    /// Lexicon fills the single-value genre field with the main genre and
    /// routes everything else to custom tags — which keeps the field usable
    /// for sorting instead of turning it into a comma-salad.
    pub genre: Option<String>,
    /// Everything that was not the main genre. Destined for custom tags.
    pub subgenres: Vec<String>,
    pub year: Option<i32>,
    pub label: Option<String>,
    /// The release this recording appeared on, for a cover-art lookup.
    pub release_id: Option<String>,
    /// 0.0–1.0. What the provider's own scoring said, normalised.
    pub score: f32,
}

/// A single proposed field change, ready to become a staged change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldProposal {
    pub field: String,
    pub before: Option<String>,
    pub after: String,
    /// Which provider this value came from. Never elided — a proposal with no
    /// attribution is a guess presented as a fact.
    pub source: String,
    pub confidence: f32,
}

/// What enriching one track would do.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackProposal {
    pub track_id: String,
    pub proposals: Vec<FieldProposal>,
    /// Subgenres to attach as custom tags, kept apart from `proposals` because
    /// they are written through a different mechanism entirely.
    pub tags: Vec<String>,
    /// Set when every provider came back empty, so the UI can distinguish
    /// "nothing to change" from "nothing found" — they look identical
    /// otherwise, and one of them means the user should fix their tags first.
    pub no_match: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EnrichError {
    #[error("http: {0}")]
    Http(#[from] crate::http::HttpError),
    #[error("provider {provider} returned {status}")]
    Status { provider: String, status: u16 },
    #[error("could not parse {provider} response: {detail}")]
    Parse { provider: String, detail: String },
    #[error("{0} needs a token; add one in Settings")]
    NeedsToken(String),
}

pub type Result<T> = std::result::Result<T, EnrichError>;

/// What we ask a provider for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Query {
    pub artist: Option<String>,
    pub title: String,
    /// Per the spec's `Original release` option: strip remix/remaster text from
    /// the title and look for the earliest release instead. "Best practice for
    /// older tracks", and the reason re-releases otherwise resolve to a 2019
    /// remaster of a 1977 record.
    pub original_release: bool,
}

impl Query {
    pub fn new(artist: Option<&str>, title: &str) -> Self {
        Self {
            artist: artist.map(str::to_string),
            title: title.to_string(),
            original_release: false,
        }
    }

    pub fn original_release(mut self) -> Self {
        self.original_release = true;
        self
    }

    /// A stable key for the response cache.
    ///
    /// Case- and whitespace-insensitive, because "Daft Punk " and "daft punk"
    /// are the same lookup and paying for both would waste a rate-limited
    /// request on a difference no provider cares about.
    pub fn cache_key(&self, provider: &str) -> String {
        let norm = |s: &str| s.trim().to_lowercase();
        format!(
            "{provider}\u{1}{}\u{1}{}\u{1}{}",
            self.artist.as_deref().map(norm).unwrap_or_default(),
            norm(&self.title),
            self.original_release
        )
    }
}
