//! The Smartlist Generator — bulk-creates smartlists so a large library gets a
//! navigable structure without hand-building dozens of rule sets.
//!
//! Output lands in a reserved folder (`LEXICON_FOLDER`). Re-running is
//! idempotent as long as generated smartlists stay in that folder: the folder
//! *is* the generation ledger, so no extra bookkeeping state is needed. Move a
//! generated smartlist out and the next run recreates it, which is the
//! documented escape hatch for customising one.

use std::collections::BTreeSet;

use rekordbox_db::Track;
use serde::{Deserialize, Serialize};

use crate::model::{Clause, Combinator, Field, Operator, Rule, Smartlist, Value};

/// The reserved playlist folder generated smartlists live in.
pub const LEXICON_FOLDER: &str = "Lexicon";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeneratorSpec {
    /// One smartlist per distinct value of a text field (genre, artist, …).
    ByField { field: Field },
    /// One smartlist per tag in a category.
    ByTagCategory {
        category_id: String,
        category_name: String,
        /// (tag id, tag name) pairs, supplied by the caller from the cache.
        tags: Vec<(String, String)>,
    },
    /// One smartlist per decade present in the library.
    ByDecade,
    /// Fixed-width BPM buckets.
    ByBpmRange { width: u32 },
    /// Played more than `threshold` times, and never played.
    ByPlayCount { threshold: i64 },
}

/// A smartlist the generator wants to exist. `id` is left to the caller, which
/// owns persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedSmartlist {
    pub name: String,
    pub combinator: Combinator,
    pub clauses: Vec<Clause>,
}

impl GeneratedSmartlist {
    fn single(name: String, rule: Rule) -> Self {
        Self {
            name,
            combinator: Combinator::All,
            clauses: vec![Clause::single(rule)],
        }
    }
}

/// Produce the smartlists a spec implies for the given library.
///
/// Pure — the caller diffs the result against what already exists in the
/// `Lexicon` folder and creates only the missing ones, which is what makes
/// re-running safe.
pub fn generate(spec: &GeneratorSpec, tracks: &[Track]) -> Vec<GeneratedSmartlist> {
    match spec {
        GeneratorSpec::ByField { field } => by_field(*field, tracks),
        GeneratorSpec::ByTagCategory {
            category_name,
            tags,
            ..
        } => tags
            .iter()
            .map(|(id, name)| {
                GeneratedSmartlist::single(
                    format!("{category_name}: {name}"),
                    Rule::new(Field::Tags, Operator::HasAll, Value::Tags(vec![id.clone()])),
                )
            })
            .collect(),
        GeneratorSpec::ByDecade => by_decade(tracks),
        GeneratorSpec::ByBpmRange { width } => by_bpm_range(*width, tracks),
        GeneratorSpec::ByPlayCount { threshold } => by_play_count(*threshold),
    }
}

fn by_field(field: Field, tracks: &[Track]) -> Vec<GeneratedSmartlist> {
    // BTreeSet gives deterministic, sorted output — important because the
    // caller diffs by name to stay idempotent.
    let mut values: BTreeSet<String> = BTreeSet::new();
    for t in tracks {
        let v = match field {
            Field::Genre => t.genre.as_deref(),
            Field::Artist => t.artist.as_deref(),
            Field::Album => t.album.as_deref(),
            Field::MusicalKey => t.musical_key.as_deref(),
            _ => None,
        };
        if let Some(v) = v {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                values.insert(trimmed.to_string());
            }
        }
    }
    values
        .into_iter()
        .map(|v| {
            GeneratedSmartlist::single(
                v.clone(),
                Rule::new(field, Operator::Equals, Value::Text(v)),
            )
        })
        .collect()
}

fn by_decade(tracks: &[Track]) -> Vec<GeneratedSmartlist> {
    let mut decades: BTreeSet<i64> = BTreeSet::new();
    for t in tracks {
        if let Some(y) = t.release_year {
            if y > 0 {
                decades.insert(y - y.rem_euclid(10));
            }
        }
    }
    decades
        .into_iter()
        .map(|start| {
            GeneratedSmartlist::single(
                format!("{start}s"),
                Rule::new(
                    Field::Year,
                    Operator::Between,
                    Value::Range(start as f64, (start + 9) as f64),
                ),
            )
        })
        .collect()
}

fn by_bpm_range(width: u32, tracks: &[Track]) -> Vec<GeneratedSmartlist> {
    if width == 0 {
        return Vec::new();
    }
    let w = width as f64;
    let mut buckets: BTreeSet<i64> = BTreeSet::new();
    for t in tracks {
        if let Some(bpm) = t.bpm {
            if bpm > 0.0 {
                buckets.insert((bpm / w).floor() as i64);
            }
        }
    }
    buckets
        .into_iter()
        .map(|b| {
            let lo = b as f64 * w;
            // Buckets are half-open in intent but `Between` is inclusive, so
            // stop just short of the next bucket's floor to avoid overlap.
            let hi = lo + w - 1.0;
            GeneratedSmartlist::single(
                format!("{}-{} BPM", lo as i64, hi as i64),
                Rule::new(Field::Bpm, Operator::Between, Value::Range(lo, hi)),
            )
        })
        .collect()
}

fn by_play_count(threshold: i64) -> Vec<GeneratedSmartlist> {
    vec![
        GeneratedSmartlist::single(
            format!("Played more than {threshold}×"),
            Rule::new(
                Field::PlayCount,
                Operator::GreaterThan,
                Value::Number(threshold as f64),
            ),
        ),
        GeneratedSmartlist::single(
            "Never played".to_string(),
            Rule::new(Field::PlayCount, Operator::LessOrEqual, Value::Number(0.0)),
        ),
    ]
}

