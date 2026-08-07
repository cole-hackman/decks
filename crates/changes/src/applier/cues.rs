use super::tracks::json_to_sql;
use crate::applier::{CueDestination, SyncOptions};
use crate::StagedChange;
use anyhow::{anyhow, bail};
use rusqlite::{params, Transaction};
use serde_json::Value;

const ALLOWED_CUE_FIELDS: &[&str] = &["InMsec", "OutMsec", "Kind", "Color", "Commnt"];

/// `TrackAddCue`:
/// - `target_id` = content (track) ID
/// - `new_value` = JSON object `{in_msec, out_msec?, kind?, color?, commnt?}`
///
/// `options.cue_destination` controls which `djmdCue.Kind` slot is written:
///   - `Hot` (default)  → write a hot-cue row (Kind = staged value if 1..=8, else 1).
///   - `Memory`         → force Kind = 0 (memory cue).
///   - `Both`           → insert both a memory row and a hot row.
///
/// `Kind` values per `crates/rekordbox-db/src/types.rs::CueKind::from_db`:
///   0   = MemoryCue
///   1–8 = HotCue (slot number)
pub(super) fn apply_add_cue(
    tx: &Transaction,
    change: &StagedChange,
    options: &SyncOptions,
) -> anyhow::Result<()> {
    let content_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let new = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let obj = new
        .as_object()
        .ok_or_else(|| anyhow!("new_value must be an object"))?;

    let in_msec = obj
        .get("in_msec")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow!("in_msec required"))?;
    let out_msec = obj.get("out_msec").and_then(Value::as_i64);
    let staged_kind = obj.get("kind").and_then(Value::as_i64).unwrap_or(0);
    let color = obj.get("color").and_then(Value::as_i64).unwrap_or(-1);
    let commnt = obj.get("commnt").and_then(Value::as_str);

    let hot_kind = if (1..=8).contains(&staged_kind) {
        staged_kind
    } else {
        1
    };
    let kinds_to_write: Vec<i64> = match options.cue_destination {
        CueDestination::Hot => vec![hot_kind],
        CueDestination::Memory => vec![0],
        CueDestination::Both => vec![0, hot_kind],
    };

    for kind in kinds_to_write {
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO djmdCue (ID, ContentID, InMsec, OutMsec, Kind, Color, Commnt)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![id, content_id, in_msec, out_msec, kind, color, commnt],
        )?;
    }
    Ok(())
}

/// `CueMetadataEdit`:
/// - `target_id` = cue ID
/// - `field` = column name (from allowlist)
/// - `new_value` = scalar
pub(super) fn apply_metadata_edit(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let cue_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let field = change
        .field
        .as_ref()
        .ok_or_else(|| anyhow!("Missing field"))?;
    let new_value = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;

    if !ALLOWED_CUE_FIELDS.contains(&field.as_str()) {
        bail!("Field {} is not in the cue allowlist", field);
    }

    let sql = format!("UPDATE djmdCue SET {} = ? WHERE ID = ?", field);
    let val = json_to_sql(new_value)?;
    let rows = tx.execute(&sql, params![val, cue_id])?;
    if rows == 0 {
        bail!("No cue updated (id {} not found)", cue_id);
    }
    Ok(())
}

