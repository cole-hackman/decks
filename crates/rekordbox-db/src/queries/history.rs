//! Play history — Rekordbox's per-session log.
//!
//! Per `docs/lexicon/09-history-backup.md §History`. Rekordbox stores a "set"
//! per session in `djmdHistory`, with its tracks in `djmdSongHistory` — the
//! same shape as playlists, keyed by date.
//!
//! This module only **reads**. The snapshot rule (history is a historical
//! record, not a view over current data) means the import copies what it finds
//! into our own tables; nothing here is a live view.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// One session as Rekordbox logged it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistorySet {
    pub id: String,
    pub name: String,
    /// ISO-8601, as Rekordbox stores it. `None` on rows that predate the
    /// column being populated.
    pub played_at: Option<String>,
}

/// A track as it appeared in a session, joined to the library **at read time**.
///
/// The fields are copied into our snapshot on import precisely because this
/// join is the thing that goes stale: editing a track later must not rewrite
/// what the gig log says was played.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub history_id: String,
    pub content_id: String,
    pub track_no: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub musical_key: Option<String>,
    pub bpm: Option<f64>,
    pub duration_secs: Option<i64>,
    pub folder_path: Option<String>,
}

/// Every session Rekordbox has not soft-deleted, newest first.
///
/// `rb_local_deleted` is Rekordbox's own tombstone. A set the user deleted in
/// Rekordbox should not reappear in our gig log the first time we import.
pub fn sets(conn: &Connection) -> Result<Vec<HistorySet>> {
    let mut stmt = conn.prepare(
        "SELECT ID, Name, DateCreated
         FROM djmdHistory
         WHERE COALESCE(rb_local_deleted, 0) = 0
         ORDER BY DateCreated DESC, Seq DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(HistorySet {
            id: row.get(0)?,
            name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            played_at: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// The tracks in one session, in play order, joined to the library.
///
/// A `LEFT JOIN` on purpose: a session can reference a track that has since
/// been removed from the library, and dropping the row would silently shorten
/// the gig log. Such a row comes back with its ids and nothing else — which is
/// exactly the case the snapshot exists to survive.
pub fn entries(conn: &Connection, history_id: &str) -> Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT sh.HistoryID, sh.ContentID, sh.TrackNo,
                c.Title, ar.Name, al.Name, g.Name, k.ScaleName,
                c.BPM, c.Length, c.FolderPath
         FROM djmdSongHistory sh
         LEFT JOIN djmdContent c  ON c.ID = sh.ContentID
         LEFT JOIN djmdArtist ar  ON ar.ID = c.ArtistID
         LEFT JOIN djmdAlbum  al  ON al.ID = c.AlbumID
         LEFT JOIN djmdGenre  g   ON g.ID = c.GenreID
         LEFT JOIN djmdKey    k   ON k.ID = c.KeyID
         WHERE sh.HistoryID = ?1
         ORDER BY sh.TrackNo",
    )?;
    let rows = stmt.query_map(params![history_id], |row| {
        Ok(HistoryEntry {
            history_id: row.get(0)?,
            content_id: row.get(1)?,
            track_no: row.get(2)?,
            title: row.get(3)?,
            artist: row.get(4)?,
            album: row.get(5)?,
            genre: row.get(6)?,
            musical_key: row.get(7)?,
            // BPM is stored as an integer x100, as everywhere else.
            bpm: row.get::<_, Option<i64>>(8)?.map(|raw| raw as f64 / 100.0),
            duration_secs: row.get(9)?,
            folder_path: row.get(10)?,
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
    fn sets_come_back_newest_first() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let got = sets(&conn).unwrap();
        assert_eq!(
            got.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["h2", "h1"]
        );
        assert_eq!(got[0].name, "2026-06-12 Rooftop");
        assert_eq!(got[0].played_at.as_deref(), Some("2026-06-12T21:30:00Z"));
    }

    #[test]
    fn a_session_rekordbox_deleted_is_not_listed() {
        // Its own tombstone. Resurrecting it in our gig log the first time we
        // import would be the opposite of helpful.
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        assert!(sets(&conn).unwrap().iter().all(|s| s.id != "h3"));
    }

    #[test]
    fn entries_come_back_in_play_order_with_library_data() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let got = entries(&conn, "h1").unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].content_id, "1");
        assert_eq!(got[0].title.as_deref(), Some("Test Track Alpha"));
        assert_eq!(got[0].artist.as_deref(), Some("Artist One"));
        assert_eq!(got[0].musical_key.as_deref(), Some("8A"));
        assert_eq!(got[0].bpm, Some(132.0));
        assert_eq!(got[1].content_id, "2");
    }

    #[test]
    fn a_track_no_longer_in_the_library_keeps_its_row() {
        // Dropping it would silently shorten the gig log — and this is exactly
        // the case the snapshot exists to survive.
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        conn.execute_batch(
            "INSERT INTO djmdSongHistory (ID, HistoryID, ContentID, TrackNo)
             VALUES ('sh9', 'h1', 'gone', 3);",
        )
        .unwrap();
        let got = entries(&conn, "h1").unwrap();
        assert_eq!(got.len(), 3);
        let orphan = got.iter().find(|e| e.content_id == "gone").unwrap();
        assert_eq!(orphan.title, None);
        assert_eq!(orphan.bpm, None);
    }

    #[test]
    fn an_unknown_session_has_no_entries() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        assert!(entries(&conn, "nope").unwrap().is_empty());
    }
}