/// Filter a generated set down to those that do not already exist, comparing by
/// name within the `Lexicon` folder. This is the idempotency guard.
pub fn only_missing(
    generated: Vec<GeneratedSmartlist>,
    existing: &[Smartlist],
) -> Vec<GeneratedSmartlist> {
    let present: BTreeSet<&str> = existing.iter().map(|s| s.name.as_str()).collect();
    generated
        .into_iter()
        .filter(|g| !present.contains(g.name.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, genre: Option<&str>, year: Option<i64>, bpm: Option<f64>) -> Track {
        Track {
            id: id.into(),
            title: "T".into(),
            artist: None,
            album: None,
            genre: genre.map(String::from),
            musical_key: None,
            bpm,
            duration_secs: None,
            rating: None,
            comment: None,
            folder_path: None,
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: year,
            dj_play_count: None,
            label: None,
            remixer: None,
            mix: None,
            color: None,
            date_added: None,
            energy: None,
        }
    }

    #[test]
    fn by_field_produces_one_per_distinct_value_sorted() {
        let tracks = vec![
            track("1", Some("Techno"), None, None),
            track("2", Some("House"), None, None),
            track("3", Some("House"), None, None),
            track("4", None, None, None),
            track("5", Some("  "), None, None),
        ];
        let out = generate(
            &GeneratorSpec::ByField {
                field: Field::Genre,
            },
            &tracks,
        );
        assert_eq!(
            out.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            vec!["House", "Techno"]
        );
    }

    #[test]
    fn by_decade_buckets_years() {
        let tracks = vec![
            track("1", None, Some(1994), None),
            track("2", None, Some(1999), None),
            track("3", None, Some(2003), None),
            track("4", None, Some(0), None),
            track("5", None, None, None),
        ];
        let out = generate(&GeneratorSpec::ByDecade, &tracks);
        assert_eq!(
            out.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            vec!["1990s", "2000s"]
        );
        assert_eq!(
            out[0].clauses[0].rules[0].value,
            Value::Range(1990.0, 1999.0)
        );
    }

    #[test]
    fn by_bpm_range_buckets_without_overlap() {
        let tracks = vec![
            track("1", None, None, Some(124.0)),
            track("2", None, None, Some(128.0)),
            track("3", None, None, Some(174.0)),
        ];
        let out = generate(&GeneratorSpec::ByBpmRange { width: 10 }, &tracks);
        assert_eq!(
            out.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            vec!["120-129 BPM", "170-179 BPM"]
        );
        assert_eq!(out[0].clauses[0].rules[0].value, Value::Range(120.0, 129.0));
    }

    #[test]
    fn by_bpm_range_rejects_zero_width() {
        let tracks = vec![track("1", None, None, Some(128.0))];
        assert!(generate(&GeneratorSpec::ByBpmRange { width: 0 }, &tracks).is_empty());
    }

    #[test]
    fn by_tag_category_names_include_the_category() {
        let spec = GeneratorSpec::ByTagCategory {
            category_id: "c1".into(),
            category_name: "Mood".into(),
            tags: vec![
                ("t1".into(), "Dark".into()),
                ("t2".into(), "Uplifting".into()),
            ],
        };
        let out = generate(&spec, &[]);
        assert_eq!(
            out.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            vec!["Mood: Dark", "Mood: Uplifting"]
        );
        assert_eq!(
            out[0].clauses[0].rules[0].value,
            Value::Tags(vec!["t1".into()])
        );
    }

    #[test]
    fn by_play_count_produces_two_lists() {
        let out = generate(&GeneratorSpec::ByPlayCount { threshold: 10 }, &[]);
        assert_eq!(
            out.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            vec!["Played more than 10×", "Never played"]
        );
    }

    #[test]
    fn only_missing_is_the_idempotency_guard() {
        let tracks = vec![
            track("1", Some("House"), None, None),
            track("2", Some("Techno"), None, None),
        ];
        let generated = generate(
            &GeneratorSpec::ByField {
                field: Field::Genre,
            },
            &tracks,
        );
        assert_eq!(generated.len(), 2);

        let existing = vec![Smartlist {
            id: "s1".into(),
            name: "House".into(),
            parent_folder_id: Some("lexicon".into()),
            combinator: Combinator::All,
            clauses: vec![],
            created_at: 0,
            updated_at: 0,
        }];
        let remaining = only_missing(generated, &existing);
        assert_eq!(
            remaining
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Techno"]
        );
    }

    #[test]
    fn rerunning_after_creating_everything_yields_nothing() {
        let tracks = vec![track("1", Some("House"), None, None)];
        let generated = generate(
            &GeneratorSpec::ByField {
                field: Field::Genre,
            },
            &tracks,
        );
        let existing: Vec<Smartlist> = generated
            .iter()
            .map(|g| Smartlist {
                id: g.name.clone(),
                name: g.name.clone(),
                parent_folder_id: Some("lexicon".into()),
                combinator: g.combinator,
                clauses: g.clauses.clone(),
                created_at: 0,
                updated_at: 0,
            })
            .collect();
        let again = generate(
            &GeneratorSpec::ByField {
                field: Field::Genre,
            },
            &tracks,
        );
        assert!(only_missing(again, &existing).is_empty());
    }
}
