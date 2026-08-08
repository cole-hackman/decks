use crate::StagedChange;
use anyhow::{anyhow, bail};
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::Value;

/// `PlaylistCreate`:
/// - `target_id` = new playlist ID (caller-supplied; stable across preview/apply)
/// - `new_value` = `{name: str, parent_id?: str, attribute?: int}`
pub(super) fn apply_create(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id (playlist id)"))?;
    let new = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let obj = new
        .as_object()
        .ok_or_else(|| anyhow!("new_value must be object"))?;

    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("name required"))?;
    let parent_id = obj.get("parent_id").and_then(Value::as_str);
    let attribute = obj.get("attribute").and_then(Value::as_i64).unwrap_or(0);

    tx.execute(
        "INSERT INTO djmdPlaylist (ID, Name, ParentID, Attribute) VALUES (?, ?, ?, ?)",
        params![id, name, parent_id, attribute],
    )?;
    Ok(())
}

/// `PlaylistRename`:
/// - `target_id` = playlist ID
/// - `new_value` = `{name: str}` OR a bare string
pub(super) fn apply_rename(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let new = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let name = match new {
        Value::String(s) => s.as_str(),
        Value::Object(o) => o
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("name required"))?,
        _ => bail!("new_value must be string or object with name"),
    };
    let rows = tx.execute(
        "UPDATE djmdPlaylist SET Name = ? WHERE ID = ?",
        params![name, id],
    )?;
    if rows == 0 {
        bail!("No playlist updated (id {} not found)", id);
    }
    Ok(())
}

/// `PlaylistDelete`:
/// - `target_id` = playlist ID
pub(super) fn apply_delete(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    tx.execute(
        "DELETE FROM djmdSongPlaylist WHERE PlaylistID = ?",
        params![id],
    )?;
    let rows = tx.execute("DELETE FROM djmdPlaylist WHERE ID = ?", params![id])?;
    if rows == 0 {
        bail!("No playlist deleted (id {} not found)", id);
    }
    Ok(())
}

/// `PlaylistAddTrack`:
/// - `target_id` = playlist ID
/// - `new_value` = `{content_id: str, track_no?: int}`
pub(super) fn apply_add_track(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let playlist_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let new = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let obj = new
        .as_object()
        .ok_or_else(|| anyhow!("new_value must be object"))?;
    let content_id = obj
        .get("content_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("content_id required"))?;

    let track_no = match obj.get("track_no").and_then(Value::as_i64) {
        Some(n) => n,
        None => {
            let max: Option<i64> = tx
                .query_row(
                    "SELECT MAX(TrackNo) FROM djmdSongPlaylist WHERE PlaylistID = ?",
                    params![playlist_id],
                    |r| r.get(0),
                )
                .ok()
                .flatten();
            max.unwrap_or(0) + 1
        }
    };

    let entry_id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO djmdSongPlaylist (ID, PlaylistID, ContentID, TrackNo) VALUES (?, ?, ?, ?)",
        params![entry_id, playlist_id, content_id, track_no],
    )?;
    Ok(())
}

/// `PlaylistRemoveTrack`:
/// - `target_id` = playlist ID
/// - `new_value` = `{content_id: str}`
pub(super) fn apply_remove_track(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let playlist_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let new = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let obj = new
        .as_object()
        .ok_or_else(|| anyhow!("new_value must be object"))?;
    let content_id = obj
        .get("content_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("content_id required"))?;

    let rows = tx.execute(
        "DELETE FROM djmdSongPlaylist WHERE PlaylistID = ? AND ContentID = ?",
        params![playlist_id, content_id],
    )?;
    if rows == 0 {
        bail!(
            "No playlist entry deleted ({} / {})",
            playlist_id,
            content_id
        );
    }
    Ok(())
}

/// `PlaylistReorderTrack`:
/// - `target_id` = playlist ID
/// - `new_value` = `{order: [content_id_in_desired_order, ...]}`
///
/// To avoid colliding under a UNIQUE(PlaylistID, TrackNo) constraint (if any),
/// first bump every existing row's TrackNo by +10000, then write the final
/// values. Both writes are inside the same outer transaction.
pub(super) fn apply_reorder(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let playlist_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let new = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let obj = new
        .as_object()
        .ok_or_else(|| anyhow!("new_value must be object"))?;
    let order = obj
        .get("order")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("order array required"))?;

    tx.execute(
        "UPDATE djmdSongPlaylist SET TrackNo = TrackNo + 10000 WHERE PlaylistID = ?",
        params![playlist_id],
    )?;
    for (idx, cid) in order.iter().enumerate() {
        let content_id = cid
            .as_str()
            .ok_or_else(|| anyhow!("order entries must be strings"))?;
        let track_no = (idx as i64) + 1;
        let rows = tx.execute(
            "UPDATE djmdSongPlaylist SET TrackNo = ? WHERE PlaylistID = ? AND ContentID = ?",
            params![track_no, playlist_id, content_id],
        )?;
        if rows == 0 {
            bail!(
                "Reorder references content {} not present in playlist {}",
                content_id,
                playlist_id
            );
        }
    }
    Ok(())
}

