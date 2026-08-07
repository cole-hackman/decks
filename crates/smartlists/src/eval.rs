//! Smartlist evaluation.
//!
//! Pure and in-memory: it takes the already-loaded track list plus a context of
//! pre-computed derived sets and returns matching track IDs. The app already
//! loads the full library to render the virtualized table and already builds
//! these sets for the filter drawer, so there is nothing to gain from a SQL
//! path at current library sizes — and half the interesting fields (energy,
//! tags, archived, cues, missing files) live in the local cache rather than
//! `master.db`, so a SQL path would fragment every query. See ADR-0013.

use std::collections::{HashMap, HashSet};

use rekordbox_db::Track;

use crate::key::canonical_key;
use crate::model::{Clause, Combinator, Field, Operator, Rule, Smartlist, Value};

/// Pre-computed cross-track data the evaluator cannot derive from a `Track`
/// alone. Mirrors the frontend `FilterContext` in
/// `apps/desktop/src/lib/filters.ts` so both sides agree on semantics.
#[derive(Debug, Default, Clone)]
pub struct EvalContext {
    pub tracks_with_cues: HashSet<String>,
    pub tracks_in_any_playlist: HashSet<String>,
    pub tracks_with_missing_files: HashSet<String>,
    pub archived_tracks: HashSet<String>,
    /// track id → tag ids bound to it.
    pub tags_by_track: HashMap<String, HashSet<String>>,
}

/// Evaluate a smartlist, returning the IDs of matching tracks in input order.
///
/// Archived tracks are excluded unless the smartlist explicitly mentions the
/// `IsArchived` field — Lexicon's documented default.
pub fn evaluate(list: &Smartlist, tracks: &[Track], ctx: &EvalContext) -> Vec<String> {
    let include_archived = list.mentions_archived();
    tracks
        .iter()
        .filter(|t| include_archived || !ctx.archived_tracks.contains(&t.id))
        .filter(|t| matches(list, t, ctx))
        .map(|t| t.id.clone())
        .collect()
}

/// Whether a single track satisfies the smartlist.
///
/// A smartlist with no clauses matches nothing rather than everything — an
/// empty rule set is a half-built smartlist, and silently returning the whole
/// library would be alarming.
pub fn matches(list: &Smartlist, track: &Track, ctx: &EvalContext) -> bool {
    if list.clauses.is_empty() {
        return false;
    }
    match list.combinator {
        // AND of clauses; each clause is an OR of its rules.
        Combinator::All => list.clauses.iter().all(|c| clause_matches(c, track, ctx)),
        // Flat union across every rule in every clause.
        Combinator::Any => list
            .clauses
            .iter()
            .flat_map(|c| c.rules.iter())
            .any(|r| rule_matches(r, track, ctx)),
    }
}

fn clause_matches(clause: &Clause, track: &Track, ctx: &EvalContext) -> bool {
    clause.rules.iter().any(|r| rule_matches(r, track, ctx))
}

fn rule_matches(rule: &Rule, track: &Track, ctx: &EvalContext) -> bool {
    match rule.field {
        Field::Tags => tag_rule_matches(rule, track, ctx),
        Field::HasCues => bool_rule_matches(rule, ctx.tracks_with_cues.contains(&track.id)),
        Field::InAnyPlaylist => {
            bool_rule_matches(rule, ctx.tracks_in_any_playlist.contains(&track.id))
        }
        Field::IsFileMissing => {
            bool_rule_matches(rule, ctx.tracks_with_missing_files.contains(&track.id))
        }
        Field::IsArchived => bool_rule_matches(rule, ctx.archived_tracks.contains(&track.id)),
        Field::MusicalKey => key_rule_matches(rule, track.musical_key.as_deref()),
        _ => match rule.field.kind() {
            crate::model::FieldKind::Text => text_rule_matches(rule, text_field(rule.field, track)),
            crate::model::FieldKind::Number => {
                number_rule_matches(rule, number_field(rule.field, track))
            }
            _ => false,
        },
    }
}

fn text_field(field: Field, t: &Track) -> Option<&str> {
    match field {
        Field::Title => Some(t.title.as_str()),
        Field::Artist => t.artist.as_deref(),
        Field::Album => t.album.as_deref(),
        Field::Genre => t.genre.as_deref(),
        Field::Comment => t.comment.as_deref(),
        Field::FilePath => t.folder_path.as_deref(),
        _ => None,
    }
}

