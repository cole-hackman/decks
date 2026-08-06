//! Tag recipes.
//!
//! Separate from the field recipes because tags are a *set* attached to a
//! track, not a string on it. The operations are therefore expressed as a
//! change to that set rather than a new field value, which is also what the
//! cache's tag accessors want.
//!
//! `ImportFromText` is the one the spec singles out: users have been
//! hand-rolling `#Techno #Vocals` in the comment field for years, and this is
//! the migration path off it. It has to be **idempotent** — running it twice
//! must not duplicate anything, and must not disturb tags added by hand.

use serde::{Deserialize, Serialize};

use crate::fields::TrackFields;

/// A tag operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TagRecipe {
    /// Convert a hashtag convention in a text field into real tags.
    ImportFromText {
        /// Defaults to Comment in the UI, per the spec.
        field: String,
        /// The marker that introduces a tag. Defaults to `#`.
        separator: String,
    },
    AddTags {
        tags: Vec<String>,
    },
    RemoveTags {
        tags: Vec<String>,
    },
    ReplaceTag {
        from: String,
        to: String,
    },
    ClearTags,
}

/// What a tag recipe would do to one track's tags.
///
/// Expressed as a delta rather than a new list so the caller can write it
/// through the cache's existing add/remove accessors without diffing again,
/// and so a preview can say "adds 3, removes 1" rather than showing two lists.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagChange {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

impl TagChange {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

/// Tags are compared case-insensitively.
///
/// `#techno` and `#Techno` are the same tag to a human, and a library that
/// ends up with both is exactly the mess this feature is meant to clean up.
fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn contains(haystack: &[String], needle: &str) -> bool {
    haystack.iter().any(|t| same(t, needle))
}

/// Pull hashtag-style tags out of a text value.
///
/// A tag runs from the separator to the next whitespace, so `#Peak time`
/// yields `Peak` — matching how the convention is actually written, where a
/// multi-word tag is spelled `#PeakTime`. Punctuation that commonly trails a
/// tag (`,` `.` `;`) is trimmed so `#Techno, #Vocals` gives two clean tags.
pub fn parse_hashtags(value: &str, separator: &str) -> Vec<String> {
    if separator.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut rest = value;

    while let Some(at) = rest.find(separator) {
        rest = &rest[at + separator.len()..];
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let (tag, remainder) = rest.split_at(end);
        rest = remainder;

        let tag = tag.trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | '!' | '?'));
        if tag.is_empty() {
            continue;
        }
        // Deduplicate within one parse, so "#Techno #techno" is one tag.
        if !contains(&out, tag) {
            out.push(tag.to_string());
        }
    }
    out
}