/// `TrackDeleteCue`:
/// - `target_id` = cue ID
///
/// Deleting a cue that no longer exists is an error rather than a silent no-op:
/// it means the staged change was built against a stale view of the library,
/// and quietly succeeding would hide that from the user.
pub(super) fn apply_delete_cue(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let cue_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id (cue id)"))?;
    let affected = tx.execute("DELETE FROM djmdCue WHERE ID = ?", params![cue_id])?;
    if affected == 0 {
        return Err(anyhow!("Cue {cue_id} not found"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangeKind, ChangeStatus};
    use rusqlite::Connection;
    use serde_json::json;

    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE djmdCue (ID TEXT PRIMARY KEY, ContentID TEXT, InMsec INTEGER, OutMsec INTEGER, Kind INTEGER, Color INTEGER, Commnt TEXT);",
        )
        .unwrap();
        conn
    }

    fn change(
        kind: ChangeKind,
        target: Option<&str>,
        field: Option<&str>,
        val: Value,
    ) -> StagedChange {
        StagedChange {
            id: "c".into(),
            library_path: None,
            kind,
            target_id: target.map(str::to_string),
            field: field.map(str::to_string),
            old_value: None,
            new_value: Some(val),
            reason: None,
            confidence: None,
            status: ChangeStatus::Accepted,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn add_cue_inserts_row() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        apply_add_cue(
            &tx,
            &change(
                ChangeKind::TrackAddCue,
                Some("track1"),
                None,
                json!({"in_msec": 12345, "kind": 0, "color": 5, "commnt": "intro"}),
            ),
            &SyncOptions::default(),
        )
        .unwrap();
        let (in_msec, commnt): (i64, String) = tx
            .query_row(
                "SELECT InMsec, Commnt FROM djmdCue WHERE ContentID='track1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(in_msec, 12345);
        assert_eq!(commnt, "intro");
    }

    #[test]
    fn add_cue_memory_destination_forces_kind_zero() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        let opts = SyncOptions {
            cue_destination: CueDestination::Memory,
            ..Default::default()
        };
        apply_add_cue(
            &tx,
            &change(
                ChangeKind::TrackAddCue,
                Some("track1"),
                None,
                json!({"in_msec": 100, "kind": 3}),
            ),
            &opts,
        )
        .unwrap();
        let kind: i64 = tx
            .query_row(
                "SELECT Kind FROM djmdCue WHERE ContentID='track1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kind, 0);
    }

    #[test]
    fn add_cue_both_inserts_two_rows() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        let opts = SyncOptions {
            cue_destination: CueDestination::Both,
            ..Default::default()
        };
        apply_add_cue(
            &tx,
            &change(
                ChangeKind::TrackAddCue,
                Some("track1"),
                None,
                json!({"in_msec": 100, "kind": 4}),
            ),
            &opts,
        )
        .unwrap();
        let mut kinds: Vec<i64> = tx
            .prepare("SELECT Kind FROM djmdCue WHERE ContentID='track1' ORDER BY Kind")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        kinds.sort();
        assert_eq!(kinds, vec![0, 4]);
    }

    #[test]
    fn add_cue_hot_preserves_staged_slot() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        apply_add_cue(
            &tx,
            &change(
                ChangeKind::TrackAddCue,
                Some("track1"),
                None,
                json!({"in_msec": 100, "kind": 7}),
            ),
            &SyncOptions::default(),
        )
        .unwrap();
        let kind: i64 = tx
            .query_row(
                "SELECT Kind FROM djmdCue WHERE ContentID='track1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kind, 7);
    }

    #[test]
    fn cue_metadata_edit_updates() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute(
            "INSERT INTO djmdCue (ID, ContentID, InMsec) VALUES ('cue1', 'track1', 0)",
            [],
        )
        .unwrap();
        apply_metadata_edit(
            &tx,
            &change(
                ChangeKind::CueMetadataEdit,
                Some("cue1"),
                Some("Commnt"),
                Value::String("hook".into()),
            ),
        )
        .unwrap();
        let c: String = tx
            .query_row("SELECT Commnt FROM djmdCue WHERE ID='cue1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(c, "hook");
    }

    #[test]
    fn cue_metadata_disallowed_field_errors() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute(
            "INSERT INTO djmdCue (ID, ContentID, InMsec) VALUES ('cue1', 'track1', 0)",
            [],
        )
        .unwrap();
        let res = apply_metadata_edit(
            &tx,
            &change(
                ChangeKind::CueMetadataEdit,
                Some("cue1"),
                Some("ContentID"),
                Value::String("hijack".into()),
            ),
        );
        assert!(res.is_err());
    }

    #[test]
    fn delete_cue_removes_the_row() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute(
            "INSERT INTO djmdCue (ID, ContentID, InMsec, Kind, Color) VALUES ('c1','t1',1000,1,-1)",
            [],
        )
        .unwrap();

        apply_delete_cue(
            &tx,
            &change(ChangeKind::TrackDeleteCue, Some("c1"), None, Value::Null),
        )
        .unwrap();

        let count: i64 = tx
            .query_row("SELECT COUNT(*) FROM djmdCue WHERE ID = 'c1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn delete_cue_errors_when_the_cue_is_gone() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        assert!(apply_delete_cue(
            &tx,
            &change(
                ChangeKind::TrackDeleteCue,
                Some("missing"),
                None,
                Value::Null
            )
        )
        .is_err());
    }

    #[test]
    fn delete_cue_requires_a_target_id() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        assert!(apply_delete_cue(
            &tx,
            &change(ChangeKind::TrackDeleteCue, None, None, Value::Null)
        )
        .is_err());
    }
}
