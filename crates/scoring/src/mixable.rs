//! Mixable Tracks — "pick a track, get a ranked list of tracks that mix with it".
//!
//! Per `docs/lexicon/04-analysis.md §Mixable Tracks`. Two tiers, as in the spec:
//! basic mode considers only BPM and key, advanced mode exposes the full rule
//! set. Both go through the same function; basic mode is simply
//! [`MixableOptions::basic`].
//!
//! **The rules filter; the score orders.** A candidate that fails any enabled
//! rule is not in the list at all, and what remains is ranked by
//! [`score_transition`](crate::score_transition). Mixing filtering into the
//! score would produce a list where a rule the user asked for is merely a
//! headwind, which is not what "must have cue points" means.
//!
//! `Match color` and `Recently added` now work: `Track` carries colour and
//! date-added since the field-widening change. What remains unimplemented is
//! `Popularity` / `Danceability` / `Happiness` — and those are not waiting on a
//! column. ADR-0012 records that Lexicon sources them from Spotify's
//! `audio-features` endpoint, which was deprecated in November 2024 and returns
//! 403 for applications registered since. Popularity is a catalog metric that
//! cannot be computed locally at all. They are absent rather than stubbed — a
//! rule that silently matches everything is worse than a rule that isn't
//! offered.

use std::collections::{HashMap, HashSet};

use rekordbox_db::Track;
use serde::{Deserialize, Serialize};

use crate::{score_transition, CamelotKey};

/// The global Key Mixing Mode, shared with the browser's compatible-key
/// indicator.
///
/// Spec: `Harmonically Compatible` is traditional harmonic mixing; `Fuzzy`
/// expands to the adjacent numbers in *both* modes, so `10m` reaches
/// `9m/9d/11m/11d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyMixingMode {
    #[default]
    HarmonicallyCompatible,
    Fuzzy,
}

/// How two keys relate on the wheel. Ordered loosely by how safe the move is,
/// but the ordering is not load-bearing — `score_transition` does the ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyRelation {
    /// Identical key.
    Same,
    /// Same number, opposite mode — the relative major/minor.
    RelativeMajorMinor,
    /// One step around the wheel, same mode.
    AdjacentSameMode,
    /// One step around the wheel, opposite mode. Only reachable in Fuzzy mode.
    AdjacentOppositeMode,
}

/// Classify two keys, or `None` when either is unparseable or they are not
/// related at all.
///
/// Accepts anything [`CamelotKey::parse`] accepts, which includes Open Key
/// (`10m`) and spelled-out keys (`C minor`) as well as Camelot.
pub fn key_relation(a: &str, b: &str) -> Option<KeyRelation> {
    let (a, b) = (CamelotKey::parse(a)?, CamelotKey::parse(b)?);
    let diff = (a.number as i16 - b.number as i16).abs();
    let distance = std::cmp::min(diff, 12 - diff);
    let same_mode = a.is_minor == b.is_minor;
    match (distance, same_mode) {
        (0, true) => Some(KeyRelation::Same),
        (0, false) => Some(KeyRelation::RelativeMajorMinor),
        (1, true) => Some(KeyRelation::AdjacentSameMode),
        (1, false) => Some(KeyRelation::AdjacentOppositeMode),
        _ => None,
    }
}

/// Whether `b` is a legal key move from `a` under `mode`.
///
/// Unparseable keys are **not** compatible. The alternative — treating an
/// unknown key as a wildcard — floods the list with the exact tracks the user
/// has not analysed yet.
pub fn keys_compatible(a: &str, b: &str, mode: KeyMixingMode) -> bool {
    match key_relation(a, b) {
        Some(KeyRelation::AdjacentOppositeMode) => mode == KeyMixingMode::Fuzzy,
        Some(_) => true,
        None => false,
    }
}