fn number_field(field: Field, t: &Track) -> Option<f64> {
    match field {
        Field::Bpm => t.bpm,
        Field::Rating => t.rating.map(|v| v as f64),
        Field::Year => t.release_year.map(|v| v as f64),
        Field::DurationSecs => t.duration_secs.map(|v| v as f64),
        Field::BitRate => t.bit_rate.map(|v| v as f64),
        Field::SampleRate => t.sample_rate.map(|v| v as f64),
        Field::PlayCount => t.dj_play_count.map(|v| v as f64),
        Field::Energy => t.energy.map(|v| v as f64),
        _ => None,
    }
}

/// Empty and whitespace-only strings count as absent, matching
/// `isMissing` in the frontend filter module.
fn is_blank(v: Option<&str>) -> bool {
    match v {
        None => true,
        Some(s) => s.trim().is_empty(),
    }
}

fn text_rule_matches(rule: &Rule, value: Option<&str>) -> bool {
    match rule.op {
        Operator::IsNone => return is_blank(value),
        Operator::IsNotNone => return !is_blank(value),
        _ => {}
    }
    let Some(actual) = value else { return false };
    let Value::Text(expected) = &rule.value else {
        return false;
    };
    // Case- and accent-insensitive, per the Lexicon search contract. Accent
    // folding is deliberately limited to Latin-1 pairs; full Unicode
    // normalisation would need a dependency and has not been needed yet.
    let a = fold(actual);
    let b = fold(expected);
    match rule.op {
        Operator::Contains => a.contains(&b),
        Operator::NotContains => !a.contains(&b),
        Operator::Equals => a == b,
        Operator::NotEquals => a != b,
        _ => false,
    }
}

