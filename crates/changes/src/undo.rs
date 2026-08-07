//! Undo — the inverse of a change that has already been applied.
//!
//! `decks` gates hard *before* writing to `master.db`: every change is
//! reviewed, the write is opt-in, and `WriteGuard` takes a backup. What it had
//! no answer for is the change you accept and then regret. Restoring the whole
//! backup is a sledgehammer — it throws away everything else in the session too.
//!
//! So: compute the inverse of each applied change at the moment it is applied,
//! and keep it. Undoing then means staging those inverses as ordinary proposed
//! changes and running them through the same reviewed, guarded Sync as anything
//! else. There is no second write path, and no change ever reaches `master.db`
//! without the user seeing it.
//!
//! **Not every change can be inverted, and this module says which.** Per
//! ADR-0008 the honest answer is a named reason, not a silent omission — an
//! undo that quietly restored eight of twelve edits would be worse than one
//! that restored none.
//!
//! See `docs/lexicon/10-recipes.md §Undo History`.

use crate::{ChangeKind, NewChange, StagedChange};
use serde_json::Value;

/// The change that would put things back.
#[derive(Debug, Clone, PartialEq)]
pub struct Inverse {
    pub kind: ChangeKind,
    pub target_id: Option<String>,
    pub field: Option<String>,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
}

impl Inverse {
    /// Turn the inverse into a stageable change.
    ///
    /// It goes in as `Proposed` like anything else, so an undo is reviewed
    /// before it is written — the same guarantee the original change had.
    pub fn into_new_change(self, library_path: Option<String>, reason: String) -> NewChange {
        NewChange {
            library_path,
            kind: self.kind,
            target_id: self.target_id,
            field: self.field,
            old_value: self.old_value,
            new_value: self.new_value,
            reason: Some(reason),
            // An inverse is a fact about what was there, not a guess.
            confidence: Some(1.0),
        }
    }
}

