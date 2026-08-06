//! Manual editing across a multi-track selection.
//!
//! The whole feature turns on one rule: **a field the user did not touch is not
//! written.** Open an editor on forty tracks, change the genre, press Save —
//! and the other nine fields must come out exactly as they went in, even though
//! the form had to show *something* in each of them. Get this wrong and the
//! editor silently flattens a library to whatever the first track happened to
//! hold.
//!
//! So the form's state is not "the values"; it is "the values, plus which ones
//! were edited". [`FieldValue::Multiple`] is what a field shows when the
//! selection disagrees, and it is a value the caller can never accidentally
//! write, because it is not a value at all.
//!
//! Pure: nothing here opens a database. See `docs/lexicon/02-library.md
//! §Manual Editing`.

use serde::{Deserialize, Serialize};

/// What a field shows across a selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum FieldValue {
    /// Every selected track agrees. `None` means they all agree it is empty.
    Same(Option<String>),
    /// They disagree — the editor shows `<multiple values>`.
    Multiple,
}

/// One track's current values, keyed by field name.
pub type TrackValues = std::collections::BTreeMap<String, String>;

/// A track in the selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditableTrack {
    pub id: String,
    pub title: String,
    pub values: TrackValues,
}

/// What the editor should show for one field.
///
/// A missing value and an empty string are the same thing here — a form cannot
/// distinguish them, and pretending otherwise would make "clear this field"
/// behave differently depending on how the field became empty.
pub fn collapse(tracks: &[EditableTrack], field: &str) -> FieldValue {
    let mut iter = tracks
        .iter()
        .map(|t| t.values.get(field).filter(|v| !v.is_empty()));
    let Some(first) = iter.next() else {
        // No selection: nothing to disagree about.
        return FieldValue::Same(None);
    };
    if iter.all(|v| v == first) {
        FieldValue::Same(first.cloned())
    } else {
        FieldValue::Multiple
    }
}

/// The form's initial state: every field collapsed across the selection.
pub fn initial(tracks: &[EditableTrack], fields: &[String]) -> Vec<(String, FieldValue)> {
    fields
        .iter()
        .map(|f| (f.clone(), collapse(tracks, f)))
        .collect()
}

/// A field the user actually changed.
///
/// Only edited fields exist here. There is deliberately no way to express
/// "write `<multiple values>`" — the type is the guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    pub field: String,
    /// `None` clears the field, which is a real edit, not an absence of one.
    pub value: Option<String>,
}

/// One write, ready to stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedEdit {
    pub track_id: String,
    pub track_title: String,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Work out what the edits would change, per track.