fn fold(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

fn number_rule_matches(rule: &Rule, value: Option<f64>) -> bool {
    match rule.op {
        Operator::IsNone => return value.is_none(),
        Operator::IsNotNone => return value.is_some(),
        _ => {}
    }
    let Some(actual) = value else { return false };
    match (&rule.op, &rule.value) {
        (Operator::Equals, Value::Number(n)) => (actual - n).abs() < f64::EPSILON,
        (Operator::NotEquals, Value::Number(n)) => (actual - n).abs() >= f64::EPSILON,
        (Operator::GreaterThan, Value::Number(n)) => actual > *n,
        (Operator::LessThan, Value::Number(n)) => actual < *n,
        (Operator::GreaterOrEqual, Value::Number(n)) => actual >= *n,
        (Operator::LessOrEqual, Value::Number(n)) => actual <= *n,
        (Operator::Between, Value::Range(lo, hi)) => actual >= *lo && actual <= *hi,
        _ => false,
    }
}

/// Key comparison canonicalises both sides first, so a rule written as `4m`,
/// `Am` or `8A` all match a track stored in any of those notations.
fn key_rule_matches(rule: &Rule, value: Option<&str>) -> bool {
    match rule.op {
        Operator::IsNone => return is_blank(value),
        Operator::IsNotNone => return !is_blank(value),
        _ => {}
    }
    let Value::Text(expected) = &rule.value else {
        return false;
    };
    let actual = value.and_then(canonical_key);
    let want = canonical_key(expected);
    match rule.op {
        // Unparseable input on either side falls back to a folded string
        // compare rather than silently matching nothing.
        Operator::Equals => match (&actual, &want) {
            (Some(a), Some(b)) => a == b,
            _ => value.map(fold) == Some(fold(expected)),
        },
        Operator::NotEquals => match (&actual, &want) {
            (Some(a), Some(b)) => a != b,
            _ => value.map(fold) != Some(fold(expected)),
        },
        _ => false,
    }
}

fn bool_rule_matches(rule: &Rule, actual: bool) -> bool {
    match rule.op {
        Operator::IsTrue => actual,
        Operator::IsFalse => !actual,
        _ => false,
    }
}

/// Tag matching is by exact tag identity. Lexicon matches full labels only,
/// never partial, as a deliberate performance decision — we match on IDs, which
/// is the same contract and cheaper.
fn tag_rule_matches(rule: &Rule, track: &Track, ctx: &EvalContext) -> bool {
    let Value::Tags(wanted) = &rule.value else {
        return false;
    };
    let empty = HashSet::new();
    let bound = ctx.tags_by_track.get(&track.id).unwrap_or(&empty);
    match rule.op {
        Operator::HasAll => wanted.iter().all(|t| bound.contains(t)),
        Operator::HasAny => wanted.iter().any(|t| bound.contains(t)),
        Operator::HasNone => !wanted.iter().any(|t| bound.contains(t)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Clause;

    fn track(id: &str) -> Track {
        Track {
            id: id.to_string(),
            title: "Song".into(),
            artist: Some("Artist".into()),
            album: None,
            genre: Some("House".into()),
            musical_key: Some("8A".into()),
            bpm: Some(128.0),
            duration_secs: Some(300),
            rating: Some(3),
            comment: None,
            folder_path: Some("/music/song.mp3".into()),
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: Some(2020),
            dj_play_count: Some(5),
            energy: Some(0.8),
        }
    }

    fn list(combinator: Combinator, clauses: Vec<Clause>) -> Smartlist {
        Smartlist {
            id: "s1".into(),
            name: "Test".into(),
            parent_folder_id: None,
            combinator,
            clauses,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn one(field: Field, op: Operator, value: Value) -> Smartlist {
        list(
            Combinator::All,
            vec![Clause::single(Rule::new(field, op, value))],
        )
    }

    #[test]
    fn text_contains_is_case_insensitive() {
        let t = track("t1");
        let ctx = EvalContext::default();
        assert!(matches(
            &one(
                Field::Artist,
                Operator::Contains,
                Value::Text("artist".into())
            ),
            &t,
            &ctx
        ));
        assert!(matches(
            &one(
                Field::Artist,
                Operator::Contains,
                Value::Text("ARTIST".into())
            ),
            &t,
            &ctx
        ));
    }

    #[test]
    fn text_matching_folds_accents() {
        let mut t = track("t1");
        t.artist = Some("Björk".into());
        let ctx = EvalContext::default();
        assert!(matches(
            &one(
                Field::Artist,
                Operator::Contains,
                Value::Text("bjork".into())
            ),
            &t,
            &ctx
        ));
    }

    #[test]
    fn is_none_treats_blank_as_absent() {
        let mut t = track("t1");
        t.album = Some("   ".into());
        let ctx = EvalContext::default();
        assert!(matches(
            &one(Field::Album, Operator::IsNone, Value::None),
            &t,
            &ctx
        ));
        t.album = Some("Real".into());
        assert!(!matches(
            &one(Field::Album, Operator::IsNone, Value::None),
            &t,
            &ctx
        ));
    }

    #[test]
    fn numeric_operators() {
        let t = track("t1");
        let ctx = EvalContext::default();
        let cases = [
            (Operator::GreaterThan, Value::Number(120.0), true),
            (Operator::GreaterThan, Value::Number(130.0), false),
            (Operator::LessThan, Value::Number(130.0), true),
            (Operator::GreaterOrEqual, Value::Number(128.0), true),
            (Operator::LessOrEqual, Value::Number(128.0), true),
            (Operator::Equals, Value::Number(128.0), true),
            (Operator::NotEquals, Value::Number(128.0), false),
            (Operator::Between, Value::Range(120.0, 130.0), true),
            (Operator::Between, Value::Range(129.0, 140.0), false),
        ];
        for (op, value, expected) in cases {
            assert_eq!(
                matches(&one(Field::Bpm, op, value.clone()), &t, &ctx),
                expected,
                "op {op:?} value {value:?}"
            );
        }
    }

    #[test]
    fn key_equality_is_notation_aware() {
        let t = track("t1"); // stored as Camelot 8A
        let ctx = EvalContext::default();
        for spelling in ["8A", "8a", "Am", "A minor", "8m"] {
            assert!(
                matches(
                    &one(
                        Field::MusicalKey,
                        Operator::Equals,
                        Value::Text(spelling.into())
                    ),
                    &t,
                    &ctx
                ),
                "spelling {spelling} should match 8A"
            );
        }
        assert!(!matches(
            &one(
                Field::MusicalKey,
                Operator::Equals,
                Value::Text("9A".into())
            ),
            &t,
            &ctx
        ));
    }

    #[test]
    fn all_mode_ands_clauses_and_ors_rules_within_a_clause() {
        let t = track("t1"); // genre House, rating 3
        let ctx = EvalContext::default();
        // (Genre = House OR Genre = Techno) AND (Rating = 3)
        let sl = list(
            Combinator::All,
            vec![
                Clause {
                    rules: vec![
                        Rule::new(Field::Genre, Operator::Equals, Value::Text("House".into())),
                        Rule::new(Field::Genre, Operator::Equals, Value::Text("Techno".into())),
                    ],
                },
                Clause::single(Rule::new(
                    Field::Rating,
                    Operator::Equals,
                    Value::Number(3.0),
                )),
            ],
        );
        assert!(matches(&sl, &t, &ctx));

        // Same rules but rating 4 → the second clause fails, so the whole fails.
        let mut t2 = track("t2");
        t2.rating = Some(4);
        assert!(!matches(&sl, &t2, &ctx));

        // Neither genre matches → first clause fails.
        let mut t3 = track("t3");
        t3.genre = Some("Drum & Bass".into());
        assert!(!matches(&sl, &t3, &ctx));
    }

    #[test]
    fn any_mode_is_a_flat_union() {
        let mut t = track("t1");
        t.genre = Some("Drum & Bass".into());
        t.rating = Some(3);
        let ctx = EvalContext::default();
        let sl = list(
            Combinator::Any,
            vec![
                Clause::single(Rule::new(
                    Field::Genre,
                    Operator::Equals,
                    Value::Text("House".into()),
                )),
                Clause::single(Rule::new(
                    Field::Rating,
                    Operator::Equals,
                    Value::Number(3.0),
                )),
            ],
        );
        // Genre fails but rating matches — Any means union.
        assert!(matches(&sl, &t, &ctx));
    }

    #[test]
    fn empty_smartlist_matches_nothing() {
        let t = track("t1");
        let ctx = EvalContext::default();
        assert!(!matches(&list(Combinator::All, vec![]), &t, &ctx));
        assert!(!matches(&list(Combinator::Any, vec![]), &t, &ctx));
    }

    #[test]
    fn derived_boolean_fields_read_from_context() {
        let t = track("t1");
        let mut ctx = EvalContext::default();
        assert!(matches(
            &one(Field::HasCues, Operator::IsFalse, Value::None),
            &t,
            &ctx
        ));
        ctx.tracks_with_cues.insert("t1".into());
        assert!(matches(
            &one(Field::HasCues, Operator::IsTrue, Value::None),
            &t,
            &ctx
        ));

        ctx.tracks_with_missing_files.insert("t1".into());
        assert!(matches(
            &one(Field::IsFileMissing, Operator::IsTrue, Value::None),
            &t,
            &ctx
        ));

        ctx.tracks_in_any_playlist.insert("t1".into());
        assert!(matches(
            &one(Field::InAnyPlaylist, Operator::IsTrue, Value::None),
            &t,
            &ctx
        ));
    }

    #[test]
    fn tag_operators() {
        let t = track("t1");
        let mut ctx = EvalContext::default();
        ctx.tags_by_track.insert(
            "t1".into(),
            HashSet::from(["a".to_string(), "b".to_string()]),
        );
        let all = |v: Vec<&str>| Value::Tags(v.into_iter().map(String::from).collect());

        assert!(matches(
            &one(Field::Tags, Operator::HasAll, all(vec!["a", "b"])),
            &t,
            &ctx
        ));
        assert!(!matches(
            &one(Field::Tags, Operator::HasAll, all(vec!["a", "c"])),
            &t,
            &ctx
        ));
        assert!(matches(
            &one(Field::Tags, Operator::HasAny, all(vec!["a", "c"])),
            &t,
            &ctx
        ));
        assert!(matches(
            &one(Field::Tags, Operator::HasNone, all(vec!["c"])),
            &t,
            &ctx
        ));
        assert!(!matches(
            &one(Field::Tags, Operator::HasNone, all(vec!["a"])),
            &t,
            &ctx
        ));
    }

    #[test]
    fn tag_rules_handle_untagged_tracks() {
        let t = track("t1");
        let ctx = EvalContext::default();
        let tags = Value::Tags(vec!["a".into()]);
        assert!(!matches(
            &one(Field::Tags, Operator::HasAll, tags.clone()),
            &t,
            &ctx
        ));
        assert!(!matches(
            &one(Field::Tags, Operator::HasAny, tags.clone()),
            &t,
            &ctx
        ));
        // A track with no tags trivially has none of them.
        assert!(matches(
            &one(Field::Tags, Operator::HasNone, tags),
            &t,
            &ctx
        ));
    }

    #[test]
    fn archived_tracks_are_excluded_by_default() {
        let tracks = vec![track("t1"), track("t2")];
        let mut ctx = EvalContext::default();
        ctx.archived_tracks.insert("t2".into());

        let sl = one(Field::Genre, Operator::Equals, Value::Text("House".into()));
        assert_eq!(evaluate(&sl, &tracks, &ctx), vec!["t1".to_string()]);
    }

    #[test]
    fn archived_tracks_are_included_when_a_rule_mentions_them() {
        let tracks = vec![track("t1"), track("t2")];
        let mut ctx = EvalContext::default();
        ctx.archived_tracks.insert("t2".into());

        let sl = one(Field::IsArchived, Operator::IsTrue, Value::None);
        assert_eq!(evaluate(&sl, &tracks, &ctx), vec!["t2".to_string()]);
    }

    #[test]
    fn evaluate_preserves_input_order() {
        let tracks = vec![track("a"), track("b"), track("c")];
        let ctx = EvalContext::default();
        let sl = one(Field::Genre, Operator::Equals, Value::Text("House".into()));
        assert_eq!(evaluate(&sl, &tracks, &ctx), vec!["a", "b", "c"]);
    }
}
