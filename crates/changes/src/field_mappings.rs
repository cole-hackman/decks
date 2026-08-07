//! Field Mappings — project fields that exist in `decks` into fields that exist
//! in the target.
//!
//! Energy, Danceability and Custom Tags have no column in Rekordbox and no
//! standard ID3 frame. A mapping writes them somewhere that does exist, usually
//! Comment, so the information survives the trip.
//!
//! Shared deliberately between sync and tag writing: the same `Energy → Comment`
//! rule should produce the same string whether it lands in `master.db` or in an
//! ID3 frame. One implementation, two call sites.
//!
//! See `docs/lexicon/01-interop.md §Field Mappings`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A field `decks` holds that the target may not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MappingSource {
    Energy,
    Danceability,
    Popularity,
    Happiness,
    /// Every custom tag, in hashtag form: `#Techno #Vocals`.
    AllCustomTags,
    /// Only the tags in one category, same hashtag form.
    TagCategory {
        name: String,
    },
    /// Track colour. Written as the colour's *name*, since a target that has no
    /// colour concept cannot do anything with a hex value.
    Colour,
}

impl MappingSource {
    /// The prefix a projected value carries, so `Energy 08` is readable in a
    /// comment field shared with other mappings.
    pub fn label(&self) -> &str {
        match self {
            MappingSource::Energy => "Energy",
            MappingSource::Danceability => "Dance",
            MappingSource::Popularity => "Pop",
            MappingSource::Happiness => "Happy",
            MappingSource::AllCustomTags | MappingSource::TagCategory { .. } => "",
            MappingSource::Colour => "Colour",
        }
    }
}

/// One source → target rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMapping {
    pub source: MappingSource,
    /// Target field name, in the vocabulary of whatever is being written.
    pub target: String,
    /// True replaces the target's existing value; false appends to it.
    #[serde(default)]
    pub overwrite: bool,
}

/// The values a projection can draw on.
///
/// Everything is optional: a track with no energy simply contributes nothing,
/// rather than writing "Energy" with no number after it.
#[derive(Debug, Clone, Default)]
pub struct MappingInput {
    /// 0–10, already on the scale the UI shows.
    pub energy: Option<u8>,
    pub danceability: Option<u8>,
    pub popularity: Option<u8>,
    pub happiness: Option<u8>,
    /// `(category, tag)` pairs, in display order.
    pub tags: Vec<(String, String)>,
    pub colour_name: Option<String>,
}

/// Numbers are zero-padded to two digits so a target that sorts text keeps them
/// in order — the same reason Lexicon's key conversion has a leading-zero
/// option.
fn number(label: &str, value: u8) -> String {
    format!("{label} {value:02}")
}

fn hashtags<'a>(tags: impl Iterator<Item = &'a str>) -> Option<String> {
    let joined: Vec<String> = tags.map(|t| format!("#{}", t.replace(' ', ""))).collect();
    if joined.is_empty() {
        None
    } else {
        Some(joined.join(" "))
    }
}

impl MappingSource {
    /// Render this source for one track, or `None` when the track has no value
    /// for it.
    pub fn render(&self, input: &MappingInput) -> Option<String> {
        match self {
            MappingSource::Energy => input.energy.map(|v| number(self.label(), v)),
            MappingSource::Danceability => input.danceability.map(|v| number(self.label(), v)),
            MappingSource::Popularity => input.popularity.map(|v| number(self.label(), v)),
            MappingSource::Happiness => input.happiness.map(|v| number(self.label(), v)),
            MappingSource::AllCustomTags => hashtags(input.tags.iter().map(|(_, t)| t.as_str())),
            MappingSource::TagCategory { name } => hashtags(
                input
                    .tags
                    .iter()
                    .filter(|(c, _)| c.eq_ignore_ascii_case(name))
                    .map(|(_, t)| t.as_str()),
            ),
            MappingSource::Colour => input.colour_name.clone(),
        }
    }
}

/// An ordered set of mappings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMappings {
    pub mappings: Vec<FieldMapping>,
}

/// How multiple sources landing on one target are joined.
pub const COMBINE_SEPARATOR: &str = ", ";

