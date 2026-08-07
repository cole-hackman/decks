//! Subfolder patterns — the nested directory levels a moved file lands in.
//!
//! Up to **three** levels, each independently optional. `Genre` then `Bpm`
//! yields `…/Music/House/128/track.mp3`.
//!
//! The rule that matters most: **an empty field drops its level, it does not
//! drop the move.** A track with no genre still lands in the target folder, just
//! one level shallower. Anything else orphans files, which is the one outcome a
//! bulk file mover must never produce.
//!
//! Beyond raw fields there are computed patterns — bitrate buckets, the first
//! tag, and date buckets. See `docs/lexicon/06-files.md §Special subfolder
//! patterns`.

use serde::{Deserialize, Serialize};

use crate::pattern::sanitize_component;

/// Lexicon allows three nested subfolder levels. Deeper nesting is rejected
/// rather than silently truncated.
pub const MAX_LEVELS: usize = 3;

/// What a single subfolder level is driven by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubfolderPattern {
    /// The raw value of a track field, e.g. `genre` → `House`.
    Field { name: String },
    /// Two buckets, `320+` or `320-` — not the raw bitrate, which would
    /// produce a folder per encoder setting.
    BitrateBucket,
    /// The first tag of the first tag category, in the category order shown on
    /// the Tags page. Ordering is the caller's job; this takes the list as
    /// already ordered.
    FirstTag,
    /// The year this ran, e.g. `2026`.
    CurrentYear,
    /// The month this ran, zero-padded: `01`–`12`.
    CurrentMonth,
    /// The decade this ran, as a range: `2020 - 2029`.
    CurrentDecade,
    /// The decade of the track's release year, as a range: `1990 - 1999`.
    ///
    /// Not in the manual's table, which lists only date-of-run buckets. Added
    /// because a decade folder computed from *today* is the same string for
    /// every track in a run, and filing a library by release decade is the
    /// obviously intended use. Recorded in `docs/lexicon/GAPS.md`.
    ReleaseDecade,
}

/// The date the organiser is running, injected rather than read from the clock
/// so the date-bucket patterns are testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunDate {
    pub year: i32,
    /// 1–12.
    pub month: u32,
}

/// Everything a subfolder pattern can be resolved against.
#[derive(Debug, Clone)]
pub struct TrackFacts<'a> {
    /// Rendered field values, the same map `Pattern::render` takes.
    pub fields: &'a std::collections::HashMap<String, String>,
    /// Bitrate in kbps, if known.
    pub bitrate_kbps: Option<u32>,
    /// The track's tags, already ordered by category order then tag order.
    pub tags: &'a [String],
    /// Release year, if known.
    pub year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubfolderError {
    TooManyLevels(usize),
}

impl std::fmt::Display for SubfolderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubfolderError::TooManyLevels(n) => {
                write!(
                    f,
                    "{n} subfolder levels requested, at most {MAX_LEVELS} allowed"
                )
            }
        }
    }
}

impl std::error::Error for SubfolderError {}

fn decade_range(year: i32) -> String {
    let start = year - year.rem_euclid(10);
    format!("{start} - {}", start + 9)
}

impl SubfolderPattern {
    /// Resolve this level to a folder name, or `None` when it has no value and
    /// the level should be skipped.
    pub fn resolve(&self, facts: &TrackFacts<'_>, now: RunDate) -> Option<String> {
        let raw = match self {
            SubfolderPattern::Field { name } => facts.fields.get(name).cloned()?,
            // A missing bitrate is genuinely unknown, so it drops the level
            // rather than defaulting into the `320-` bucket and mislabelling a
            // lossless file.
            SubfolderPattern::BitrateBucket => {
                let kbps = facts.bitrate_kbps?;
                if kbps >= 320 { "320+" } else { "320-" }.to_string()
            }
            SubfolderPattern::FirstTag => facts.tags.first().cloned()?,
            SubfolderPattern::CurrentYear => now.year.to_string(),
            SubfolderPattern::CurrentMonth => format!("{:02}", now.month),
            SubfolderPattern::CurrentDecade => decade_range(now.year),
            SubfolderPattern::ReleaseDecade => decade_range(facts.year?),
        };

        let cleaned = sanitize_component(&raw);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }
}

/// An ordered list of subfolder levels.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SubfolderSpec {
    pub levels: Vec<SubfolderPattern>,
}

impl SubfolderSpec {
    pub fn new(levels: Vec<SubfolderPattern>) -> Result<Self, SubfolderError> {
        if levels.len() > MAX_LEVELS {
            return Err(SubfolderError::TooManyLevels(levels.len()));
        }
        Ok(SubfolderSpec { levels })
    }