///
/// A track already holding the value produces nothing — the point of an editor
/// over forty tracks is usually that most of them are already right, and
/// staging forty no-op changes would bury the two that matter.
pub fn plan(tracks: &[EditableTrack], edits: &[Edit]) -> Vec<PlannedEdit> {
    let mut out = Vec::new();
    for track in tracks {
        for edit in edits {
            let before = track.values.get(&edit.field).filter(|v| !v.is_empty());
            let after = edit.value.as_ref().filter(|v| !v.is_empty());
            if before == after {
                continue;
            }
            out.push(PlannedEdit {
                track_id: track.id.clone(),
                track_title: track.title.clone(),
                field: edit.field.clone(),
                before: before.cloned(),
                after: after.cloned(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, pairs: &[(&str, &str)]) -> EditableTrack {
        EditableTrack {
            id: id.into(),
            title: format!("track {id}"),
            values: pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    // ── collapse ────────────────────────────────────────────────────────────

    #[test]
    fn one_track_shows_its_own_value() {
        let tracks = [track("1", &[("genre", "House")])];
        assert_eq!(
            collapse(&tracks, "genre"),
            FieldValue::Same(Some("House".into()))
        );
    }

    #[test]
    fn agreement_across_the_selection_shows_the_shared_value() {
        let tracks = [
            track("1", &[("genre", "House")]),
            track("2", &[("genre", "House")]),
        ];
        assert_eq!(
            collapse(&tracks, "genre"),
            FieldValue::Same(Some("House".into()))
        );
    }

    #[test]
    fn disagreement_shows_multiple_values() {
        let tracks = [
            track("1", &[("genre", "House")]),
            track("2", &[("genre", "Techno")]),
        ];
        assert_eq!(collapse(&tracks, "genre"), FieldValue::Multiple);
    }

    #[test]
    fn a_field_nobody_has_is_agreed_to_be_empty() {
        let tracks = [track("1", &[]), track("2", &[])];
        assert_eq!(collapse(&tracks, "genre"), FieldValue::Same(None));
    }

    #[test]
    fn one_track_missing_the_field_is_a_disagreement() {
        // The editor must not show "House" when half the selection is empty —
        // pressing Save would then be indistinguishable from doing nothing.
        let tracks = [track("1", &[("genre", "House")]), track("2", &[])];
        assert_eq!(collapse(&tracks, "genre"), FieldValue::Multiple);
    }

    #[test]
    fn an_empty_string_and_a_missing_value_are_the_same_field_state() {
        // A form cannot tell them apart, and "clear this" must not behave
        // differently depending on how the field became empty.
        let tracks = [track("1", &[("genre", "")]), track("2", &[])];
        assert_eq!(collapse(&tracks, "genre"), FieldValue::Same(None));
    }

    #[test]
    fn an_empty_selection_has_nothing_to_disagree_about() {
        assert_eq!(collapse(&[], "genre"), FieldValue::Same(None));
    }

    #[test]
    fn the_initial_form_carries_every_field_asked_for() {
        let tracks = [track("1", &[("genre", "House")])];
        let fields = vec!["genre".to_string(), "album".to_string()];
        let got = initial(&tracks, &fields);
        assert_eq!(got.len(), 2);
        assert_eq!(got[1], ("album".into(), FieldValue::Same(None)));
    }

    // ── plan ────────────────────────────────────────────────────────────────

    #[test]
    fn an_edit_is_written_to_every_selected_track() {
        let tracks = [
            track("1", &[("genre", "House")]),
            track("2", &[("genre", "Techno")]),
        ];
        let edits = [Edit {
            field: "genre".into(),
            value: Some("Disco".into()),
        }];
        let got = plan(&tracks, &edits);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].after.as_deref(), Some("Disco"));
        assert_eq!(got[1].before.as_deref(), Some("Techno"));
    }

    #[test]
    fn untouched_fields_are_never_written() {
        // The rule the whole feature turns on: opening the editor on a mixed
        // selection and pressing Save must not flatten anything.
        let tracks = [
            track("1", &[("genre", "House"), ("album", "A")]),
            track("2", &[("genre", "Techno"), ("album", "B")]),
        ];
        let edits = [Edit {
            field: "genre".into(),
            value: Some("Disco".into()),
        }];
        let got = plan(&tracks, &edits);
        assert!(got.iter().all(|e| e.field == "genre"));
    }

    #[test]
    fn a_track_already_holding_the_value_produces_nothing() {
        // Most of a forty-track selection is usually already right; staging
        // forty no-ops would bury the two that matter.
        let tracks = [
            track("1", &[("genre", "Disco")]),
            track("2", &[("genre", "Techno")]),
        ];
        let edits = [Edit {
            field: "genre".into(),
            value: Some("Disco".into()),
        }];
        let got = plan(&tracks, &edits);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].track_id, "2");
    }

    #[test]
    fn clearing_a_field_is_a_real_edit() {
        let tracks = [track("1", &[("genre", "House")])];
        let edits = [Edit {
            field: "genre".into(),
            value: None,
        }];
        let got = plan(&tracks, &edits);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].after, None);
    }

    #[test]
    fn clearing_an_already_empty_field_produces_nothing() {
        let tracks = [track("1", &[])];
        let edits = [Edit {
            field: "genre".into(),
            value: None,
        }];
        assert!(plan(&tracks, &edits).is_empty());
    }

    #[test]
    fn setting_a_field_to_whitespace_is_not_the_same_as_clearing_it() {
        // Whitespace is a value the user typed; only an empty value clears.
        let tracks = [track("1", &[("genre", "House")])];
        let edits = [Edit {
            field: "genre".into(),
            value: Some(" ".into()),
        }];
        assert_eq!(plan(&tracks, &edits)[0].after.as_deref(), Some(" "));
    }

    #[test]
    fn several_edits_apply_to_the_same_track_independently() {
        let tracks = [track("1", &[("genre", "House"), ("album", "A")])];
        let edits = [
            Edit {
                field: "genre".into(),
                value: Some("Disco".into()),
            },
            Edit {
                field: "album".into(),
                value: Some("B".into()),
            },
        ];
        assert_eq!(plan(&tracks, &edits).len(), 2);
    }

    #[test]
    fn no_edits_means_no_changes_however_large_the_selection() {
        let tracks: Vec<_> = (0..50)
            .map(|i| track(&i.to_string(), &[("genre", "House")]))
            .collect();
        assert!(plan(&tracks, &[]).is_empty());
    }

    #[test]
    fn field_values_round_trip_through_json() {
        let values = vec![
            FieldValue::Same(Some("House".into())),
            FieldValue::Same(None),
            FieldValue::Multiple,
        ];
        let json = serde_json::to_string(&values).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<FieldValue>>(&json).unwrap(),
            values
        );
    }
}