impl FieldMappings {
    pub fn new(mappings: Vec<FieldMapping>) -> Self {
        FieldMappings { mappings }
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Project a track's values onto its targets.
    ///
    /// `existing` is what the target already holds, so an appending mapping has
    /// something to append to. Returns only the targets a mapping actually
    /// produced a value for — a track with nothing to say about a target leaves
    /// it alone rather than blanking it.
    ///
    /// Several sources on one target combine with `, `, in mapping order. The
    /// **first** mapping for a target decides overwrite-vs-append: mixing the
    /// two on one target is a configuration mistake, and picking the first is
    /// both predictable and matches reading order.
    pub fn project(
        &self,
        input: &MappingInput,
        existing: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let mut parts: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut overwrite: BTreeMap<String, bool> = BTreeMap::new();

        for mapping in &self.mappings {
            let Some(value) = mapping.source.render(input) else {
                continue;
            };
            overwrite
                .entry(mapping.target.clone())
                .or_insert(mapping.overwrite);
            parts.entry(mapping.target.clone()).or_default().push(value);
        }

        let mut out = BTreeMap::new();
        for (target, values) in parts {
            let combined = values.join(COMBINE_SEPARATOR);
            let replaces = overwrite.get(&target).copied().unwrap_or(false);
            let value = match (replaces, existing.get(&target)) {
                (true, _) => combined,
                (false, Some(prev)) if !prev.trim().is_empty() => {
                    format!("{}{COMBINE_SEPARATOR}{combined}", prev.trim())
                }
                (false, _) => combined,
            };
            out.insert(target, value);
        }
        out
    }

    /// Every target any mapping writes to, so a caller can warn before
    /// overwriting a field the user cares about.
    pub fn targets(&self) -> Vec<&str> {
        let mut seen = Vec::new();
        for m in &self.mappings {
            if !seen.contains(&m.target.as_str()) {
                seen.push(m.target.as_str());
            }
        }
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(source: MappingSource, target: &str, overwrite: bool) -> FieldMapping {
        FieldMapping {
            source,
            target: target.to_string(),
            overwrite,
        }
    }

    fn input() -> MappingInput {
        MappingInput {
            energy: Some(8),
            popularity: Some(5),
            tags: vec![
                ("Genre".into(), "Techno".into()),
                ("Vibe".into(), "Peak time".into()),
            ],
            colour_name: Some("Red_Dark".into()),
            ..Default::default()
        }
    }

    fn existing(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn energy_to_comment_is_the_manuals_worked_example() {
        let m = FieldMappings::new(vec![map(MappingSource::Energy, "Comment", true)]);
        let got = m.project(&input(), &existing(&[]));
        assert_eq!(got.get("Comment").map(String::as_str), Some("Energy 08"));
    }

    #[test]
    fn several_sources_on_one_target_combine_in_mapping_order() {
        let m = FieldMappings::new(vec![
            map(MappingSource::Energy, "Comment", true),
            map(MappingSource::Popularity, "Comment", true),
        ]);
        let got = m.project(&input(), &existing(&[]));
        assert_eq!(
            got.get("Comment").map(String::as_str),
            Some("Energy 08, Pop 05")
        );
    }

    #[test]
    fn overwrite_off_appends_to_what_is_already_there() {
        let m = FieldMappings::new(vec![map(MappingSource::Energy, "Comment", false)]);
        let got = m.project(&input(), &existing(&[("Comment", "Great intro")]));
        assert_eq!(
            got.get("Comment").map(String::as_str),
            Some("Great intro, Energy 08")
        );
    }

    #[test]
    fn overwrite_on_replaces_it() {
        let m = FieldMappings::new(vec![map(MappingSource::Energy, "Comment", true)]);
        let got = m.project(&input(), &existing(&[("Comment", "Great intro")]));
        assert_eq!(got.get("Comment").map(String::as_str), Some("Energy 08"));
    }

    #[test]
    fn appending_to_an_empty_target_does_not_leave_a_leading_separator() {
        let m = FieldMappings::new(vec![map(MappingSource::Energy, "Comment", false)]);
        let got = m.project(&input(), &existing(&[("Comment", "   ")]));
        assert_eq!(got.get("Comment").map(String::as_str), Some("Energy 08"));
    }

    #[test]
    fn all_custom_tags_write_the_hashtag_form() {
        let m = FieldMappings::new(vec![map(MappingSource::AllCustomTags, "Comment", true)]);
        let got = m.project(&input(), &existing(&[]));
        // Spaces inside a tag are removed — "#Peak time" would read as two tags.
        assert_eq!(
            got.get("Comment").map(String::as_str),
            Some("#Techno #Peaktime")
        );
    }

    #[test]
    fn a_single_tag_category_can_be_the_source_instead() {
        let m = FieldMappings::new(vec![map(
            MappingSource::TagCategory {
                name: "Genre".into(),
            },
            "Comment",
            true,
        )]);
        let got = m.project(&input(), &existing(&[]));
        assert_eq!(got.get("Comment").map(String::as_str), Some("#Techno"));
    }

    #[test]
    fn a_tag_category_matches_case_insensitively() {
        let m = FieldMappings::new(vec![map(
            MappingSource::TagCategory {
                name: "genre".into(),
            },
            "Comment",
            true,
        )]);
        assert_eq!(
            m.project(&input(), &existing(&[]))
                .get("Comment")
                .map(String::as_str),
            Some("#Techno")
        );
    }

    #[test]
    fn colour_writes_its_name_because_a_text_target_cannot_use_a_hex_value() {
        let m = FieldMappings::new(vec![map(MappingSource::Colour, "Grouping", true)]);
        let got = m.project(&input(), &existing(&[]));
        assert_eq!(got.get("Grouping").map(String::as_str), Some("Red_Dark"));
    }

    #[test]
    fn a_track_with_no_value_for_a_source_contributes_nothing() {
        // Not "Energy" with no number, and not a blanked target.
        let m = FieldMappings::new(vec![map(MappingSource::Energy, "Comment", true)]);
        let got = m.project(&MappingInput::default(), &existing(&[("Comment", "keep")]));
        assert!(got.is_empty());
    }

    #[test]
    fn an_empty_tag_list_contributes_nothing_rather_than_an_empty_string() {
        let m = FieldMappings::new(vec![map(MappingSource::AllCustomTags, "Comment", true)]);
        assert!(m
            .project(&MappingInput::default(), &existing(&[]))
            .is_empty());
    }

    #[test]
    fn the_first_mapping_for_a_target_decides_overwrite() {
        // Mixing the two on one target is a configuration mistake; picking the
        // first is predictable and matches reading order.
        let m = FieldMappings::new(vec![
            map(MappingSource::Energy, "Comment", false),
            map(MappingSource::Popularity, "Comment", true),
        ]);
        let got = m.project(&input(), &existing(&[("Comment", "prev")]));
        assert_eq!(
            got.get("Comment").map(String::as_str),
            Some("prev, Energy 08, Pop 05")
        );
    }

    #[test]
    fn numbers_are_zero_padded_so_a_text_target_sorts_them_correctly() {
        let m = FieldMappings::new(vec![map(MappingSource::Energy, "Comment", true)]);
        let low = MappingInput {
            energy: Some(3),
            ..Default::default()
        };
        assert_eq!(
            m.project(&low, &existing(&[]))
                .get("Comment")
                .map(String::as_str),
            Some("Energy 03")
        );
    }

    #[test]
    fn targets_are_reported_once_each_in_order() {
        let m = FieldMappings::new(vec![
            map(MappingSource::Energy, "Comment", true),
            map(MappingSource::Popularity, "Comment", true),
            map(MappingSource::Colour, "Grouping", true),
        ]);
        assert_eq!(m.targets(), vec!["Comment", "Grouping"]);
    }

    #[test]
    fn mappings_round_trip_through_json() {
        let m = FieldMappings::new(vec![
            map(MappingSource::Energy, "Comment", true),
            map(
                MappingSource::TagCategory {
                    name: "Genre".into(),
                },
                "Grouping",
                false,
            ),
        ]);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<FieldMappings>(&json).unwrap(), m);
    }

    #[test]
    fn no_mappings_produces_no_writes() {
        assert!(FieldMappings::default()
            .project(&input(), &existing(&[("Comment", "keep")]))
            .is_empty());
    }
}