/// Every key that mixes out of `key` under `mode`, in Camelot notation.
///
/// Used by the compatible-key indicator, which needs the set rather than a
/// pairwise test.
pub fn compatible_keys(key: &str, mode: KeyMixingMode) -> Vec<String> {
    let Some(k) = CamelotKey::parse(key) else {
        return Vec::new();
    };
    let wrap = |n: i16| -> u8 { (((n - 1).rem_euclid(12)) + 1) as u8 };
    let n = k.number as i16;
    let letter = |minor: bool| if minor { 'A' } else { 'B' };

    let mut out = vec![
        format!("{}{}", k.number, letter(k.is_minor)),
        format!("{}{}", k.number, letter(!k.is_minor)),
        format!("{}{}", wrap(n - 1), letter(k.is_minor)),
        format!("{}{}", wrap(n + 1), letter(k.is_minor)),
    ];
    if mode == KeyMixingMode::Fuzzy {
        out.push(format!("{}{}", wrap(n - 1), letter(!k.is_minor)));
        out.push(format!("{}{}", wrap(n + 1), letter(!k.is_minor)));
    }
    out
}

/// How a candidate's tempo relates to the source's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BpmRelation {
    Direct,
    Half,
    Double,
}

/// A numeric rule over a 1-to-N field (Energy, Rating).
///
/// Spec: "Match input ±1, or a supplied range."
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NumericRule {
    /// Rule not applied.
    #[default]
    Off,
    /// Within one of the source track's value. A source with no value disables
    /// the rule rather than excluding everything — there is nothing to be
    /// within one of.
    NearSource,
    /// An explicit inclusive range.
    Range { min: f64, max: f64 },
}

impl NumericRule {
    /// `f64` rather than an integer because Energy is a float on `Track` while
    /// Rating is not, and one rule type serves both.
    fn admits(&self, source: Option<f64>, candidate: Option<f64>) -> bool {
        match self {
            Self::Off => true,
            Self::NearSource => match (source, candidate) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some(s), Some(c)) => (s - c).abs() <= 1.0,
            },
            Self::Range { min, max } => candidate.is_some_and(|c| c >= *min && c <= *max),
        }
    }
}

/// A rule over the release year.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum YearRule {
    #[default]
    Off,
    /// Same year as the source track.
    SameAsSource,
    Range {
        min: i64,
        max: i64,
    },
}

impl YearRule {
    fn admits(&self, source: Option<i64>, candidate: Option<i64>) -> bool {
        match self {
            Self::Off => true,
            Self::SameAsSource => match (source, candidate) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some(s), Some(c)) => s == c,
            },
            Self::Range { min, max } => candidate.is_some_and(|c| c >= *min && c <= *max),
        }
    }
}

/// The advanced rule set. [`MixableOptions::basic`] is the spec's basic mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MixableOptions {
    /// Allowed BPM difference as a percentage of the source tempo. `None`
    /// accepts any tempo.
    pub bpm_tolerance_pct: Option<f64>,
    /// Restrict to keys compatible under `key_mixing_mode`.
    pub match_key: bool,
    pub key_mixing_mode: KeyMixingMode,
    /// Also accept half-time and double-time candidates.
    pub include_half_double: bool,
    /// Skip un-cued tracks.
    pub must_have_cues: bool,
    /// Restrict to this genre set. Empty means unrestricted. Matched
    /// case-insensitively, since genre strings are hand-typed.
    pub genres: Vec<String>,
    pub year: YearRule,
    pub energy: NumericRule,
    pub rating: NumericRule,
    /// Tag ids the candidate must all carry.
    pub must_have_tags: Vec<String>,
    /// Tag ids that disqualify a candidate.
    pub must_not_have_tags: Vec<String>,
    /// Restrict to candidates carrying the source track's colour.
    ///
    /// A source with no colour admits nothing rather than everything: the rule
    /// says "the same colour as this", and "the same as nothing" is not a set
    /// worth returning.
    pub match_color: bool,
    /// Only candidates added on or after this ISO-8601 date.
    ///
    /// A string rather than a duration, because `djmdContent.DateCreated` is
    /// stored as text of varying precision and the caller — which knows what
    /// "recently" means to the user — can compute the cutoff once.
    pub added_since: Option<String>,
    pub limit: usize,
}

impl Default for MixableOptions {
    fn default() -> Self {
        Self::basic()
    }
}

