//! Turning smartlists into something a DJ app can hold.
//!
//! Rekordbox has no equivalent of most smartlist rules, so Lexicon's answer is
//! to **materialise**: write a plain playlist containing whatever currently
//! matches. The user keeps the right tracks even though the app loses the rule.
//! That is what `SyncOptions.all_smartlists_to_playlists` turns on.
//!
//! This module also owns the `Excluded From Sync` conventions, which are
//! deliberately name-based so they need no new UI.

use changes::{ChangeKind, NewChange};
use serde_json::json;

use crate::model::Smartlist;

/// Playlists, smartlists and folders whose name starts with this string are
/// skipped when syncing, and a custom tag with this exact name excludes an
/// individual track. Convention over configuration — no settings screen.
pub const EXCLUDED_FROM_SYNC: &str = "Excluded From Sync";

/// Whether a playlist/smartlist/folder name opts out of syncing.
///
/// Matching is case-insensitive and on a prefix, so `Excluded From Sync —
/// archive 2024` works, which is how people actually name things.
pub fn is_excluded_by_name(name: &str) -> bool {
    name.trim()
        .to_lowercase()
        .starts_with(&EXCLUDED_FROM_SYNC.to_lowercase())
}

/// Whether a tag name is the opt-out tag.
pub fn is_exclusion_tag(tag_name: &str) -> bool {
    tag_name.trim().eq_ignore_ascii_case(EXCLUDED_FROM_SYNC)
}

/// Staged changes that recreate a smartlist as a plain Rekordbox playlist.
///
/// `playlist_id` must be caller-supplied and stable between preview and apply —
/// the applier relies on `target_id` being the same in both passes.
///
/// Returns an empty vec when the smartlist opts out by name, so callers can
/// pass everything through this function without pre-filtering.
pub fn materialize_changes(
    library_path: &str,
    list: &Smartlist,
    playlist_id: &str,
    track_ids: &[String],
) -> Vec<NewChange> {
    if is_excluded_by_name(&list.name) {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(track_ids.len() + 1);
    out.push(NewChange {
        library_path: Some(library_path.to_string()),
        kind: ChangeKind::PlaylistCreate,
        target_id: Some(playlist_id.to_string()),
        field: None,
        old_value: None,
        new_value: Some(json!({ "name": list.name })),
        reason: Some(format!(
            "Materialised from smartlist \"{}\" — Rekordbox cannot express these rules",
            list.name
        )),
        confidence: None,
    });

    for (i, track_id) in track_ids.iter().enumerate() {
        out.push(NewChange {
            library_path: Some(library_path.to_string()),
            kind: ChangeKind::PlaylistAddTrack,
            target_id: Some(playlist_id.to_string()),
            field: None,
            old_value: None,
            new_value: Some(json!({ "content_id": track_id, "track_no": i as i64 + 1 })),
            reason: Some(format!("Member of smartlist \"{}\"", list.name)),
            confidence: None,
        });
    }
    out
}

/// How faithfully Rekordbox can represent a smartlist.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    /// Rekordbox can express the rules natively.
    Native,
    /// Rekordbox cannot; the smartlist will be flattened to a plain playlist.
    Materialised { reason: String },
}