/// Work out what a tag recipe would change.
///
/// `current` is the track's tags today. Nothing is removed that the recipe did
/// not ask for, and nothing is added twice.
pub fn apply_tag_recipe(recipe: &TagRecipe, fields: &TrackFields, current: &[String]) -> TagChange {
    let mut change = TagChange::default();

    match recipe {
        TagRecipe::ImportFromText { field, separator } => {
            let Some(value) = fields.get(field) else {
                return change;
            };
            for tag in parse_hashtags(value, separator) {
                // Idempotent: a tag the track already has is left alone, and
                // nothing existing is ever removed. Re-running is a no-op.
                if !contains(current, &tag) && !contains(&change.added, &tag) {
                    change.added.push(tag);
                }
            }
        }
        TagRecipe::AddTags { tags } => {
            for tag in tags {
                let tag = tag.trim();
                if tag.is_empty() {
                    continue;
                }
                if !contains(current, tag) && !contains(&change.added, tag) {
                    change.added.push(tag.to_string());
                }
            }
        }
        TagRecipe::RemoveTags { tags } => {
            for existing in current {
                if contains(tags, existing) && !contains(&change.removed, existing) {
                    change.removed.push(existing.clone());
                }
            }
        }
        TagRecipe::ReplaceTag { from, to } => {
            let to = to.trim();
            if to.is_empty() || !contains(current, from) {
                return change;
            }
            for existing in current {
                if same(existing, from) {
                    change.removed.push(existing.clone());
                }
            }
            // Replacing with a tag the track already has is a removal only —
            // otherwise the track ends up with it twice.
            if !contains(current, to) {
                change.added.push(to.to_string());
            }
        }
        TagRecipe::ClearTags => {
            change.removed = current.to_vec();
        }
    }

    change
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn fields(pairs: &[(&str, &str)]) -> TrackFields {
        pairs.iter().copied().collect()
    }

    fn import() -> TagRecipe {
        TagRecipe::ImportFromText {
            field: "comment".into(),
            separator: "#".into(),
        }
    }

    // ── parsing ─────────────────────────────────────────────────────────────

    #[test]
    fn hashtags_are_pulled_out_of_a_comment() {
        assert_eq!(
            parse_hashtags("#Techno #Vocals", "#"),
            tags(&["Techno", "Vocals"])
        );
    }

    #[test]
    fn text_around_the_tags_is_ignored() {
        assert_eq!(
            parse_hashtags("great intro #Techno more words #Vocals", "#"),
            tags(&["Techno", "Vocals"])
        );
    }

    #[test]
    fn trailing_punctuation_is_trimmed() {
        assert_eq!(
            parse_hashtags("#Techno, #Vocals.", "#"),
            tags(&["Techno", "Vocals"])
        );
    }

    #[test]
    fn a_tag_ends_at_whitespace_because_that_is_how_the_convention_is_written() {
        // A multi-word tag is spelled "#PeakTime"; "#Peak time" is one tag
        // followed by a word.
        assert_eq!(parse_hashtags("#Peak time", "#"), tags(&["Peak"]));
        assert_eq!(parse_hashtags("#PeakTime", "#"), tags(&["PeakTime"]));
    }

    #[test]
    fn a_lone_separator_yields_no_tag() {
        assert_eq!(parse_hashtags("# #Techno", "#"), tags(&["Techno"]));
    }

    #[test]
    fn duplicates_within_one_comment_collapse_case_insensitively() {
        assert_eq!(parse_hashtags("#Techno #techno", "#"), tags(&["Techno"]));
    }

    #[test]
    fn a_custom_separator_works() {
        assert_eq!(
            parse_hashtags("@Techno @Vocals", "@"),
            tags(&["Techno", "Vocals"])
        );
    }

    #[test]
    fn an_empty_separator_parses_nothing_rather_than_looping() {
        assert!(parse_hashtags("#Techno", "").is_empty());
    }

    #[test]
    fn a_comment_with_no_tags_yields_none() {
        assert!(parse_hashtags("just a normal comment", "#").is_empty());
    }

    // ── import ──────────────────────────────────────────────────────────────

    #[test]
    fn import_adds_the_tags_it_finds() {
        let got = apply_tag_recipe(&import(), &fields(&[("comment", "#Techno #Vocals")]), &[]);
        assert_eq!(got.added, tags(&["Techno", "Vocals"]));
        assert!(got.removed.is_empty());
    }

    #[test]
    fn import_is_idempotent() {
        // The whole point: safe to re-run over a library repeatedly.
        let f = fields(&[("comment", "#Techno #Vocals")]);
        let current = tags(&["Techno", "Vocals"]);
        let got = apply_tag_recipe(&import(), &f, &current);
        assert!(got.is_empty());
    }

    #[test]
    fn import_preserves_tags_added_by_hand() {
        // It only ever adds — a tag not mentioned in the comment stays.
        let f = fields(&[("comment", "#Techno")]);
        let got = apply_tag_recipe(&import(), &f, &tags(&["Handpicked"]));
        assert_eq!(got.added, tags(&["Techno"]));
        assert!(got.removed.is_empty());
    }

    #[test]
    fn import_matches_existing_tags_case_insensitively() {
        // Otherwise the library ends up holding both "#techno" and "#Techno".
        let f = fields(&[("comment", "#techno")]);
        let got = apply_tag_recipe(&import(), &f, &tags(&["Techno"]));
        assert!(got.is_empty());
    }

    #[test]
    fn import_from_an_empty_field_does_nothing() {
        let got = apply_tag_recipe(&import(), &fields(&[("title", "T")]), &[]);
        assert!(got.is_empty());
    }

    // ── add / remove / replace / clear ──────────────────────────────────────

    #[test]
    fn add_skips_tags_the_track_already_has() {
        let got = apply_tag_recipe(
            &TagRecipe::AddTags {
                tags: tags(&["Techno", "Vocals"]),
            },
            &TrackFields::new(),
            &tags(&["techno"]),
        );
        assert_eq!(got.added, tags(&["Vocals"]));
    }

    #[test]
    fn add_ignores_blank_entries() {
        let got = apply_tag_recipe(
            &TagRecipe::AddTags {
                tags: tags(&["  ", "Techno"]),
            },
            &TrackFields::new(),
            &[],
        );
        assert_eq!(got.added, tags(&["Techno"]));
    }

    #[test]
    fn remove_only_touches_tags_the_track_actually_has() {
        let got = apply_tag_recipe(
            &TagRecipe::RemoveTags {
                tags: tags(&["Techno", "Absent"]),
            },
            &TrackFields::new(),
            &tags(&["Techno", "Vocals"]),
        );
        assert_eq!(got.removed, tags(&["Techno"]));
        assert!(got.added.is_empty());
    }

    #[test]
    fn replace_swaps_one_tag_for_another() {
        let got = apply_tag_recipe(
            &TagRecipe::ReplaceTag {
                from: "Techno".into(),
                to: "Tech House".into(),
            },
            &TrackFields::new(),
            &tags(&["Techno", "Vocals"]),
        );
        assert_eq!(got.removed, tags(&["Techno"]));
        assert_eq!(got.added, tags(&["Tech House"]));
    }

    #[test]
    fn replacing_with_a_tag_the_track_already_has_is_a_removal_only() {
        // Otherwise the track ends up holding it twice.
        let got = apply_tag_recipe(
            &TagRecipe::ReplaceTag {
                from: "Techno".into(),
                to: "Vocals".into(),
            },
            &TrackFields::new(),
            &tags(&["Techno", "Vocals"]),
        );
        assert_eq!(got.removed, tags(&["Techno"]));
        assert!(got.added.is_empty());
    }

    #[test]
    fn replacing_a_tag_the_track_does_not_have_does_nothing() {
        let got = apply_tag_recipe(
            &TagRecipe::ReplaceTag {
                from: "Absent".into(),
                to: "New".into(),
            },
            &TrackFields::new(),
            &tags(&["Techno"]),
        );
        assert!(got.is_empty());
    }

    #[test]
    fn replacing_with_nothing_is_refused_rather_than_deleting() {
        // An empty target would silently turn Replace into Remove.
        let got = apply_tag_recipe(
            &TagRecipe::ReplaceTag {
                from: "Techno".into(),
                to: "  ".into(),
            },
            &TrackFields::new(),
            &tags(&["Techno"]),
        );
        assert!(got.is_empty());
    }

    #[test]
    fn clear_removes_everything_and_adds_nothing() {
        let got = apply_tag_recipe(
            &TagRecipe::ClearTags,
            &TrackFields::new(),
            &tags(&["Techno", "Vocals"]),
        );
        assert_eq!(got.removed, tags(&["Techno", "Vocals"]));
        assert!(got.added.is_empty());
    }

    #[test]
    fn clearing_an_untagged_track_is_a_no_op() {
        let got = apply_tag_recipe(&TagRecipe::ClearTags, &TrackFields::new(), &[]);
        assert!(got.is_empty());
    }

    #[test]
    fn tag_recipes_round_trip_through_json() {
        let recipes = vec![
            import(),
            TagRecipe::AddTags {
                tags: tags(&["Techno"]),
            },
            TagRecipe::ReplaceTag {
                from: "a".into(),
                to: "b".into(),
            },
            TagRecipe::ClearTags,
        ];
        let json = serde_json::to_string(&recipes).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<TagRecipe>>(&json).unwrap(),
            recipes
        );
    }
}