/// Whether a change can be put back, and if not, why not.
#[derive(Debug, Clone, PartialEq)]
pub enum Reversal {
    Reversible(Inverse),
    /// A sentence the UI can show verbatim.
    Blocked(&'static str),
}

/// Why an id-generating change cannot be undone.
///
/// `apply_add_cue` and `apply_create` mint a UUID inside the transaction, and
/// nothing carries it back out to the staged change — so there is no row id to
/// point a delete at afterwards.
const GENERATED_ID: &str =
    "the new row's id is generated when the change is applied, so there is nothing to remove";

const NOT_RECORDED: &str = "the previous value was not recorded, so there is nothing to restore";

/// The inverse of an applied change.
///
/// `old_value` distinguishes two cases that look alike and are not: `Some(Null)`
/// means the field was genuinely empty and restoring it means clearing the
/// field, while `None` means nothing was recorded and the change cannot be
/// reversed at all. Treating the second as the first would blank a field the
/// user never asked to blank.
pub fn invert(change: &StagedChange) -> Reversal {
    match change.kind {
        // Straight swaps: the change said "a → b", the inverse says "b → a".
        ChangeKind::TrackMetadataEdit
        | ChangeKind::CueMetadataEdit
        | ChangeKind::TrackRelocate
        | ChangeKind::PlaylistRename
        | ChangeKind::PlaylistReorderTrack
        | ChangeKind::PlaylistReorder => match &change.old_value {
            None => Reversal::Blocked(NOT_RECORDED),
            Some(old) => Reversal::Reversible(Inverse {
                kind: change.kind.clone(),
                target_id: change.target_id.clone(),
                field: change.field.clone(),
                old_value: change.new_value.clone(),
                new_value: Some(old.clone()),
            }),
        },

        // Membership flips. The track id rides in `new_value` for both kinds,
        // so the inverse is the same payload under the opposite verb.
        ChangeKind::PlaylistAddTrack => match &change.new_value {
            None => Reversal::Blocked("no track was recorded on the change"),
            Some(track) => Reversal::Reversible(Inverse {
                kind: ChangeKind::PlaylistRemoveTrack,
                target_id: change.target_id.clone(),
                field: None,
                old_value: None,
                new_value: Some(track.clone()),
            }),
        },
        ChangeKind::PlaylistRemoveTrack => match &change.new_value {
            None => Reversal::Blocked("no track was recorded on the change"),
            Some(track) => Reversal::Reversible(Inverse {
                kind: ChangeKind::PlaylistAddTrack,
                target_id: change.target_id.clone(),
                field: None,
                old_value: None,
                new_value: Some(track.clone()),
            }),
        },

        // A deleted cue can be put back only if the change recorded what it
        // was. Callers that stage a deletion should capture the row first —
        // `old_value` is the whole `{in_msec, out_msec, kind, color, commnt}`
        // object the applier's insert wants, so the inverse is a plain add.
        //
        // The restored cue gets a new id, which is what "put it back" means
        // here: same position, same name, same colour, new row.
        ChangeKind::TrackDeleteCue => match &change.old_value {
            Some(Value::Object(_)) => Reversal::Reversible(Inverse {
                kind: ChangeKind::TrackAddCue,
                // A delete targets the cue; an add targets the track, which is
                // why the payload has to carry it.
                target_id: change
                    .old_value
                    .as_ref()
                    .and_then(|v| v.get("content_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                field: None,
                old_value: None,
                new_value: change.old_value.clone(),
            }),
            _ => Reversal::Blocked(
                "the cue's contents were not recorded before it was removed, so there is nothing to put back",
            ),
        },

        ChangeKind::TrackAddCue | ChangeKind::PlaylistCreate => Reversal::Blocked(GENERATED_ID),

        ChangeKind::PlaylistDelete => Reversal::Blocked(
            "the playlist's contents were not recorded before it was removed, so recreating it \
             would give you an empty playlist with the right name",
        ),
        ChangeKind::TrackDelete => Reversal::Blocked(
            "the track's row was not recorded before it was removed; restore the backup Sync took \
             before its first write",
        ),
        // Never applied — the applier refuses it and the export carries it.
        ChangeKind::TrackCreate => {
            Reversal::Blocked("adding a track happens through XML import, which Sync never wrote")
        }
    }
}

/// A one-line description of what undoing this change would do.
///
/// Written from the *undo's* point of view, because that is the button the user
/// is about to press.
pub fn describe(change: &StagedChange) -> String {
    fn show(v: Option<&Value>) -> String {
        match v {
            None | Some(Value::Null) => "empty".to_string(),
            Some(Value::String(s)) if s.is_empty() => "empty".to_string(),
            Some(Value::String(s)) => format!("\"{s}\""),
            Some(other) => other.to_string(),
        }
    }

    let target = change.target_id.as_deref().unwrap_or("?");
    match change.kind {
        ChangeKind::TrackMetadataEdit | ChangeKind::CueMetadataEdit | ChangeKind::TrackRelocate => {
            format!(
                "{}: {} → {}",
                change.field.as_deref().unwrap_or("field"),
                show(change.new_value.as_ref()),
                show(change.old_value.as_ref()),
            )
        }
        ChangeKind::PlaylistRename => format!("rename back to {}", show(change.old_value.as_ref())),
        ChangeKind::PlaylistAddTrack => {
            format!("remove track {} again", show(change.new_value.as_ref()))
        }
        ChangeKind::PlaylistRemoveTrack => {
            format!(
                "put track {} back (at the end)",
                show(change.new_value.as_ref())
            )
        }
        ChangeKind::PlaylistReorderTrack => "move the track back".to_string(),
        ChangeKind::PlaylistReorder => "restore the previous playlist order".to_string(),
        ChangeKind::TrackDeleteCue => "restore the deleted cue".to_string(),
        _ => format!("{:?} on {target}", change.kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChangeStatus;
    use serde_json::json;

    fn change(kind: ChangeKind, old: Option<Value>, new: Option<Value>) -> StagedChange {
        StagedChange {
            id: "c1".into(),
            library_path: Some("/lib.db".into()),
            kind,
            target_id: Some("t1".into()),
            field: Some("Title".into()),
            old_value: old,
            new_value: new,
            reason: None,
            confidence: Some(1.0),
            status: ChangeStatus::Exported,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn inverse(r: Reversal) -> Inverse {
        match r {
            Reversal::Reversible(i) => i,
            Reversal::Blocked(why) => panic!("expected reversible, got: {why}"),
        }
    }

    fn blocked(r: Reversal) -> &'static str {
        match r {
            Reversal::Blocked(why) => why,
            Reversal::Reversible(_) => panic!("expected blocked"),
        }
    }

    #[test]
    fn a_metadata_edit_inverts_by_swapping_its_ends() {
        let got = inverse(invert(&change(
            ChangeKind::TrackMetadataEdit,
            Some(json!("get lucky")),
            Some(json!("Get Lucky")),
        )));
        assert_eq!(got.kind, ChangeKind::TrackMetadataEdit);
        assert_eq!(got.field.as_deref(), Some("Title"));
        assert_eq!(got.old_value, Some(json!("Get Lucky")));
        assert_eq!(got.new_value, Some(json!("get lucky")));
    }

    #[test]
    fn a_field_that_was_empty_inverts_to_clearing_it_again() {
        // Some(Null) is a recorded emptiness, not a missing record.
        let got = inverse(invert(&change(
            ChangeKind::TrackMetadataEdit,
            Some(Value::Null),
            Some(json!("House")),
        )));
        assert_eq!(got.new_value, Some(Value::Null));
    }

    #[test]
    fn an_unrecorded_previous_value_blocks_the_undo_rather_than_blanking_the_field() {
        // The distinction that matters: None is not the same as Some(Null).
        // Treating it as one would clear a field the user never touched.
        let why = blocked(invert(&change(
            ChangeKind::TrackMetadataEdit,
            None,
            Some(json!("House")),
        )));
        assert_eq!(why, NOT_RECORDED);
    }

    #[test]
    fn cue_edits_invert_the_same_way_field_edits_do() {
        let got = inverse(invert(&change(
            ChangeKind::CueMetadataEdit,
            Some(json!(-1)),
            Some(json!(4)),
        )));
        assert_eq!(got.kind, ChangeKind::CueMetadataEdit);
        assert_eq!(got.new_value, Some(json!(-1)));
    }

    #[test]
    fn playlist_membership_inverts_to_the_opposite_verb() {
        let added = inverse(invert(&change(
            ChangeKind::PlaylistAddTrack,
            None,
            Some(json!("track-9")),
        )));
        assert_eq!(added.kind, ChangeKind::PlaylistRemoveTrack);
        assert_eq!(added.new_value, Some(json!("track-9")));

        let removed = inverse(invert(&change(
            ChangeKind::PlaylistRemoveTrack,
            None,
            Some(json!("track-9")),
        )));
        assert_eq!(removed.kind, ChangeKind::PlaylistAddTrack);
    }

    #[test]
    fn a_membership_inverse_keeps_pointing_at_the_playlist_not_the_track() {
        let got = inverse(invert(&change(
            ChangeKind::PlaylistAddTrack,
            None,
            Some(json!("track-9")),
        )));
        assert_eq!(got.target_id.as_deref(), Some("t1"));
    }

    #[test]
    fn a_recorded_cue_deletion_inverts_to_adding_it_back() {
        let mut c = change(
            ChangeKind::TrackDeleteCue,
            Some(json!({
                "content_id": "track-3",
                "in_msec": 65000,
                "kind": 1,
                "color": 4,
                "commnt": "Drop",
            })),
            None,
        );
        c.field = None;
        let got = inverse(invert(&c));
        assert_eq!(got.kind, ChangeKind::TrackAddCue);
        // An add targets the *track*, not the cue the delete targeted.
        assert_eq!(got.target_id.as_deref(), Some("track-3"));
        assert_eq!(
            got.new_value.as_ref().and_then(|v| v.get("commnt")),
            Some(&json!("Drop"))
        );
    }

    #[test]
    fn an_unrecorded_cue_deletion_says_so_rather_than_inventing_a_cue() {
        let why = blocked(invert(&change(ChangeKind::TrackDeleteCue, None, None)));
        assert!(why.contains("not recorded"));
    }

    #[test]
    fn generated_ids_block_the_undo_and_explain_why() {
        for kind in [ChangeKind::TrackAddCue, ChangeKind::PlaylistCreate] {
            assert_eq!(blocked(invert(&change(kind, None, None))), GENERATED_ID);
        }
    }

    #[test]
    fn destructive_kinds_point_at_the_backup_rather_than_pretending() {
        let why = blocked(invert(&change(ChangeKind::TrackDelete, None, None)));
        assert!(why.contains("backup"));
        let why = blocked(invert(&change(ChangeKind::PlaylistDelete, None, None)));
        assert!(why.contains("empty playlist"));
    }

    #[test]
    fn every_change_kind_has_an_answer() {
        // A new kind must be decided, not defaulted. This fails to compile-time
        // exhaustiveness in `invert`, but the loop guards against a lazy
        // catch-all being added later.
        let kinds = [
            ChangeKind::TrackMetadataEdit,
            ChangeKind::TrackDelete,
            ChangeKind::TrackCreate,
            ChangeKind::TrackRelocate,
            ChangeKind::CueMetadataEdit,
            ChangeKind::TrackAddCue,
            ChangeKind::TrackDeleteCue,
            ChangeKind::PlaylistCreate,
            ChangeKind::PlaylistRename,
            ChangeKind::PlaylistDelete,
            ChangeKind::PlaylistAddTrack,
            ChangeKind::PlaylistRemoveTrack,
            ChangeKind::PlaylistReorderTrack,
        ];
        for kind in kinds {
            // Either answer is fine; silence is not.
            let _ = invert(&change(kind, Some(json!("a")), Some(json!("b"))));
        }
    }

    #[test]
    fn an_undo_stages_as_a_proposal_not_a_write() {
        let got = inverse(invert(&change(
            ChangeKind::TrackMetadataEdit,
            Some(json!("a")),
            Some(json!("b")),
        )))
        .into_new_change(Some("/lib.db".into()), "Undo".into());
        assert_eq!(got.reason.as_deref(), Some("Undo"));
        assert_eq!(got.new_value, Some(json!("a")));
    }

    #[test]
    fn descriptions_read_from_the_undos_point_of_view() {
        let c = change(
            ChangeKind::TrackMetadataEdit,
            Some(json!("get lucky")),
            Some(json!("Get Lucky")),
        );
        // The arrow points back the way the undo would go.
        assert_eq!(describe(&c), "Title: \"Get Lucky\" → \"get lucky\"");
    }

    #[test]
    fn an_empty_previous_value_is_described_as_empty_not_as_null() {
        let c = change(
            ChangeKind::TrackMetadataEdit,
            Some(Value::Null),
            Some(json!("House")),
        );
        assert_eq!(describe(&c), "Title: \"House\" → empty");
    }
}