/// `PlaylistReorder`:
/// - `target_id` = parent folder ID, or absent for the root level
/// - `new_value` = `{order: [playlist_id, …]}`
///
/// Writes `djmdPlaylist.Seq`, which is what orders the tree. Unlike the track
/// reorder this does **not** two-phase the update: `Seq` has no uniqueness
/// constraint, so there is no collision to step around.
pub(super) fn apply_reorder_playlists(
    tx: &Transaction,
    change: &StagedChange,
) -> anyhow::Result<()> {
    let new = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let order = new
        .as_object()
        .and_then(|o| o.get("order"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("order array required"))?;

    for (idx, pid) in order.iter().enumerate() {
        let playlist_id = pid
            .as_str()
            .ok_or_else(|| anyhow!("order entries must be strings"))?;
        // The parent is part of the WHERE clause, so a reorder cannot move a
        // playlist between folders by accident — that would be a different
        // change, and this one only claims to reorder.
        let rows = match change.target_id.as_deref() {
            Some(parent) => tx.execute(
                "UPDATE djmdPlaylist SET Seq = ? WHERE ID = ? AND ParentID = ?",
                params![(idx as i64) + 1, playlist_id, parent],
            )?,
            None => tx.execute(
                "UPDATE djmdPlaylist SET Seq = ? WHERE ID = ? AND ParentID IS NULL",
                params![(idx as i64) + 1, playlist_id],
            )?,
        };
        if rows == 0 {
            bail!(
                "Reorder references playlist {} which is not in the target folder",
                playlist_id
            );
        }
    }
    Ok(())
}

/// `PlaylistMove`:
/// - `target_id` = the playlist or folder being moved
/// - `new_value` = `{parent_id: string | null, seq?: number}`
///
/// The drag-between half of the tree. `PlaylistReorder` deliberately refuses to
/// change a parent — a reorder that silently restructured the tree would be a
/// nasty surprise — so moving is its own change kind, and it carries its own
/// refusals.
///
/// Two things are checked before the write, because `djmdPlaylist` enforces
/// neither and getting either wrong corrupts the tree:
///
/// - **The destination must be a folder.** Rekordbox nests under folders only;
///   a playlist parented to a playlist is a shape nothing can render.
/// - **A folder cannot be moved inside itself or any of its descendants.**
///   That detaches the whole subtree from the root — it still exists, and it is
///   unreachable from the tree forever.
pub(super) fn apply_move(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let playlist_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let new = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let obj = new
        .as_object()
        .ok_or_else(|| anyhow!("new_value must be object"))?;

    // Absent and null both mean the root. A caller that omits the key is not
    // asking for something different from one that sends null.
    let parent_id = obj.get("parent_id").and_then(|v| v.as_str());
    let seq = obj.get("seq").and_then(Value::as_i64);

    if let Some(parent) = parent_id {
        if parent == playlist_id {
            bail!("A playlist cannot be moved into itself");
        }
        if !is_folder(tx, parent)? {
            bail!("Destination {parent} is not a folder");
        }
        if is_descendant_of(tx, parent, playlist_id)? {
            bail!("Cannot move a folder into its own descendant");
        }
    }

    let rows = match (parent_id, seq) {
        (Some(parent), Some(seq)) => tx.execute(
            "UPDATE djmdPlaylist SET ParentID = ?, Seq = ? WHERE ID = ?",
            params![parent, seq, playlist_id],
        )?,
        (Some(parent), None) => tx.execute(
            "UPDATE djmdPlaylist SET ParentID = ? WHERE ID = ?",
            params![parent, playlist_id],
        )?,
        (None, Some(seq)) => tx.execute(
            "UPDATE djmdPlaylist SET ParentID = NULL, Seq = ? WHERE ID = ?",
            params![seq, playlist_id],
        )?,
        (None, None) => tx.execute(
            "UPDATE djmdPlaylist SET ParentID = NULL WHERE ID = ?",
            params![playlist_id],
        )?,
    };
    if rows == 0 {
        bail!("No rows updated (playlist {} not found)", playlist_id);
    }
    Ok(())
}

/// `djmdPlaylist.Attribute` 1 is a folder; 0 is a playlist, 4 a smart playlist.
fn is_folder(tx: &Transaction, id: &str) -> anyhow::Result<bool> {
    let attribute: Option<i64> = tx
        .query_row(
            "SELECT Attribute FROM djmdPlaylist WHERE ID = ?",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(attribute == Some(1))
}

/// Is `candidate` `ancestor`, or somewhere beneath it?
///
/// Walks upward from `candidate` rather than downward from `ancestor`: a tree
/// is much wider than it is deep, and the walk is bounded by depth either way.
/// The visited set is a cycle guard — a database that already contains one must
/// not hang the sync.
fn is_descendant_of(tx: &Transaction, candidate: &str, ancestor: &str) -> anyhow::Result<bool> {
    let mut current = candidate.to_string();
    let mut seen = std::collections::HashSet::new();
    loop {
        if current == ancestor {
            return Ok(true);
        }
        if !seen.insert(current.clone()) {
            // Already-cyclic data. Report "not a descendant" rather than
            // looping; the move is not what created the problem.
            return Ok(false);
        }
        let parent: Option<String> = tx
            .query_row(
                "SELECT ParentID FROM djmdPlaylist WHERE ID = ?",
                params![current],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten();
        match parent {
            Some(p) if !p.is_empty() => current = p,
            _ => return Ok(false),
        }
    }
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
            "CREATE TABLE djmdPlaylist (ID TEXT PRIMARY KEY, Seq INTEGER, Name TEXT, Attribute INTEGER, ParentID TEXT);
             CREATE TABLE djmdSongPlaylist (ID TEXT PRIMARY KEY, PlaylistID TEXT, ContentID TEXT, TrackNo INTEGER);",
        )
        .unwrap();
        conn
    }

    fn ch(kind: ChangeKind, target: &str, val: Value) -> StagedChange {
        StagedChange {
            id: "c".into(),
            library_path: None,
            kind,
            target_id: Some(target.into()),
            field: None,
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
    fn create_rename_delete_roundtrip() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        apply_create(
            &tx,
            &ch(ChangeKind::PlaylistCreate, "p1", json!({"name": "Set 1"})),
        )
        .unwrap();
        apply_rename(
            &tx,
            &ch(
                ChangeKind::PlaylistRename,
                "p1",
                json!({"name": "Set 1 (final)"}),
            ),
        )
        .unwrap();
        let name: String = tx
            .query_row("SELECT Name FROM djmdPlaylist WHERE ID='p1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "Set 1 (final)");
        apply_delete(&tx, &ch(ChangeKind::PlaylistDelete, "p1", Value::Null)).unwrap();
        let n: i64 = tx
            .query_row("SELECT COUNT(*) FROM djmdPlaylist WHERE ID='p1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn add_track_assigns_next_track_no() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO djmdPlaylist (ID, Name) VALUES ('p1', 'x')", [])
            .unwrap();
        apply_add_track(
            &tx,
            &ch(
                ChangeKind::PlaylistAddTrack,
                "p1",
                json!({"content_id": "t1"}),
            ),
        )
        .unwrap();
        apply_add_track(
            &tx,
            &ch(
                ChangeKind::PlaylistAddTrack,
                "p1",
                json!({"content_id": "t2"}),
            ),
        )
        .unwrap();
        let nos: Vec<i64> = tx
            .prepare("SELECT TrackNo FROM djmdSongPlaylist WHERE PlaylistID='p1' ORDER BY TrackNo")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(nos, vec![1, 2]);
    }

    #[test]
    fn remove_track_deletes_row() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO djmdPlaylist (ID, Name) VALUES ('p1','x')", [])
            .unwrap();
        tx.execute(
            "INSERT INTO djmdSongPlaylist (ID, PlaylistID, ContentID, TrackNo) VALUES ('e1','p1','t1',1)",
            [],
        )
        .unwrap();
        apply_remove_track(
            &tx,
            &ch(
                ChangeKind::PlaylistRemoveTrack,
                "p1",
                json!({"content_id": "t1"}),
            ),
        )
        .unwrap();
        let n: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM djmdSongPlaylist WHERE PlaylistID='p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn reorder_rewrites_track_nos() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO djmdPlaylist (ID, Name) VALUES ('p1','x')", [])
            .unwrap();
        tx.execute_batch(
            "INSERT INTO djmdSongPlaylist (ID, PlaylistID, ContentID, TrackNo) VALUES
                ('e1','p1','t1',1),
                ('e2','p1','t2',2),
                ('e3','p1','t3',3);",
        )
        .unwrap();
        apply_reorder(
            &tx,
            &ch(
                ChangeKind::PlaylistReorderTrack,
                "p1",
                json!({"order": ["t3", "t1", "t2"]}),
            ),
        )
        .unwrap();
        let rows: Vec<(String, i64)> = tx
            .prepare("SELECT ContentID, TrackNo FROM djmdSongPlaylist WHERE PlaylistID='p1' ORDER BY TrackNo")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            vec![("t3".into(), 1), ("t1".into(), 2), ("t2".into(), 3),]
        );
    }

    #[test]
    fn reorder_with_unknown_track_errors_and_rolls_back() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO djmdPlaylist (ID, Name) VALUES ('p1','x')", [])
            .unwrap();
        tx.execute(
            "INSERT INTO djmdSongPlaylist (ID, PlaylistID, ContentID, TrackNo) VALUES ('e1','p1','t1',1)",
            [],
        )
        .unwrap();
        let res = apply_reorder(
            &tx,
            &ch(
                ChangeKind::PlaylistReorderTrack,
                "p1",
                json!({"order": ["t1", "ghost"]}),
            ),
        );
        assert!(res.is_err());
    }

    fn seqs(tx: &Transaction) -> Vec<(String, i64)> {
        tx.prepare("SELECT ID, Seq FROM djmdPlaylist ORDER BY Seq")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn reordering_playlists_rewrites_seq_within_the_folder() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute_batch(
            "INSERT INTO djmdPlaylist (ID, Seq, Name, ParentID) VALUES
                ('a', 1, 'Alpha', 'root'),
                ('b', 2, 'Beta',  'root'),
                ('c', 3, 'Gamma', 'root');",
        )
        .unwrap();

        apply_reorder_playlists(
            &tx,
            &ch(
                ChangeKind::PlaylistReorder,
                "root",
                json!({"order": ["c", "a", "b"]}),
            ),
        )
        .unwrap();

        assert_eq!(
            seqs(&tx),
            vec![("c".into(), 1), ("a".into(), 2), ("b".into(), 3)]
        );
    }

    #[test]
    fn a_root_level_reorder_targets_the_null_parent() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute_batch(
            "INSERT INTO djmdPlaylist (ID, Seq, Name, ParentID) VALUES
                ('a', 1, 'Alpha', NULL),
                ('b', 2, 'Beta',  NULL);",
        )
        .unwrap();

        let mut change = ch(
            ChangeKind::PlaylistReorder,
            "unused",
            json!({"order": ["b", "a"]}),
        );
        change.target_id = None;

        apply_reorder_playlists(&tx, &change).unwrap();
        assert_eq!(seqs(&tx), vec![("b".into(), 1), ("a".into(), 2)]);
    }

    #[test]
    fn a_reorder_cannot_move_a_playlist_between_folders() {
        // The parent is in the WHERE clause, so naming a playlist that lives
        // somewhere else fails loudly rather than silently reparenting it.
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        tx.execute_batch(
            "INSERT INTO djmdPlaylist (ID, Seq, Name, ParentID) VALUES
                ('a', 1, 'Alpha', 'root'),
                ('elsewhere', 1, 'Other', 'other');",
        )
        .unwrap();

        let res = apply_reorder_playlists(
            &tx,
            &ch(
                ChangeKind::PlaylistReorder,
                "root",
                json!({"order": ["a", "elsewhere"]}),
            ),
        );
        assert!(res.is_err());
    }

    /// Root folder `f1` holds folder `f2`, which holds playlist `p1`.
    /// `f3` is a sibling folder, `p2` a sibling playlist.
    fn tree() -> Connection {
        let conn = fixture();
        conn.execute_batch(
            "INSERT INTO djmdPlaylist (ID, Seq, Name, Attribute, ParentID) VALUES
               ('f1', 1, 'Folder One',   1, NULL),
               ('f2', 1, 'Folder Two',   1, 'f1'),
               ('f3', 2, 'Folder Three', 1, NULL),
               ('p1', 1, 'Playlist One', 0, 'f2'),
               ('p2', 3, 'Playlist Two', 0, NULL);",
        )
        .unwrap();
        conn
    }

    fn parent_of(conn: &Connection, id: &str) -> Option<String> {
        conn.query_row(
            "SELECT ParentID FROM djmdPlaylist WHERE ID = ?",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn do_move(conn: &mut Connection, id: &str, value: Value) -> anyhow::Result<()> {
        let tx = conn.transaction().unwrap();
        let res = apply_move(&tx, &ch(ChangeKind::PlaylistMove, id, value));
        if res.is_ok() {
            tx.commit().unwrap();
        }
        res
    }

    #[test]
    fn a_playlist_moves_into_a_folder() {
        let mut conn = tree();
        do_move(&mut conn, "p2", serde_json::json!({"parent_id": "f3"})).unwrap();
        assert_eq!(parent_of(&conn, "p2").as_deref(), Some("f3"));
    }

    #[test]
    fn a_playlist_moves_back_to_the_root() {
        let mut conn = tree();
        do_move(&mut conn, "p1", serde_json::json!({"parent_id": null})).unwrap();
        assert_eq!(parent_of(&conn, "p1"), None);
    }

    #[test]
    fn an_absent_parent_key_means_the_root_just_like_null() {
        // A caller that omits the key is not asking for something different
        // from one that sends null.
        let mut conn = tree();
        do_move(&mut conn, "p1", serde_json::json!({})).unwrap();
        assert_eq!(parent_of(&conn, "p1"), None);
    }

    #[test]
    fn a_move_can_set_the_position_at_the_same_time() {
        let mut conn = tree();
        do_move(
            &mut conn,
            "p2",
            serde_json::json!({"parent_id": "f3", "seq": 7}),
        )
        .unwrap();
        let seq: i64 = conn
            .query_row("SELECT Seq FROM djmdPlaylist WHERE ID = 'p2'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(seq, 7);
    }

    /// Rekordbox nests under folders only.
    #[test]
    fn a_playlist_cannot_become_the_parent_of_another() {
        let mut conn = tree();
        let err = do_move(&mut conn, "p2", serde_json::json!({"parent_id": "p1"})).unwrap_err();
        assert!(err.to_string().contains("not a folder"), "{err}");
        assert_eq!(parent_of(&conn, "p2"), None, "nothing should have moved");
    }

    /// The move that would detach a whole subtree from the root forever.
    #[test]
    fn a_folder_cannot_be_moved_into_its_own_descendant() {
        let mut conn = tree();
        let err = do_move(&mut conn, "f1", serde_json::json!({"parent_id": "f2"})).unwrap_err();
        assert!(err.to_string().contains("own descendant"), "{err}");
        assert_eq!(parent_of(&conn, "f1"), None);
    }

    #[test]
    fn a_folder_cannot_be_moved_into_itself() {
        let mut conn = tree();
        let err = do_move(&mut conn, "f1", serde_json::json!({"parent_id": "f1"})).unwrap_err();
        assert!(err.to_string().contains("into itself"), "{err}");
    }

    #[test]
    fn a_folder_can_still_move_into_an_unrelated_folder() {
        // The descendant check must not be so broad that it refuses legitimate
        // moves — `f3` is a sibling, not a descendant.
        let mut conn = tree();
        do_move(&mut conn, "f1", serde_json::json!({"parent_id": "f3"})).unwrap();
        assert_eq!(parent_of(&conn, "f1").as_deref(), Some("f3"));
    }

    #[test]
    fn moving_a_playlist_that_does_not_exist_fails_loudly() {
        let mut conn = tree();
        let err = do_move(&mut conn, "nope", serde_json::json!({"parent_id": "f3"})).unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
    }

    #[test]
    fn already_cyclic_data_does_not_hang_the_move() {
        // A database that already contains a cycle must not loop the sync. The
        // move is not what created the problem, so it is allowed through.
        let mut conn = tree();
        conn.execute_batch(
            "UPDATE djmdPlaylist SET ParentID = 'f2' WHERE ID = 'f1';
             UPDATE djmdPlaylist SET ParentID = 'f1' WHERE ID = 'f2';",
        )
        .unwrap();
        let _ = do_move(&mut conn, "p2", serde_json::json!({"parent_id": "f3"}));
    }
}
