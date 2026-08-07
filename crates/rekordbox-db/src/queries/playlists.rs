use crate::types::{Playlist, PlaylistEntry, PlaylistKind};
use anyhow::Result;
use rusqlite::{params, Connection};

pub fn all(conn: &Connection) -> Result<Vec<Playlist>> {
    let mut stmt = conn.prepare(
        "SELECT ID, Name, ParentID, Seq, Attribute
         FROM djmdPlaylist
         ORDER BY ParentID NULLS FIRST, Seq",
    )?;
    let rows = stmt.query_map([], |row| {
        let kind = PlaylistKind::from_attribute(row.get::<_, i64>(4)?);
        Ok(Playlist {
            id: row.get(0)?,
            name: row.get(1)?,
            parent_id: row.get(2)?,
            seq: row.get(3)?,
            kind,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn by_id(conn: &Connection, id: &str) -> Result<Option<Playlist>> {
    let mut stmt = conn.prepare(
        "SELECT ID, Name, ParentID, Seq, Attribute
         FROM djmdPlaylist
         WHERE ID = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        let kind = PlaylistKind::from_attribute(row.get::<_, i64>(4)?);
        Ok(Playlist {
            id: row.get(0)?,
            name: row.get(1)?,
            parent_id: row.get(2)?,
            seq: row.get(3)?,
            kind,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Return the distinct set of track/content IDs that appear in any
/// playlist (including folders' descendant playlists, since every entry
/// in djmdSongPlaylist points directly at its containing playlist).
/// Used by the UI to power a "not in any playlist" filter.
pub fn track_ids_in_any_playlist(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT ContentID FROM djmdSongPlaylist WHERE ContentID IS NOT NULL")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// How many **distinct** playlists hold each track.
///
/// Tracks in no playlist are absent from the map rather than present with a
/// zero — the query cannot see them, and inventing rows for them here would
/// mean loading the whole library into a map that only two callers need.
/// `occurrence_counts` is what turns that absence back into an N=0 answer.
///
/// Distinct because Rekordbox allows the same track twice in one playlist, and
/// "appears in two playlists" must not be satisfied by appearing twice in one.
pub fn playlist_occurrence(conn: &Connection) -> Result<std::collections::HashMap<String, usize>> {
    let mut stmt = conn.prepare(
        "SELECT ContentID, COUNT(DISTINCT PlaylistID)
         FROM djmdSongPlaylist
         WHERE ContentID IS NOT NULL
         GROUP BY ContentID",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
    })?;
    rows.collect::<rusqlite::Result<std::collections::HashMap<_, _>>>()
        .map_err(Into::into)
}

pub fn entries(conn: &Connection, playlist_id: &str) -> Result<Vec<PlaylistEntry>> {
    let mut stmt = conn.prepare(
        "SELECT PlaylistID, ContentID, TrackNo
         FROM djmdSongPlaylist
         WHERE PlaylistID = ?1
         ORDER BY TrackNo",
    )?;
    let rows = stmt.query_map(params![playlist_id], |row| {
        Ok(PlaylistEntry {
            playlist_id: row.get(0)?,
            content_id: row.get(1)?,
            track_no: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::test_helpers::create_test_db;
    use tempfile::NamedTempFile;

    fn make_db() -> tempfile::TempPath {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.into_temp_path();
        let conn = create_test_db(&path).unwrap();
        conn.execute_batch(include_str!("../sql/schema.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../sql/seed.sql")).unwrap();
        drop(conn);
        path
    }

    #[test]
    fn all_playlists() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let playlists = all(&conn).unwrap();
        // seed has 1 folder + 2 playlists
        assert_eq!(playlists.len(), 3);
    }

    #[test]
    fn folder_kind() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let playlists = all(&conn).unwrap();
        let folder = playlists.iter().find(|p| p.name == "Root Folder").unwrap();
        assert_eq!(folder.kind, PlaylistKind::Folder);
    }

    #[test]
    fn playlist_entries_ordered() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        // playlist ID "2" has 2 entries in seed
        let entries = entries(&conn, "2").unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].track_no <= entries[1].track_no);
    }

    #[test]
    fn by_id_found() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let playlist = by_id(&conn, "2").unwrap().unwrap();
        assert_eq!(playlist.name, "Techno Set");
        assert_eq!(playlist.kind, PlaylistKind::Playlist);
    }

    #[test]
    fn by_id_missing() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        assert!(by_id(&conn, "9999").unwrap().is_none());
    }

    #[test]
    fn entries_empty_playlist() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let entries = entries(&conn, "9999").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn track_ids_in_any_playlist_distinct() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let ids = track_ids_in_any_playlist(&conn).unwrap();
        // Seed places at least one track in a playlist; result must be
        // distinct and non-empty.
        assert!(!ids.is_empty(), "seed should put tracks in playlists");
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "result must be distinct");
    }

    #[test]
    fn playlist_occurrence_counts_distinct_playlists() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        // Seed: track 1 in "Techno Set", track 2 in "Techno Set", track 3 in
        // "House Vibes". Add track 1 to "House Vibes" twice — the duplicate
        // row must not make it look like three playlists.
        conn.execute_batch(
            "INSERT INTO djmdSongPlaylist (ID, PlaylistID, ContentID, TrackNo)
             VALUES ('x1', 3, '1', 2), ('x2', 3, '1', 3);",
        )
        .unwrap();

        let counts = playlist_occurrence(&conn).unwrap();
        assert_eq!(counts.get("1"), Some(&2));
        assert_eq!(counts.get("2"), Some(&1));
        // A track in no playlist is absent, not zero.
        assert_eq!(counts.get("4"), None);
    }
}
