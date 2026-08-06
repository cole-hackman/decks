//! Recipes — parameterized bulk-edit operations applied per track.
//!
//! Distinct from `crates/smart-fixes`, which is the *other* bulk-editing
//! system: ten fixed, zero-parameter cleanups. A recipe takes parameters, and a
//! user assembles the one they need rather than picking from a fixed menu.
//!
//! Everything here is a pure function of (recipe, fields) → fields. Nothing
//! reads a database, touches a file, or knows what a track is. That keeps the
//! whole operation vocabulary testable in isolation and lets the same engine
//! back a preview screen, an apply pass and an agent tool without three
//! implementations.
//!
//! Tag recipes live in [`tags`] rather than alongside the field recipes,
//! because tags are a *set* attached to a track rather than a string on it —
//! the operations are expressed as a delta to that set.
//!
//! Cue and beatgrid recipes are deliberately **not** here. They operate on cue
//! lists and beat grids rather than text fields, they need the quantize
//! arithmetic from `crates/rekordbox-db`, and the spec itself says to sequence
//! field/text/tag recipes first. See `docs/lexicon/10-recipes.md`.

pub mod casing;
pub mod fields;
pub mod tags;
pub mod text;

use serde::{Deserialize, Serialize};

pub use fields::{diff, FieldChange, TrackFields};
pub use tags::{apply_tag_recipe, parse_hashtags, TagChange, TagRecipe};
pub use text::{DelimiterPair, SpecialCharacterMode};

/// One parameterized operation.
///
/// Serde-tagged so a recipe can be stored, shared and replayed — the whole
/// point of the feature is that a user builds one once and runs it on every
/// batch of downloads thereafter.
///
/// `PartialEq` but not `Eq`: `AdjustNumber` carries an `f64`, and a recipe
/// list is compared for "is this the same configuration", never used as a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Recipe {
    // ── Casing ──────────────────────────────────────────────────────────────
    ToUpperCase {
        field: String,
        #[serde(default)]
        ignore_words: Vec<String>,
    },
    ToLowerCase {
        field: String,
        #[serde(default)]
        ignore_words: Vec<String>,
    },
    ToTitleCase {
        field: String,
        #[serde(default)]
        ignore_words: Vec<String>,
    },
    ToSentenceCase {
        field: String,
    },

    // ── Field ───────────────────────────────────────────────────────────────
    /// Copy, leaving the source intact.
    CopyField {
        from: String,
        to: String,
    },
    /// Copy, then clear the source.
    MoveField {
        from: String,
        to: String,
    },
    MergeFields {
        first: String,
        second: String,
        target: String,
        #[serde(default)]
        separator: String,
    },
    PrefixField {
        field: String,
        text: String,
    },
    SuffixField {
        field: String,
        text: String,
    },
    SwapFields {
        first: String,
        second: String,
    },
    SplitField {
        field: String,
        delimiter: String,
        first_target: String,
        second_target: String,
        /// Keep the delimiter attached to the first part.
        #[serde(default)]
        preserve_split_text: bool,
        /// Add to the targets rather than overwrite them.
        #[serde(default)]
        append: bool,
    },

    // ── Text ────────────────────────────────────────────────────────────────
    RemoveText {
        field: String,
        text: String,
        #[serde(default)]
        case_insensitive: bool,
    },
    ReplaceText {
        field: String,
        find: String,
        replace: String,
        #[serde(default)]
        case_insensitive: bool,
    },
    ExtractText {
        field: String,
        start: String,
        end: String,
        target: String,
        #[serde(default)]
        include_delimiters: bool,
        #[serde(default)]
        delete_from_source: bool,
        #[serde(default)]
        append: bool,
    },
    ShortenText {
        field: String,
        chars_per_word: usize,
    },
    RemoveSpecialCharacters {
        field: String,
        mode: SpecialCharacterMode,
    },
    RemoveBetween {
        field: String,
        pair: DelimiterPair,
    },

    // ── Number ──────────────────────────────────────────────────────────────
    /// Signed, so one operation covers increase and decrease.
    AdjustNumber {
        field: String,
        amount: f64,
    },
}

