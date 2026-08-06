//! Translate `StagedChange` records into SQL mutations on `master.db`.
//!
//! All column names that appear in dynamically-built SQL must come from a
//! private allowlist inside each submodule. All values are bound parameters.

use crate::{ChangeKind, StagedChange};
use rusqlite::Transaction;

mod cues;
mod playlists;
mod tracks;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ApplyResult {
    pub applied: Vec<String>,
    pub failed: Vec<(String, String)>,
    /// Non-fatal warnings accumulated during apply (e.g. a key value that
    /// could not be converted to the requested Camelot/OpenKey format and was
    /// written as the original string). The change is still counted as
    /// `applied`; this lets the UI surface "N of M actually converted" without
    /// failing the whole sync.
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("sql error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("other: {0}")]
    Other(#[from] anyhow::Error),
}

/// User-controlled flags governing how staged changes are written to master.db.
///
/// Sourced from the SyncPanel option group: cue destination, beat-grid
/// preservation, key-notation conversion, and the two "auto" Cleanup toggles.
/// All-defaults yields the conservative, no-rewriting behavior — equivalent to
/// the pre-Sub-Plan-6 applier.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct SyncOptions {
    #[serde(default)]
    pub cue_destination: CueDestination,
    #[serde(default)]
    pub keep_grids: bool,
    #[serde(default)]
    pub convert_keys: KeyFormat,
    #[serde(default)]
    pub change_to_nearest_color: bool,
    #[serde(default)]
    pub all_smartlists_to_playlists: bool,
}

#[derive(Debug, Clone, Copy, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CueDestination {
    #[default]
    Hot,
    Memory,
    Both,
}

#[derive(Debug, Clone, Copy, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyFormat {
    #[default]
    Original,
    Camelot,
    OpenKey,
}

/// Backwards-compatible entry point — applies with default options.
pub fn apply(tx: &Transaction, changes: &[StagedChange]) -> Result<ApplyResult, ApplyError> {
    apply_with_options(tx, changes, &SyncOptions::default())
}

pub fn apply_with_options(
    tx: &Transaction,
    changes: &[StagedChange],
    options: &SyncOptions,
) -> Result<ApplyResult, ApplyError> {
    let mut applied = Vec::new();
    let mut failed = Vec::new();
    let mut warnings = Vec::new();
    for change in changes {
        // `keep_grids` short-circuit: skip BPM edits with a no-op success so
        // the change is still marked exported (it was a user-accepted change,
        // we just chose not to write it). The applied list will include it.
        if options.keep_grids && is_grid_change(change) {
            tracing::debug!(
                change_id = %change.id,
                "keep_grids=true: skipping beat-grid change"
            );
            applied.push(change.id.clone());
            continue;
        }
        match apply_single(tx, change, options, &mut warnings) {
            Ok(_) => applied.push(change.id.clone()),
            Err(e) => failed.push((change.id.clone(), e.to_string())),
        }
    }
    Ok(ApplyResult {
        applied,
        failed,
        warnings,
    })
}

fn is_grid_change(change: &StagedChange) -> bool {
    // BPM scalar edits — covered fully.
    if matches!(change.kind, ChangeKind::TrackMetadataEdit)
        && change.field.as_deref() == Some("BPM")
    {
        return true;
    }
    // Beat-grid CueMetadataEdit pass-through is not currently fully wired —
    // `djmdCue` rows distinguish hot/memory by `Kind` but the beat-grid lives
    // in ANLZ files (PQTZ/PQT2). Honest acknowledgement: we do not edit beat
    // grids today, so there is nothing to skip there.
    false
}

fn apply_single(
    tx: &Transaction,
    change: &StagedChange,
    options: &SyncOptions,
    warnings: &mut Vec<String>,
) -> anyhow::Result<()> {
    match change.kind {
        ChangeKind::TrackMetadataEdit => tracks::apply_metadata_edit(tx, change, options, warnings),
        ChangeKind::TrackDelete => tracks::apply_delete(tx, change),
        ChangeKind::TrackAddCue => cues::apply_add_cue(tx, change, options),
        ChangeKind::CueMetadataEdit => cues::apply_metadata_edit(tx, change),
        ChangeKind::TrackDeleteCue => cues::apply_delete_cue(tx, change),
        ChangeKind::PlaylistCreate => playlists::apply_create(tx, change),
        ChangeKind::PlaylistRename => playlists::apply_rename(tx, change),
        ChangeKind::PlaylistDelete => playlists::apply_delete(tx, change),
        ChangeKind::PlaylistAddTrack => playlists::apply_add_track(tx, change),
        ChangeKind::PlaylistRemoveTrack => playlists::apply_remove_track(tx, change),
        ChangeKind::PlaylistReorderTrack => playlists::apply_reorder(tx, change),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangeKind, ChangeStatus};
    use rusqlite::Connection;
    use serde_json::Value;

    fn bpm_fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE djmdContent (ID TEXT PRIMARY KEY, BPM REAL);
             INSERT INTO djmdContent (ID, BPM) VALUES ('t1', 120.0);",
        )
        .unwrap();
        conn
    }

    fn bpm_change(new_bpm: f64) -> StagedChange {
        StagedChange {
            id: "c1".into(),
            library_path: None,
            kind: ChangeKind::TrackMetadataEdit,
            target_id: Some("t1".into()),
            field: Some("BPM".into()),
            old_value: Some(Value::from(120.0)),
            new_value: Some(Value::from(new_bpm)),
            reason: None,
            confidence: None,
            status: ChangeStatus::Accepted,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn keep_grids_skips_bpm_edit() {
        let mut conn = bpm_fixture();
        let tx = conn.transaction().unwrap();
        let opts = SyncOptions {
            keep_grids: true,
            ..Default::default()
        };
        let res = apply_with_options(&tx, &[bpm_change(128.0)], &opts).unwrap();
        // We mark it applied (acknowledging the user's accept), but the DB
        // row stays at its original value.
        assert_eq!(res.applied, vec!["c1"]);
        let bpm: f64 = tx
            .query_row("SELECT BPM FROM djmdContent WHERE ID='t1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(bpm, 120.0);
    }

    #[test]
    fn keep_grids_false_writes_bpm_edit() {
        let mut conn = bpm_fixture();
        let tx = conn.transaction().unwrap();
        let res = apply_with_options(&tx, &[bpm_change(128.0)], &SyncOptions::default()).unwrap();
        assert_eq!(res.applied, vec!["c1"]);
        let bpm: f64 = tx
            .query_row("SELECT BPM FROM djmdContent WHERE ID='t1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(bpm, 128.0);
    }
}