/// Rekordbox 6/7 maps only tag rules onto MyTag smartlists, and even then only
/// two of them: `Has all these tags` → `contains` and `Has none of these tags`
/// → `does not contain`. It caps MyTag at **4 categories and 2 rules**.
/// Everything else has to be materialised.
///
/// `tag_category_count` is how many distinct tag categories the rules touch;
/// the caller knows the tag→category mapping, this crate does not.
pub fn rekordbox_compatibility(list: &Smartlist, tag_category_count: usize) -> Compatibility {
    use crate::model::{Field, Operator};

    let rules: Vec<_> = list.clauses.iter().flat_map(|c| c.rules.iter()).collect();

    if rules.is_empty() {
        return Compatibility::Materialised {
            reason: "Smartlist has no rules".into(),
        };
    }

    let non_tag = rules.iter().filter(|r| r.field != Field::Tags).count();
    if non_tag > 0 {
        return Compatibility::Materialised {
            reason: "Rekordbox only expresses tag (MyTag) rules; other fields are flattened".into(),
        };
    }

    let unsupported_op = rules
        .iter()
        .any(|r| !matches!(r.op, Operator::HasAll | Operator::HasNone));
    if unsupported_op {
        return Compatibility::Materialised {
            reason: "Rekordbox MyTag supports only 'has all' and 'has none' tag rules".into(),
        };
    }

    if rules.len() > 2 {
        return Compatibility::Materialised {
            reason: format!(
                "Rekordbox allows at most 2 MyTag rules; this smartlist has {}",
                rules.len()
            ),
        };
    }

    if tag_category_count > 4 {
        return Compatibility::Materialised {
            reason: format!(
                "Rekordbox allows at most 4 MyTag categories; this smartlist spans {tag_category_count}"
            ),
        };
    }

    Compatibility::Native
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Clause, Combinator, Field, Operator, Rule, Value};

    fn list_named(name: &str, clauses: Vec<Clause>) -> Smartlist {
        Smartlist {
            id: "s1".into(),
            name: name.into(),
            parent_folder_id: None,
            combinator: Combinator::All,
            clauses,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn tag_rule(op: Operator) -> Clause {
        Clause::single(Rule::new(Field::Tags, op, Value::Tags(vec!["t1".into()])))
    }

    #[test]
    fn exclusion_matches_prefix_case_insensitively() {
        assert!(is_excluded_by_name("Excluded From Sync"));
        assert!(is_excluded_by_name("excluded from sync"));
        assert!(is_excluded_by_name("Excluded From Sync — archive 2024"));
        assert!(is_excluded_by_name("  Excluded From Sync  "));
        assert!(!is_excluded_by_name("My Excluded From Sync list"));
        assert!(!is_excluded_by_name("Peak time"));
    }

    #[test]
    fn exclusion_tag_matches_exactly() {
        assert!(is_exclusion_tag("Excluded From Sync"));
        assert!(is_exclusion_tag("excluded from sync"));
        assert!(!is_exclusion_tag("Excluded From Sync 2"));
    }

    #[test]
    fn materialise_emits_create_then_adds_in_order() {
        let list = list_named("Peak time", vec![tag_rule(Operator::HasAll)]);
        let changes = materialize_changes(
            "/lib.db",
            &list,
            "pl-1",
            &["t1".to_string(), "t2".to_string()],
        );

        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].kind, ChangeKind::PlaylistCreate);
        assert_eq!(changes[0].target_id.as_deref(), Some("pl-1"));
        assert_eq!(changes[0].new_value.as_ref().unwrap()["name"], "Peak time");

        assert_eq!(changes[1].kind, ChangeKind::PlaylistAddTrack);
        assert_eq!(changes[1].new_value.as_ref().unwrap()["content_id"], "t1");
        assert_eq!(changes[1].new_value.as_ref().unwrap()["track_no"], 1);
        assert_eq!(changes[2].new_value.as_ref().unwrap()["content_id"], "t2");
        assert_eq!(changes[2].new_value.as_ref().unwrap()["track_no"], 2);
    }

    #[test]
    fn materialise_skips_excluded_smartlists() {
        let list = list_named(
            "Excluded From Sync — scratch",
            vec![tag_rule(Operator::HasAll)],
        );
        assert!(materialize_changes("/lib.db", &list, "pl-1", &["t1".to_string()]).is_empty());
    }

    #[test]
    fn materialise_an_empty_smartlist_still_creates_the_playlist() {
        // An empty result is meaningful — the playlist should exist and be empty
        // rather than silently not appear in the DJ app.
        let list = list_named("Nothing matches", vec![tag_rule(Operator::HasAll)]);
        let changes = materialize_changes("/lib.db", &list, "pl-1", &[]);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::PlaylistCreate);
    }

    #[test]
    fn compatibility_native_for_one_or_two_supported_tag_rules() {
        let list = list_named("Tagged", vec![tag_rule(Operator::HasAll)]);
        assert_eq!(rekordbox_compatibility(&list, 1), Compatibility::Native);

        let list = list_named(
            "Tagged",
            vec![tag_rule(Operator::HasAll), tag_rule(Operator::HasNone)],
        );
        assert_eq!(rekordbox_compatibility(&list, 2), Compatibility::Native);
    }

    #[test]
    fn compatibility_materialises_non_tag_rules() {
        let list = list_named(
            "BPM",
            vec![Clause::single(Rule::new(
                Field::Bpm,
                Operator::Between,
                Value::Range(120.0, 130.0),
            ))],
        );
        assert!(matches!(
            rekordbox_compatibility(&list, 0),
            Compatibility::Materialised { .. }
        ));
    }

    #[test]
    fn compatibility_materialises_unsupported_tag_operator() {
        let list = list_named("Any tag", vec![tag_rule(Operator::HasAny)]);
        assert!(matches!(
            rekordbox_compatibility(&list, 1),
            Compatibility::Materialised { .. }
        ));
    }

    #[test]
    fn compatibility_enforces_the_two_rule_cap() {
        let list = list_named(
            "Three",
            vec![
                tag_rule(Operator::HasAll),
                tag_rule(Operator::HasAll),
                tag_rule(Operator::HasNone),
            ],
        );
        assert!(matches!(
            rekordbox_compatibility(&list, 1),
            Compatibility::Materialised { .. }
        ));
    }

    #[test]
    fn compatibility_enforces_the_four_category_cap() {
        let list = list_named("Tagged", vec![tag_rule(Operator::HasAll)]);
        assert!(matches!(
            rekordbox_compatibility(&list, 5),
            Compatibility::Materialised { .. }
        ));
    }

    #[test]
    fn compatibility_materialises_an_empty_rule_set() {
        let list = list_named("Empty", vec![]);
        assert!(matches!(
            rekordbox_compatibility(&list, 0),
            Compatibility::Materialised { .. }
        ));
    }
}