/// Why a recipe did nothing.
///
/// Reported rather than swallowed: "23 of 400 tracks unchanged" is useful, and
/// "nothing happened" is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Skipped {
    /// The source field has no value on this track.
    SourceEmpty { field: String },
    /// The delimiters or search text were not found.
    NoMatch { field: String },
    /// The field's value is not a number.
    NotANumber { field: String, value: String },
    /// The recipe's own parameters make it a no-op.
    Misconfigured { detail: String },
}

/// The result of running one recipe against one track.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeOutcome {
    pub changes: Vec<FieldChange>,
    pub skipped: Option<Skipped>,
}

impl RecipeOutcome {
    pub fn changed(&self) -> bool {
        !self.changes.is_empty()
    }
}

/// Append `value` to `target`, or overwrite it.
///
/// Appending to an empty target must not leave a leading separator, which is
/// the same rule the field-mapping engine follows.
fn write(out: &mut TrackFields, target: &str, value: &str, append: bool) {
    match (append, out.get(target)) {
        (true, Some(existing)) => {
            let joined = format!("{existing} {value}");
            out.set(target, joined);
        }
        _ => out.set(target, value),
    }
}

/// Run one recipe against one track's fields.
///
/// Never partially applies: the recipe works on a clone, and the caller gets
/// both the new field set and a description of what moved. A recipe that cannot
/// run reports why and leaves the track alone.
pub fn apply(recipe: &Recipe, input: &TrackFields) -> (TrackFields, RecipeOutcome) {
    let mut out = input.clone();
    let mut skipped = None;

    match recipe {
        Recipe::ToUpperCase {
            field,
            ignore_words,
        } => match input.get(field) {
            Some(v) => out.set(field, casing::to_upper(v, ignore_words)),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::ToLowerCase {
            field,
            ignore_words,
        } => match input.get(field) {
            Some(v) => out.set(field, casing::to_lower(v, ignore_words)),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::ToTitleCase {
            field,
            ignore_words,
        } => match input.get(field) {
            Some(v) => out.set(field, casing::to_title(v, ignore_words)),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::ToSentenceCase { field } => match input.get(field) {
            Some(v) => out.set(field, casing::to_sentence(v)),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },

        Recipe::CopyField { from, to } => match input.get(from) {
            Some(v) => out.set(to, v),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: from.clone(),
                })
            }
        },
        Recipe::MoveField { from, to } => match input.get(from) {
            Some(v) => {
                out.set(to, v);
                out.clear(from);
            }
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: from.clone(),
                })
            }
        },
        Recipe::MergeFields {
            first,
            second,
            target,
            separator,
        } => {
            // A missing half is not a failure — merging artist and remixer
            // where there is no remixer should yield the artist, not nothing.
            let merged = match (input.get(first), input.get(second)) {
                (Some(a), Some(b)) => Some(format!("{a}{separator}{b}")),
                (Some(a), None) => Some(a.to_string()),
                (None, Some(b)) => Some(b.to_string()),
                (None, None) => None,
            };
            match merged {
                Some(v) => out.set(target, v),
                None => {
                    skipped = Some(Skipped::SourceEmpty {
                        field: first.clone(),
                    })
                }
            }
        }
        Recipe::PrefixField { field, text } => match input.get(field) {
            Some(v) => out.set(field, format!("{text}{v}")),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::SuffixField { field, text } => match input.get(field) {
            Some(v) => out.set(field, format!("{v}{text}")),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::SwapFields { first, second } => {
            let a = input.raw(first).map(String::from);
            let b = input.raw(second).map(String::from);
            match (&a, &b) {
                (None, None) => {
                    skipped = Some(Skipped::SourceEmpty {
                        field: first.clone(),
                    })
                }
                _ => {
                    match b {
                        Some(v) => out.set(first, v),
                        None => out.clear(first),
                    }
                    match a {
                        Some(v) => out.set(second, v),
                        None => out.clear(second),
                    }
                }
            }
        }
        Recipe::SplitField {
            field,
            delimiter,
            first_target,
            second_target,
            preserve_split_text,
            append,
        } => {
            if delimiter.is_empty() {
                skipped = Some(Skipped::Misconfigured {
                    detail: "a delimiter is required to split".into(),
                });
            } else {
                match input.get(field) {
                    None => {
                        skipped = Some(Skipped::SourceEmpty {
                            field: field.clone(),
                        })
                    }
                    Some(v) => match v.find(delimiter.as_str()) {
                        None => {
                            skipped = Some(Skipped::NoMatch {
                                field: field.clone(),
                            })
                        }
                        Some(at) => {
                            let head_end = if *preserve_split_text {
                                at + delimiter.len()
                            } else {
                                at
                            };
                            let head = v[..head_end].trim().to_string();
                            let tail = v[at + delimiter.len()..].trim().to_string();
                            write(&mut out, first_target, &head, *append);
                            write(&mut out, second_target, &tail, *append);
                        }
                    },
                }
            }
        }

        Recipe::RemoveText {
            field,
            text,
            case_insensitive,
        } => match input.get(field) {
            Some(v) => out.set(field, text::remove_text(v, text, *case_insensitive)),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::ReplaceText {
            field,
            find,
            replace,
            case_insensitive,
        } => match input.get(field) {
            Some(v) => out.set(
                field,
                text::replace_text(v, find, replace, *case_insensitive),
            ),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::ExtractText {
            field,
            start,
            end,
            target,
            include_delimiters,
            delete_from_source,
            append,
        } => match input.get(field) {
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
            Some(v) => {
                let got =
                    text::extract_text(v, start, end, *include_delimiters, *delete_from_source);
                match got.extracted {
                    None => {
                        skipped = Some(Skipped::NoMatch {
                            field: field.clone(),
                        })
                    }
                    Some(extracted) => {
                        write(&mut out, target, extracted.trim(), *append);
                        if *delete_from_source {
                            out.set(field, got.remaining_source);
                        }
                    }
                }
            }
        },
        Recipe::ShortenText {
            field,
            chars_per_word,
        } => match input.get(field) {
            Some(v) => out.set(field, text::shorten_text(v, *chars_per_word)),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::RemoveSpecialCharacters { field, mode } => match input.get(field) {
            Some(v) => out.set(field, text::remove_special_characters(v, *mode)),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },
        Recipe::RemoveBetween { field, pair } => match input.get(field) {
            Some(v) => out.set(field, text::remove_between(v, *pair)),
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
        },

        Recipe::AdjustNumber { field, amount } => match input.get(field) {
            None => {
                skipped = Some(Skipped::SourceEmpty {
                    field: field.clone(),
                })
            }
            Some(v) => match v.trim().parse::<f64>() {
                Err(_) => {
                    skipped = Some(Skipped::NotANumber {
                        field: field.clone(),
                        value: v.to_string(),
                    })
                }
                Ok(n) => {
                    let result = n + amount;
                    // An integer in, an integer out — bumping a track number
                    // from 3 to 4 must not write "4".
                    let formatted = if v.contains('.') || result.fract().abs() > f64::EPSILON {
                        format!("{result}")
                    } else {
                        format!("{}", result as i64)
                    };
                    out.set(field, formatted);
                }
            },
        },
    }

    let changes = diff(input, &out);
    (out, RecipeOutcome { changes, skipped })
}

/// Run several recipes in order, threading the result of each into the next.
///
/// Order matters and is the user's: extracting a remixer before title-casing
/// gives a different result from doing it after, and the engine must not
/// second-guess which they meant.
pub fn apply_all(recipes: &[Recipe], input: &TrackFields) -> (TrackFields, Vec<RecipeOutcome>) {
    let mut current = input.clone();
    let mut outcomes = Vec::with_capacity(recipes.len());
    for recipe in recipes {
        let (next, outcome) = apply(recipe, &current);
        current = next;
        outcomes.push(outcome);
    }
    (current, outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> TrackFields {
        pairs.iter().copied().collect()
    }

    fn run(recipe: Recipe, pairs: &[(&str, &str)]) -> (TrackFields, RecipeOutcome) {
        apply(&recipe, &fields(pairs))
    }

    // ── Casing ──────────────────────────────────────────────────────────────

    #[test]
    fn casing_recipes_edit_the_named_field_only() {
        let (out, outcome) = run(
            Recipe::ToTitleCase {
                field: "title".into(),
                ignore_words: vec![],
            },
            &[("title", "get lucky"), ("artist", "daft punk")],
        );
        assert_eq!(out.get("title"), Some("Get Lucky"));
        assert_eq!(out.get("artist"), Some("daft punk"));
        assert_eq!(outcome.changes.len(), 1);
    }

    #[test]
    fn a_recipe_on_an_empty_field_reports_why_rather_than_silently_doing_nothing() {
        let (out, outcome) = run(
            Recipe::ToUpperCase {
                field: "remixer".into(),
                ignore_words: vec![],
            },
            &[("title", "T")],
        );
        assert!(!outcome.changed());
        assert_eq!(
            outcome.skipped,
            Some(Skipped::SourceEmpty {
                field: "remixer".into()
            })
        );
        assert_eq!(out, fields(&[("title", "T")]));
    }

    #[test]
    fn a_recipe_that_changes_nothing_reports_no_changes() {
        let (_, outcome) = run(
            Recipe::ToUpperCase {
                field: "title".into(),
                ignore_words: vec![],
            },
            &[("title", "ALREADY UPPER")],
        );
        assert!(!outcome.changed());
        assert!(outcome.skipped.is_none());
    }

    // ── Field ───────────────────────────────────────────────────────────────

    #[test]
    fn copy_leaves_the_source_intact_and_move_does_not() {
        let (copied, _) = run(
            Recipe::CopyField {
                from: "title".into(),
                to: "comment".into(),
            },
            &[("title", "T")],
        );
        assert_eq!(copied.get("title"), Some("T"));
        assert_eq!(copied.get("comment"), Some("T"));

        let (moved, _) = run(
            Recipe::MoveField {
                from: "title".into(),
                to: "comment".into(),
            },
            &[("title", "T")],
        );
        assert!(moved.get("title").is_none());
        assert_eq!(moved.get("comment"), Some("T"));
    }

    #[test]
    fn merge_joins_two_fields_with_a_separator() {
        let (out, _) = run(
            Recipe::MergeFields {
                first: "artist".into(),
                second: "title".into(),
                target: "comment".into(),
                separator: " - ".into(),
            },
            &[("artist", "Daft Punk"), ("title", "Get Lucky")],
        );
        assert_eq!(out.get("comment"), Some("Daft Punk - Get Lucky"));
    }

    #[test]
    fn merge_with_one_half_missing_yields_the_other_half_not_a_stray_separator() {
        let (out, _) = run(
            Recipe::MergeFields {
                first: "artist".into(),
                second: "remixer".into(),
                target: "comment".into(),
                separator: " & ".into(),
            },
            &[("artist", "Daft Punk")],
        );
        assert_eq!(out.get("comment"), Some("Daft Punk"));
    }

    #[test]
    fn merge_with_both_halves_missing_is_a_skip() {
        let (_, outcome) = run(
            Recipe::MergeFields {
                first: "a".into(),
                second: "b".into(),
                target: "c".into(),
                separator: "-".into(),
            },
            &[("title", "T")],
        );
        assert!(matches!(outcome.skipped, Some(Skipped::SourceEmpty { .. })));
    }

    #[test]
    fn prefix_and_suffix_wrap_the_existing_value() {
        let (p, _) = run(
            Recipe::PrefixField {
                field: "title".into(),
                text: ">> ".into(),
            },
            &[("title", "T")],
        );
        assert_eq!(p.get("title"), Some(">> T"));

        let (s, _) = run(
            Recipe::SuffixField {
                field: "title".into(),
                text: " (Live)".into(),
            },
            &[("title", "T")],
        );
        assert_eq!(s.get("title"), Some("T (Live)"));
    }

    #[test]
    fn swap_exchanges_two_fields() {
        let (out, _) = run(
            Recipe::SwapFields {
                first: "artist".into(),
                second: "title".into(),
            },
            &[("artist", "A"), ("title", "T")],
        );
        assert_eq!(out.get("artist"), Some("T"));
        assert_eq!(out.get("title"), Some("A"));
    }

    #[test]
    fn swap_with_one_side_empty_moves_the_emptiness_too() {
        // Otherwise a swap would duplicate the non-empty value into both.
        let (out, _) = run(
            Recipe::SwapFields {
                first: "artist".into(),
                second: "remixer".into(),
            },
            &[("artist", "A")],
        );
        assert!(out.get("artist").is_none());
        assert_eq!(out.get("remixer"), Some("A"));
    }

    #[test]
    fn split_divides_on_the_delimiter() {
        // The manual's worked example.
        let (out, _) = run(
            Recipe::SplitField {
                field: "title".into(),
                delimiter: " - ".into(),
                first_target: "title".into(),
                second_target: "artist".into(),
                preserve_split_text: false,
                append: false,
            },
            &[("title", "Get Lucky - Daft Punk")],
        );
        assert_eq!(out.get("title"), Some("Get Lucky"));
        assert_eq!(out.get("artist"), Some("Daft Punk"));
    }

    #[test]
    fn split_can_preserve_the_delimiter_on_the_first_part() {
        let (out, _) = run(
            Recipe::SplitField {
                field: "title".into(),
                delimiter: ":".into(),
                first_target: "a".into(),
                second_target: "b".into(),
                preserve_split_text: true,
                append: false,
            },
            &[("title", "Vol:1")],
        );
        assert_eq!(out.get("a"), Some("Vol:"));
        assert_eq!(out.get("b"), Some("1"));
    }

    #[test]
    fn split_can_append_rather_than_overwrite() {
        let (out, _) = run(
            Recipe::SplitField {
                field: "title".into(),
                delimiter: " - ".into(),
                first_target: "comment".into(),
                second_target: "grouping".into(),
                preserve_split_text: false,
                append: true,
            },
            &[("title", "A - B"), ("comment", "existing")],
        );
        assert_eq!(out.get("comment"), Some("existing A"));
        assert_eq!(out.get("grouping"), Some("B"));
    }

    #[test]
    fn split_on_a_missing_delimiter_is_a_no_match_not_a_wrong_split() {
        let (out, outcome) = run(
            Recipe::SplitField {
                field: "title".into(),
                delimiter: " - ".into(),
                first_target: "a".into(),
                second_target: "b".into(),
                preserve_split_text: false,
                append: false,
            },
            &[("title", "No delimiter here")],
        );
        assert_eq!(
            outcome.skipped,
            Some(Skipped::NoMatch {
                field: "title".into()
            })
        );
        assert_eq!(out.get("title"), Some("No delimiter here"));
    }

    #[test]
    fn split_with_an_empty_delimiter_is_refused_as_misconfigured() {
        let (_, outcome) = run(
            Recipe::SplitField {
                field: "title".into(),
                delimiter: "".into(),
                first_target: "a".into(),
                second_target: "b".into(),
                preserve_split_text: false,
                append: false,
            },
            &[("title", "T")],
        );
        assert!(matches!(
            outcome.skipped,
            Some(Skipped::Misconfigured { .. })
        ));
    }

    // ── Text ────────────────────────────────────────────────────────────────

    #[test]
    fn extract_writes_to_the_target_and_can_clean_the_source() {
        let (out, _) = run(
            Recipe::ExtractText {
                field: "title".into(),
                start: "(".into(),
                end: ")".into(),
                target: "remixer".into(),
                include_delimiters: false,
                delete_from_source: true,
                append: false,
            },
            &[("title", "Get Lucky (Daft Punk Remix)")],
        );
        assert_eq!(out.get("remixer"), Some("Daft Punk Remix"));
        assert_eq!(out.get("title"), Some("Get Lucky"));
    }

    #[test]
    fn extract_with_no_match_leaves_the_target_alone() {
        // Writing an empty string would blank a good remixer field.
        let (out, outcome) = run(
            Recipe::ExtractText {
                field: "title".into(),
                start: "(".into(),
                end: ")".into(),
                target: "remixer".into(),
                include_delimiters: false,
                delete_from_source: true,
                append: false,
            },
            &[("title", "Get Lucky"), ("remixer", "keep me")],
        );
        assert_eq!(
            outcome.skipped,
            Some(Skipped::NoMatch {
                field: "title".into()
            })
        );
        assert_eq!(out.get("remixer"), Some("keep me"));
    }

    #[test]
    fn remove_between_and_special_characters_run_through_the_dispatcher() {
        let (out, _) = run(
            Recipe::RemoveBetween {
                field: "title".into(),
                pair: DelimiterPair::Parentheses,
            },
            &[("title", "Track (Original Mix)")],
        );
        assert_eq!(out.get("title"), Some("Track"));

        let (out, _) = run(
            Recipe::RemoveSpecialCharacters {
                field: "artist".into(),
                mode: SpecialCharacterMode::Special,
            },
            &[("artist", "Émile")],
        );
        assert_eq!(out.get("artist"), Some("Emile"));
    }

    // ── Number ──────────────────────────────────────────────────────────────

    #[test]
    fn adjusting_a_number_keeps_it_looking_like_an_integer() {
        let (out, _) = run(
            Recipe::AdjustNumber {
                field: "year".into(),
                amount: 1.0,
            },
            &[("year", "2013")],
        );
        assert_eq!(out.get("year"), Some("2014"));
    }

    #[test]
    fn a_negative_amount_decreases() {
        let (out, _) = run(
            Recipe::AdjustNumber {
                field: "rating".into(),
                amount: -2.0,
            },
            &[("rating", "5")],
        );
        assert_eq!(out.get("rating"), Some("3"));
    }

    #[test]
    fn a_fractional_field_stays_fractional() {
        let (out, _) = run(
            Recipe::AdjustNumber {
                field: "bpm".into(),
                amount: 0.5,
            },
            &[("bpm", "128.0")],
        );
        assert_eq!(out.get("bpm"), Some("128.5"));
    }

    #[test]
    fn a_non_numeric_field_is_reported_rather_than_zeroed() {
        let (out, outcome) = run(
            Recipe::AdjustNumber {
                field: "title".into(),
                amount: 1.0,
            },
            &[("title", "Get Lucky")],
        );
        assert_eq!(
            outcome.skipped,
            Some(Skipped::NotANumber {
                field: "title".into(),
                value: "Get Lucky".into()
            })
        );
        assert_eq!(out.get("title"), Some("Get Lucky"));
    }

    // ── Chaining ────────────────────────────────────────────────────────────

    #[test]
    fn recipes_run_in_the_order_given_and_thread_their_results() {
        let (out, outcomes) = apply_all(
            &[
                Recipe::ExtractText {
                    field: "title".into(),
                    start: "(".into(),
                    end: ")".into(),
                    target: "remixer".into(),
                    include_delimiters: false,
                    delete_from_source: true,
                    append: false,
                },
                Recipe::ToTitleCase {
                    field: "title".into(),
                    ignore_words: vec![],
                },
            ],
            &fields(&[("title", "get lucky (SOME remix)")]),
        );
        assert_eq!(out.get("remixer"), Some("SOME remix"));
        assert_eq!(out.get("title"), Some("Get Lucky"));
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.changed()));
    }

    #[test]
    fn a_skipped_step_does_not_stop_the_ones_after_it() {
        let (out, outcomes) = apply_all(
            &[
                Recipe::ToUpperCase {
                    field: "missing".into(),
                    ignore_words: vec![],
                },
                Recipe::ToUpperCase {
                    field: "title".into(),
                    ignore_words: vec![],
                },
            ],
            &fields(&[("title", "t")]),
        );
        assert!(outcomes[0].skipped.is_some());
        assert_eq!(out.get("title"), Some("T"));
    }

    #[test]
    fn an_empty_recipe_list_changes_nothing() {
        let input = fields(&[("title", "T")]);
        let (out, outcomes) = apply_all(&[], &input);
        assert_eq!(out, input);
        assert!(outcomes.is_empty());
    }

    #[test]
    fn recipes_round_trip_through_json_so_they_can_be_saved_and_replayed() {
        let recipes = vec![
            Recipe::ToTitleCase {
                field: "title".into(),
                ignore_words: vec!["EDM".into()],
            },
            Recipe::SplitField {
                field: "title".into(),
                delimiter: " - ".into(),
                first_target: "title".into(),
                second_target: "artist".into(),
                preserve_split_text: false,
                append: false,
            },
            Recipe::RemoveSpecialCharacters {
                field: "artist".into(),
                mode: SpecialCharacterMode::Emojis,
            },
            Recipe::AdjustNumber {
                field: "year".into(),
                amount: -1.0,
            },
        ];
        let json = serde_json::to_string(&recipes).unwrap();
        assert_eq!(serde_json::from_str::<Vec<Recipe>>(&json).unwrap(), recipes);
    }
}