impl MixableOptions {
    /// The spec's basic mode: BPM and key only.
    pub fn basic() -> Self {
        Self {
            bpm_tolerance_pct: Some(6.0),
            match_key: true,
            key_mixing_mode: KeyMixingMode::default(),
            include_half_double: false,
            must_have_cues: false,
            genres: Vec::new(),
            year: YearRule::Off,
            energy: NumericRule::Off,
            rating: NumericRule::Off,
            must_have_tags: Vec::new(),
            must_not_have_tags: Vec::new(),
            match_color: false,
            added_since: None,
            limit: 25,
        }
    }
}

/// Facts that do not live on `Track`. Mirrors `smartlists::EvalContext` so the
/// two engines assemble their inputs the same way.
#[derive(Debug, Clone, Default)]
pub struct MixableContext {
    pub tracks_with_cues: HashSet<String>,
    /// Tag **ids** per track, as stored — the UI resolves names.
    pub tags_by_track: HashMap<String, HashSet<String>>,
    /// Never suggested. Archiving a track is a statement that it is out of
    /// rotation, and a "what do I play next" list is exactly where that should
    /// be honoured.
    pub archived_tracks: HashSet<String>,
}

/// One row of the result list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixableMatch {
    pub track: Track,
    pub score: f32,
    pub reasons: Vec<String>,
    pub bpm_relation: BpmRelation,
    /// `None` when either key is unparseable and `match_key` was off.
    pub key_relation: Option<KeyRelation>,
}

/// Which BPM relation, if any, brings `candidate` within tolerance of `source`.
///
/// Returns `Some(Direct)` when no tolerance is set, so a tempo-agnostic search
/// still reports a relation.
fn bpm_relation(
    source: Option<f64>,
    candidate: Option<f64>,
    tolerance_pct: Option<f64>,
    include_half_double: bool,
) -> Option<BpmRelation> {
    let Some(tolerance) = tolerance_pct else {
        return Some(BpmRelation::Direct);
    };
    let (Some(a), Some(b)) = (source, candidate) else {
        // A tempo rule cannot be satisfied by a track with no tempo.
        return None;
    };
    if a <= 0.0 || b <= 0.0 {
        return None;
    }
    let within = |target: f64| ((target - b).abs() / target * 100.0) <= tolerance;
    if within(a) {
        return Some(BpmRelation::Direct);
    }
    if include_half_double {
        // Compared against the *stretched source*, so the tolerance stays a
        // percentage of the tempo actually being matched.
        if within(a * 2.0) {
            return Some(BpmRelation::Double);
        }
        if within(a / 2.0) {
            return Some(BpmRelation::Half);
        }
    }
    None
}

fn has_all(tags: Option<&HashSet<String>>, required: &[String]) -> bool {
    required
        .iter()
        .all(|t| tags.is_some_and(|have| have.contains(t)))
}

fn has_none(tags: Option<&HashSet<String>>, excluded: &[String]) -> bool {
    !excluded
        .iter()
        .any(|t| tags.is_some_and(|have| have.contains(t)))
}

