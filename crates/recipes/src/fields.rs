//! The field vocabulary recipes operate on, and the per-track values they edit.
//!
//! Recipes address fields by name rather than by a typed enum. That is a
//! deliberate concession to how the feature is used: the vocabulary is the
//! user-facing Lexicon field list, it grows as `decks` models more of it, and a
//! recipe stored last month must still deserialise after a field is added. A
//! name that no longer exists reads as absent, which is exactly how a track
//! that simply has no value for it behaves.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A track's editable values, keyed by field name.
///
/// `BTreeMap` rather than `HashMap` so the changed-field report has a stable
/// order — a preview screen that reshuffles its rows between runs is
/// unreviewable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackFields(BTreeMap<String, String>);

impl TrackFields {
    pub fn new() -> Self {
        Self::default()
    }

    /// A field's value, or `None` when it is absent *or* blank.
    ///
    /// Recipes treat "" and "   " as absent throughout: a whitespace-only
    /// artist is not an artist, and every operation that asks "does this field
    /// have a value" should agree.
    pub fn get(&self, field: &str) -> Option<&str> {
        self.0
            .get(field)
            .map(|s| s.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// The raw stored value, untrimmed — for operations that must preserve
    /// exactly what is there.
    pub fn raw(&self, field: &str) -> Option<&str> {
        self.0.get(field).map(String::as_str)
    }

    /// Set a field. An empty or whitespace-only value clears it, so a recipe
    /// cannot leave a field holding `"  "`.
    pub fn set(&mut self, field: &str, value: impl Into<String>) {
        let value = value.into();
        if value.trim().is_empty() {
            self.0.remove(field);
        } else {
            self.0.insert(field.to_string(), value);
        }
    }

    pub fn clear(&mut self, field: &str) {
        self.0.remove(field);
    }

    pub fn contains(&self, field: &str) -> bool {
        self.get(field).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for TrackFields {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut out = TrackFields::new();
        for (k, v) in iter {
            out.set(&k.into(), v);
        }
        out
    }
}

/// What one recipe changed on one track.
///
/// Reported rather than applied blind, because the repo's editing flows are
/// preview-then-accept throughout — `smart_fix_preview` / `smart_fix_apply`
/// set that precedent and recipes follow it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    /// `None` when the field had no value before.
    pub before: Option<String>,
    /// `None` when the recipe cleared it.
    pub after: Option<String>,
}

/// Compare two field sets and describe what moved.
pub fn diff(before: &TrackFields, after: &TrackFields) -> Vec<FieldChange> {
    let mut names: Vec<&str> = before.0.keys().map(String::as_str).collect();
    for name in after.0.keys() {
        if !names.contains(&name.as_str()) {
            names.push(name);
        }
    }
    names.sort_unstable();

    names
        .into_iter()
        .filter_map(|field| {
            let b = before.raw(field);
            let a = after.raw(field);
            if a == b {
                return None;
            }
            Some(FieldChange {
                field: field.to_string(),
                before: b.map(String::from),
                after: a.map(String::from),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> TrackFields {
        pairs.iter().copied().collect()
    }

    #[test]
    fn a_blank_value_reads_as_absent() {
        let f = fields(&[("artist", "   "), ("title", "A")]);
        assert!(f.get("artist").is_none());
        assert!(!f.contains("artist"));
        assert_eq!(f.get("title"), Some("A"));
    }

    #[test]
    fn values_are_trimmed_on_read_but_stored_verbatim() {
        let mut f = TrackFields::new();
        f.set("title", "  Get Lucky  ");
        assert_eq!(f.get("title"), Some("Get Lucky"));
        assert_eq!(f.raw("title"), Some("  Get Lucky  "));
    }

    #[test]
    fn setting_a_blank_value_clears_the_field() {
        // Otherwise a recipe could leave a field holding "  ", which reads as
        // present to anything using `raw`.
        let mut f = fields(&[("artist", "A")]);
        f.set("artist", "   ");
        assert!(f.raw("artist").is_none());
    }

    #[test]
    fn an_unknown_field_is_simply_absent() {
        let f = fields(&[("title", "A")]);
        assert!(f.get("nonexistent_field").is_none());
    }

    #[test]
    fn a_diff_reports_only_what_moved_in_stable_order() {
        let before = fields(&[("artist", "A"), ("title", "T"), ("genre", "G")]);
        let mut after = before.clone();
        after.set("title", "T2");
        after.clear("genre");

        let changes = diff(&before, &after);
        assert_eq!(
            changes.iter().map(|c| c.field.as_str()).collect::<Vec<_>>(),
            vec!["genre", "title"]
        );
        assert_eq!(changes[0].after, None);
        assert_eq!(changes[1].after.as_deref(), Some("T2"));
    }

    #[test]
    fn a_diff_reports_a_newly_added_field() {
        let before = fields(&[("title", "T")]);
        let mut after = before.clone();
        after.set("genre", "House");
        let changes = diff(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].before, None);
        assert_eq!(changes[0].after.as_deref(), Some("House"));
    }

    #[test]
    fn an_unchanged_track_produces_no_diff() {
        let f = fields(&[("title", "T")]);
        assert!(diff(&f, &f.clone()).is_empty());
    }

    #[test]
    fn field_sets_round_trip_through_json() {
        let f = fields(&[("title", "T"), ("artist", "A")]);
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(serde_json::from_str::<TrackFields>(&json).unwrap(), f);
    }
}