    /// Resolve to the path components to append to the target folder.
    ///
    /// Levels that resolve to nothing are skipped, so a track missing the
    /// middle field still files under the outer one — it is never left behind.
    pub fn resolve(&self, facts: &TrackFacts<'_>, now: RunDate) -> Vec<String> {
        self.levels
            .iter()
            .filter_map(|level| level.resolve(facts, now))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const NOW: RunDate = RunDate {
        year: 2026,
        month: 8,
    };

    fn fields(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn facts<'a>(f: &'a HashMap<String, String>, tags: &'a [String]) -> TrackFacts<'a> {
        TrackFacts {
            fields: f,
            bitrate_kbps: None,
            tags,
            year: None,
        }
    }

    fn field(name: &str) -> SubfolderPattern {
        SubfolderPattern::Field {
            name: name.to_string(),
        }
    }

    #[test]
    fn genre_then_bpm_is_the_manuals_worked_example() {
        let f = fields(&[("genre", "House"), ("bpm", "128")]);
        let spec = SubfolderSpec::new(vec![field("genre"), field("bpm")]).unwrap();
        assert_eq!(spec.resolve(&facts(&f, &[]), NOW), vec!["House", "128"]);
    }

    #[test]
    fn an_empty_level_is_skipped_but_the_track_still_moves() {
        let f = fields(&[("bpm", "128")]);
        let spec = SubfolderSpec::new(vec![field("genre"), field("bpm")]).unwrap();
        assert_eq!(spec.resolve(&facts(&f, &[]), NOW), vec!["128"]);
    }

    #[test]
    fn every_level_empty_lands_in_the_target_folder_itself() {
        let f = fields(&[]);
        let spec = SubfolderSpec::new(vec![field("genre"), field("bpm")]).unwrap();
        assert!(spec.resolve(&facts(&f, &[]), NOW).is_empty());
    }

    #[test]
    fn whitespace_only_field_values_do_not_create_a_blank_folder() {
        let f = fields(&[("genre", "   ")]);
        let spec = SubfolderSpec::new(vec![field("genre")]).unwrap();
        assert!(spec.resolve(&facts(&f, &[]), NOW).is_empty());
    }

    #[test]
    fn separators_in_a_field_value_cannot_invent_extra_levels() {
        // "Drum & Bass / Jungle" must be one folder, not two.
        let f = fields(&[("genre", "Drum & Bass / Jungle")]);
        let spec = SubfolderSpec::new(vec![field("genre")]).unwrap();
        assert_eq!(
            spec.resolve(&facts(&f, &[]), NOW),
            vec!["Drum & Bass - Jungle"]
        );
    }

    #[test]
    fn bitrate_buckets_rather_than_raw_numbers() {
        let f = fields(&[]);
        let spec = SubfolderSpec::new(vec![SubfolderPattern::BitrateBucket]).unwrap();
        let mut fa = facts(&f, &[]);

        fa.bitrate_kbps = Some(320);
        assert_eq!(spec.resolve(&fa, NOW), vec!["320+"]);
        fa.bitrate_kbps = Some(1411);
        assert_eq!(spec.resolve(&fa, NOW), vec!["320+"]);
        fa.bitrate_kbps = Some(192);
        assert_eq!(spec.resolve(&fa, NOW), vec!["320-"]);
    }

    #[test]
    fn an_unknown_bitrate_drops_the_level_rather_than_guessing() {
        let f = fields(&[]);
        let spec = SubfolderSpec::new(vec![SubfolderPattern::BitrateBucket]).unwrap();
        assert!(spec.resolve(&facts(&f, &[]), NOW).is_empty());
    }

    #[test]
    fn first_tag_takes_the_first_of_the_supplied_order() {
        let f = fields(&[]);
        let tags = vec!["Peak time".to_string(), "Vocal".to_string()];
        let spec = SubfolderSpec::new(vec![SubfolderPattern::FirstTag]).unwrap();
        assert_eq!(spec.resolve(&facts(&f, &tags), NOW), vec!["Peak time"]);
    }

    #[test]
    fn an_untagged_track_skips_the_first_tag_level() {
        let f = fields(&[]);
        let spec = SubfolderSpec::new(vec![SubfolderPattern::FirstTag]).unwrap();
        assert!(spec.resolve(&facts(&f, &[]), NOW).is_empty());
    }

    #[test]
    fn date_buckets() {
        let f = fields(&[]);
        let spec = SubfolderSpec::new(vec![
            SubfolderPattern::CurrentYear,
            SubfolderPattern::CurrentMonth,
            SubfolderPattern::CurrentDecade,
        ])
        .unwrap();
        assert_eq!(
            spec.resolve(&facts(&f, &[]), NOW),
            vec!["2026", "08", "2020 - 2029"]
        );
    }

    #[test]
    fn release_decade_renders_as_a_range() {
        let f = fields(&[]);
        let spec = SubfolderSpec::new(vec![SubfolderPattern::ReleaseDecade]).unwrap();
        let mut fa = facts(&f, &[]);
        fa.year = Some(1994);
        assert_eq!(spec.resolve(&fa, NOW), vec!["1990 - 1999"]);
        fa.year = Some(2000);
        assert_eq!(spec.resolve(&fa, NOW), vec!["2000 - 2009"]);
    }

    #[test]
    fn a_track_with_no_release_year_skips_the_release_decade_level() {
        let f = fields(&[]);
        let spec = SubfolderSpec::new(vec![SubfolderPattern::ReleaseDecade]).unwrap();
        assert!(spec.resolve(&facts(&f, &[]), NOW).is_empty());
    }

    #[test]
    fn three_levels_are_allowed_and_four_are_not() {
        assert!(SubfolderSpec::new(vec![field("a"), field("b"), field("c")]).is_ok());
        assert_eq!(
            SubfolderSpec::new(vec![field("a"), field("b"), field("c"), field("d")]).unwrap_err(),
            SubfolderError::TooManyLevels(4)
        );
    }

    #[test]
    fn specs_round_trip_through_json() {
        let spec =
            SubfolderSpec::new(vec![field("genre"), SubfolderPattern::BitrateBucket]).unwrap();
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(serde_json::from_str::<SubfolderSpec>(&json).unwrap(), spec);
    }
}