/// Rank `candidates` by how well they mix out of `source`.
///
/// `source` is never in its own result list. Results are sorted by score
/// descending, then by track id, so a tie resolves the same way on every run —
/// a list that reshuffles between two identical searches is not usable live.
pub fn find_mixable(
    source: &Track,
    candidates: &[Track],
    opts: &MixableOptions,
    ctx: &MixableContext,
) -> Vec<MixableMatch> {
    let source_energy = source.energy.map(f64::from);
    let source_rating = source.rating.map(|r| r as f64);
    let source_year = source.release_year;
    let genres: Vec<String> = opts.genres.iter().map(|g| g.to_lowercase()).collect();

    let mut out: Vec<MixableMatch> = candidates
        .iter()
        .filter(|t| t.id != source.id)
        .filter(|t| !ctx.archived_tracks.contains(&t.id))
        .filter_map(|t| {
            let relation = bpm_relation(
                source.bpm,
                t.bpm,
                opts.bpm_tolerance_pct,
                opts.include_half_double,
            )?;

            let key_relation = match (&source.musical_key, &t.musical_key) {
                (Some(a), Some(b)) => key_relation(a, b),
                _ => None,
            };
            if opts.match_key {
                let (Some(a), Some(b)) = (&source.musical_key, &t.musical_key) else {
                    return None;
                };
                if !keys_compatible(a, b, opts.key_mixing_mode) {
                    return None;
                }
            }

            if opts.must_have_cues && !ctx.tracks_with_cues.contains(&t.id) {
                return None;
            }
            if !genres.is_empty() {
                let g = t.genre.as_deref().unwrap_or("").to_lowercase();
                if !genres.contains(&g) {
                    return None;
                }
            }
            if !opts.year.admits(source_year, t.release_year) {
                return None;
            }
            if !opts.energy.admits(source_energy, t.energy.map(f64::from)) {
                return None;
            }
            if !opts
                .rating
                .admits(source_rating, t.rating.map(|r| r as f64))
            {
                return None;
            }

            if opts.match_color {
                let (Some(a), Some(b)) = (source.color.as_deref(), t.color.as_deref()) else {
                    return None;
                };
                if !a.eq_ignore_ascii_case(b) {
                    return None;
                }
            }
            if let Some(since) = opts.added_since.as_deref() {
                // Lexicographic, matching the smartlist date rules — ISO-8601
                // sorts correctly as text. A track with no date is excluded:
                // we do not know when it arrived, so we cannot claim it is new.
                let added = t.date_added.as_deref()?;
                if added.trim() < since.trim() {
                    return None;
                }
            }

            let tags = ctx.tags_by_track.get(&t.id);
            if !has_all(tags, &opts.must_have_tags) || !has_none(tags, &opts.must_not_have_tags) {
                return None;
            }

            let mut scored = score_transition(source, t);
            if relation != BpmRelation::Direct {
                scored.reasons.push(match relation {
                    BpmRelation::Half => "Half-time match".to_string(),
                    BpmRelation::Double => "Double-time match".to_string(),
                    BpmRelation::Direct => unreachable!(),
                });
            }

            Some(MixableMatch {
                track: t.clone(),
                score: scored.score,
                reasons: scored.reasons,
                bpm_relation: relation,
                key_relation,
            })
        })
        .collect();

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.track.id.cmp(&b.track.id))
    });
    out.truncate(opts.limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, key: Option<&str>, bpm: Option<f64>) -> Track {
        Track {
            id: id.into(),
            title: format!("Track {id}"),
            artist: None,
            album: None,
            genre: None,
            musical_key: key.map(str::to_string),
            bpm,
            duration_secs: None,
            rating: None,
            comment: None,
            folder_path: None,
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: None,
            dj_play_count: None,
            label: None,
            remixer: None,
            mix: None,
            color: None,
            date_added: None,
            energy: None,
        }
    }

    fn ids(matches: &[MixableMatch]) -> Vec<&str> {
        matches.iter().map(|m| m.track.id.as_str()).collect()
    }

    // ── Key relations ────────────────────────────────────────────────────

    #[test]
    fn classifies_the_four_relations() {
        assert_eq!(key_relation("8A", "8A"), Some(KeyRelation::Same));
        assert_eq!(
            key_relation("8A", "8B"),
            Some(KeyRelation::RelativeMajorMinor)
        );
        assert_eq!(
            key_relation("8A", "9A"),
            Some(KeyRelation::AdjacentSameMode)
        );
        assert_eq!(
            key_relation("8A", "9B"),
            Some(KeyRelation::AdjacentOppositeMode)
        );
        assert_eq!(key_relation("8A", "11A"), None);
    }

    #[test]
    fn key_wheel_wraps_at_twelve() {
        assert_eq!(
            key_relation("12A", "1A"),
            Some(KeyRelation::AdjacentSameMode)
        );
        assert_eq!(
            key_relation("1A", "12A"),
            Some(KeyRelation::AdjacentSameMode)
        );
    }

    #[test]
    fn open_key_and_spelled_keys_compare_against_camelot() {
        // 10m == 10A; the whole point of routing through key_format.
        assert_eq!(key_relation("10m", "10A"), Some(KeyRelation::Same));
        assert_eq!(key_relation("C minor", "5A"), Some(KeyRelation::Same));
    }

    #[test]
    fn fuzzy_mode_admits_the_diagonal_but_harmonic_does_not() {
        assert!(!keys_compatible(
            "10A",
            "9B",
            KeyMixingMode::HarmonicallyCompatible
        ));
        assert!(keys_compatible("10A", "9B", KeyMixingMode::Fuzzy));
        // Both modes still admit the traditional moves.
        for mode in [KeyMixingMode::HarmonicallyCompatible, KeyMixingMode::Fuzzy] {
            assert!(keys_compatible("10A", "10A", mode));
            assert!(keys_compatible("10A", "10B", mode));
            assert!(keys_compatible("10A", "11A", mode));
        }
    }

    #[test]
    fn unparseable_keys_are_never_compatible() {
        assert!(!keys_compatible("", "8A", KeyMixingMode::Fuzzy));
        assert!(!keys_compatible("Banana", "8A", KeyMixingMode::Fuzzy));
        assert!(!keys_compatible("8A", "99Z", KeyMixingMode::Fuzzy));
    }

    #[test]
    fn compatible_key_sets_match_the_spec_example() {
        let mut harmonic = compatible_keys("10A", KeyMixingMode::HarmonicallyCompatible);
        harmonic.sort();
        assert_eq!(harmonic, vec!["10A", "10B", "11A", "9A"]);

        let mut fuzzy = compatible_keys("10A", KeyMixingMode::Fuzzy);
        fuzzy.sort();
        // The spec: 10m → 9m/9d/11m/11d, plus the key itself and its relative.
        assert_eq!(fuzzy, vec!["10A", "10B", "11A", "11B", "9A", "9B"]);
    }

    #[test]
    fn compatible_keys_of_an_unparseable_key_is_empty() {
        assert!(compatible_keys("nonsense", KeyMixingMode::Fuzzy).is_empty());
    }

    // ── BPM ──────────────────────────────────────────────────────────────

    #[test]
    fn half_and_double_are_off_unless_asked_for() {
        let source = track("s", Some("8A"), Some(140.0));
        let candidates = vec![track("half", Some("8A"), Some(70.0))];
        let mut opts = MixableOptions::basic();
        opts.bpm_tolerance_pct = Some(3.0);

        assert!(find_mixable(&source, &candidates, &opts, &MixableContext::default()).is_empty());

        opts.include_half_double = true;
        let got = find_mixable(&source, &candidates, &opts, &MixableContext::default());
        assert_eq!(ids(&got), vec!["half"]);
        assert_eq!(got[0].bpm_relation, BpmRelation::Half);
        assert!(got[0].reasons.iter().any(|r| r == "Half-time match"));
    }

    #[test]
    fn double_time_tolerance_is_a_percentage_of_the_stretched_tempo() {
        // 3% of 280 is 8.4 BPM, so 286 is in and 290 is out. If the tolerance
        // were taken against the source (3% of 140 = 4.2) both would be out.
        let source = track("s", Some("8A"), Some(140.0));
        let candidates = vec![
            track("in", Some("8A"), Some(286.0)),
            track("out", Some("8A"), Some(290.0)),
        ];
        let opts = MixableOptions {
            bpm_tolerance_pct: Some(3.0),
            include_half_double: true,
            ..MixableOptions::basic()
        };
        let got = find_mixable(&source, &candidates, &opts, &MixableContext::default());
        assert_eq!(ids(&got), vec!["in"]);
        assert_eq!(got[0].bpm_relation, BpmRelation::Double);
    }

    #[test]
    fn a_tempo_rule_excludes_tracks_with_no_tempo() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates = vec![track("no-bpm", Some("8A"), None)];
        let opts = MixableOptions::basic();
        assert!(find_mixable(&source, &candidates, &opts, &MixableContext::default()).is_empty());
    }

    #[test]
    fn no_tolerance_accepts_any_tempo() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates = vec![track("slow", Some("8A"), Some(60.0))];
        let opts = MixableOptions {
            bpm_tolerance_pct: None,
            ..MixableOptions::basic()
        };
        let got = find_mixable(&source, &candidates, &opts, &MixableContext::default());
        assert_eq!(ids(&got), vec!["slow"]);
    }

    // ── Filtering ────────────────────────────────────────────────────────

    #[test]
    fn the_source_is_never_in_its_own_list() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates = vec![source.clone()];
        let got = find_mixable(
            &source,
            &candidates,
            &MixableOptions::basic(),
            &MixableContext::default(),
        );
        assert!(got.is_empty());
    }

    #[test]
    fn archived_tracks_are_never_suggested() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates = vec![track("a", Some("8A"), Some(128.0))];
        let ctx = MixableContext {
            archived_tracks: ["a".to_string()].into_iter().collect(),
            ..Default::default()
        };
        assert!(find_mixable(&source, &candidates, &MixableOptions::basic(), &ctx).is_empty());
    }

    #[test]
    fn must_have_cues_skips_uncued_tracks() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates = vec![
            track("cued", Some("8A"), Some(128.0)),
            track("bare", Some("8A"), Some(128.0)),
        ];
        let ctx = MixableContext {
            tracks_with_cues: ["cued".to_string()].into_iter().collect(),
            ..Default::default()
        };
        let opts = MixableOptions {
            must_have_cues: true,
            ..MixableOptions::basic()
        };
        assert_eq!(
            ids(&find_mixable(&source, &candidates, &opts, &ctx)),
            ["cued"]
        );
    }

    #[test]
    fn genre_matching_ignores_case_and_an_empty_set_is_unrestricted() {
        let source = track("s", Some("8A"), Some(128.0));
        let mut house = track("house", Some("8A"), Some(128.0));
        house.genre = Some("HOUSE".into());
        let mut techno = track("techno", Some("8A"), Some(128.0));
        techno.genre = Some("Techno".into());
        let candidates = vec![house, techno];

        let opts = MixableOptions {
            genres: vec!["house".into()],
            ..MixableOptions::basic()
        };
        assert_eq!(
            ids(&find_mixable(
                &source,
                &candidates,
                &opts,
                &MixableContext::default()
            )),
            ["house"]
        );

        let all = find_mixable(
            &source,
            &candidates,
            &MixableOptions::basic(),
            &MixableContext::default(),
        );
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn tag_rules_require_all_and_exclude_any() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates = vec![
            track("both", Some("8A"), Some(128.0)),
            track("one", Some("8A"), Some(128.0)),
            track("banned", Some("8A"), Some(128.0)),
            track("untagged", Some("8A"), Some(128.0)),
        ];
        let ctx = MixableContext {
            tags_by_track: [
                (
                    "both".to_string(),
                    ["t1".to_string(), "t2".to_string()].into_iter().collect(),
                ),
                ("one".to_string(), ["t1".to_string()].into_iter().collect()),
                (
                    "banned".to_string(),
                    ["t1".to_string(), "t2".to_string(), "no".to_string()]
                        .into_iter()
                        .collect(),
                ),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let opts = MixableOptions {
            must_have_tags: vec!["t1".into(), "t2".into()],
            must_not_have_tags: vec!["no".into()],
            ..MixableOptions::basic()
        };
        assert_eq!(
            ids(&find_mixable(&source, &candidates, &opts, &ctx)),
            ["both"]
        );
    }

    #[test]
    fn near_source_energy_admits_plus_or_minus_one() {
        let source = {
            let mut t = track("s", Some("8A"), Some(128.0));
            t.energy = Some(6.0);
            t
        };
        let candidates: Vec<Track> = [(5.0, "five"), (7.0, "seven"), (9.0, "nine")]
            .into_iter()
            .map(|(e, id)| {
                let mut t = track(id, Some("8A"), Some(128.0));
                t.energy = Some(e);
                t
            })
            .collect();
        let opts = MixableOptions {
            energy: NumericRule::NearSource,
            ..MixableOptions::basic()
        };
        let got = find_mixable(&source, &candidates, &opts, &MixableContext::default());
        let mut got_ids = ids(&got);
        got_ids.sort();
        assert_eq!(got_ids, ["five", "seven"]);
    }

    #[test]
    fn near_source_is_disabled_when_the_source_has_no_value() {
        // Nothing to be within one of — better than excluding the library.
        let source = track("s", Some("8A"), Some(128.0));
        let mut candidate = track("c", Some("8A"), Some(128.0));
        candidate.energy = Some(9.0);
        let opts = MixableOptions {
            energy: NumericRule::NearSource,
            ..MixableOptions::basic()
        };
        let got = find_mixable(&source, &[candidate], &opts, &MixableContext::default());
        assert_eq!(ids(&got), ["c"]);
    }

    #[test]
    fn an_explicit_range_excludes_tracks_with_no_value() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates = vec![track("blank", Some("8A"), Some(128.0))];
        let opts = MixableOptions {
            rating: NumericRule::Range { min: 3.0, max: 5.0 },
            ..MixableOptions::basic()
        };
        assert!(find_mixable(&source, &candidates, &opts, &MixableContext::default()).is_empty());
    }

    #[test]
    fn year_rules_match_the_source_or_a_range() {
        let source = {
            let mut t = track("s", Some("8A"), Some(128.0));
            t.release_year = Some(2001);
            t
        };
        let candidates: Vec<Track> = [(2001, "same"), (2010, "later")]
            .into_iter()
            .map(|(y, id)| {
                let mut t = track(id, Some("8A"), Some(128.0));
                t.release_year = Some(y);
                t
            })
            .collect();

        let same = MixableOptions {
            year: YearRule::SameAsSource,
            ..MixableOptions::basic()
        };
        assert_eq!(
            ids(&find_mixable(
                &source,
                &candidates,
                &same,
                &MixableContext::default()
            )),
            ["same"]
        );

        let range = MixableOptions {
            year: YearRule::Range {
                min: 2005,
                max: 2020,
            },
            ..MixableOptions::basic()
        };
        assert_eq!(
            ids(&find_mixable(
                &source,
                &candidates,
                &range,
                &MixableContext::default()
            )),
            ["later"]
        );
    }

    #[test]
    fn match_key_excludes_tracks_with_no_key() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates = vec![track("keyless", None, Some(128.0))];
        assert!(find_mixable(
            &source,
            &candidates,
            &MixableOptions::basic(),
            &MixableContext::default()
        )
        .is_empty());

        let opts = MixableOptions {
            match_key: false,
            ..MixableOptions::basic()
        };
        let got = find_mixable(&source, &candidates, &opts, &MixableContext::default());
        assert_eq!(ids(&got), ["keyless"]);
        assert_eq!(got[0].key_relation, None);
    }

    // ── Ordering ─────────────────────────────────────────────────────────

    #[test]
    fn results_rank_by_score_and_ties_break_deterministically() {
        let source = track("s", Some("8A"), Some(128.0));
        // Two identical tracks and one weaker one; the identical pair must come
        // back in id order every run.
        let candidates = vec![
            track("zzz", Some("8A"), Some(128.0)),
            track("aaa", Some("8A"), Some(128.0)),
            track("weak", Some("9A"), Some(128.0)),
        ];
        let got = find_mixable(
            &source,
            &candidates,
            &MixableOptions::basic(),
            &MixableContext::default(),
        );
        assert_eq!(ids(&got), ["aaa", "zzz", "weak"]);
    }

    #[test]
    fn the_limit_is_honoured() {
        let source = track("s", Some("8A"), Some(128.0));
        let candidates: Vec<Track> = (0..50)
            .map(|i| track(&format!("t{i:02}"), Some("8A"), Some(128.0)))
            .collect();
        let opts = MixableOptions {
            limit: 5,
            ..MixableOptions::basic()
        };
        assert_eq!(
            find_mixable(&source, &candidates, &opts, &MixableContext::default()).len(),
            5
        );
    }

    #[test]
    fn basic_mode_applies_no_advanced_rule() {
        let opts = MixableOptions::basic();
        assert_eq!(opts.year, YearRule::Off);
        assert_eq!(opts.energy, NumericRule::Off);
        assert_eq!(opts.rating, NumericRule::Off);
        assert!(!opts.must_have_cues);
        assert!(opts.genres.is_empty());
        assert!(opts.must_have_tags.is_empty());
        assert!(opts.must_not_have_tags.is_empty());
    }

    #[test]
    fn options_round_trip_through_json() {
        // Templates are stored as JSON, so this is the persistence contract.
        let opts = MixableOptions {
            key_mixing_mode: KeyMixingMode::Fuzzy,
            energy: NumericRule::Range { min: 4.0, max: 8.0 },
            year: YearRule::SameAsSource,
            genres: vec!["House".into()],
            ..MixableOptions::basic()
        };
        let json = serde_json::to_string(&opts).unwrap();
        assert_eq!(serde_json::from_str::<MixableOptions>(&json).unwrap(), opts);
    }

    #[test]
    fn a_partial_template_fills_in_from_basic_mode() {
        // Older stored templates must keep loading as new options are added.
        let opts: MixableOptions = serde_json::from_str(r#"{"match_key": false}"#).unwrap();
        assert!(!opts.match_key);
        assert_eq!(opts.limit, MixableOptions::basic().limit);
    }

    fn coloured(id: &str, color: Option<&str>, added: Option<&str>) -> Track {
        Track {
            color: color.map(str::to_string),
            date_added: added.map(str::to_string),
            ..track(id, Some("8A"), Some(128.0))
        }
    }

    #[test]
    fn match_color_keeps_only_the_sources_colour() {
        let source = coloured("src", Some("Red"), None);
        let candidates = vec![
            coloured("same", Some("Red"), None),
            coloured("other", Some("Blue"), None),
            coloured("none", None, None),
        ];
        let opts = MixableOptions {
            match_color: true,
            ..MixableOptions::basic()
        };
        let got = find_mixable(&source, &candidates, &opts, &MixableContext::default());
        assert_eq!(ids(&got), vec!["same"]);
    }

    #[test]
    fn match_color_is_case_insensitive() {
        // Rekordbox's own casing is not something a user should have to know.
        let source = coloured("src", Some("Red"), None);
        let candidates = vec![coloured("same", Some("RED"), None)];
        let opts = MixableOptions {
            match_color: true,
            ..MixableOptions::basic()
        };
        let got = find_mixable(&source, &candidates, &opts, &MixableContext::default());
        assert_eq!(ids(&got), vec!["same"]);
    }

    #[test]
    fn an_uncoloured_source_admits_nothing_rather_than_everything() {
        // "the same colour as this" where this has no colour is not a set worth
        // returning — and returning all of them would look like the rule is off.
        let source = coloured("src", None, None);
        let candidates = vec![coloured("a", Some("Red"), None), coloured("b", None, None)];
        let opts = MixableOptions {
            match_color: true,
            ..MixableOptions::basic()
        };
        assert!(find_mixable(&source, &candidates, &opts, &MixableContext::default()).is_empty());
    }

    #[test]
    fn added_since_is_inclusive_of_the_cutoff_day() {
        let source = coloured("src", None, Some("2025-01-01"));
        let candidates = vec![
            coloured("on", None, Some("2025-06-01T00:00:00Z")),
            coloured("after", None, Some("2025-07-01T00:00:00Z")),
            coloured("before", None, Some("2025-05-01T00:00:00Z")),
        ];
        let opts = MixableOptions {
            added_since: Some("2025-06-01".into()),
            ..MixableOptions::basic()
        };
        let found = find_mixable(&source, &candidates, &opts, &MixableContext::default());
        let mut got = ids(&found);
        got.sort();
        assert_eq!(got, vec!["after", "on"]);
    }

    #[test]
    fn a_track_with_no_date_is_not_treated_as_recent() {
        // We do not know when it arrived, so claiming it is new would be a
        // guess presented as a fact.
        let source = coloured("src", None, Some("2025-01-01"));
        let candidates = vec![coloured("undated", None, None)];
        let opts = MixableOptions {
            added_since: Some("2020-01-01".into()),
            ..MixableOptions::basic()
        };
        assert!(find_mixable(&source, &candidates, &opts, &MixableContext::default()).is_empty());
    }

    #[test]
    fn the_new_rules_are_off_in_basic_mode() {
        let basic = MixableOptions::basic();
        assert!(!basic.match_color);
        assert_eq!(basic.added_since, None);
    }
}
