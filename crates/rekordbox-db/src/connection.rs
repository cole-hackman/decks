use crate::types::{BrokenMetadataReport, DuplicateGroup};
use crate::{
    queries,
    types::{BeatGridEntry, HotCue, Playlist, PlaylistEntry, Track},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// Shared passphrase for every Rekordbox 6/7 database.
/// Derived by reverse-engineering the Rekordbox application; publicly documented
/// in pyrekordbox and reklawdbox.
pub(crate) const RB_SQLCIPHER_KEY: &str =
    "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";

/// Read-only handle to a Rekordbox `master.db`.
pub struct RekordboxDb {
    pub(crate) conn: Connection,
}

impl RekordboxDb {
    /// Open `master.db` at `path` in read-only mode.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("opening master.db at {}", path.display()))?;

        apply_pragmas(&conn)?;
        Ok(Self { conn })
    }

    // ── Tracks ──────────────────────────────────────────────────────────────

    pub fn tracks(&self) -> Result<Vec<Track>> {
        queries::tracks::all(&self.conn)
    }

    pub fn track_by_id(&self, id: &str) -> Result<Option<Track>> {
        queries::tracks::by_id(&self.conn, id)
    }

    pub fn search_tracks(&self, query: &str) -> Result<Vec<Track>> {
        queries::tracks::search(&self.conn, query)
    }

    pub fn list_genres(&self) -> Result<Vec<crate::types::GenreCount>> {
        queries::tracks::list_genres(&self.conn)
    }

    pub fn list_artists(&self) -> Result<Vec<crate::types::ArtistCount>> {
        queries::tracks::list_artists(&self.conn)
    }

    pub fn tracks_by_genre(&self, genre: &str) -> Result<Vec<Track>> {
        queries::tracks::by_genre(&self.conn, genre)
    }

    pub fn tracks_by_artist(&self, artist: &str) -> Result<Vec<Track>> {
        queries::tracks::by_artist(&self.conn, artist)
    }

    pub fn tracks_added_since(&self, watermark_iso: &str) -> Result<Vec<Track>> {
        queries::tracks::added_since(&self.conn, watermark_iso)
    }

    pub fn tracks_by_ids(&self, ids: &[String]) -> Result<Vec<Track>> {
        queries::tracks::by_ids(&self.conn, ids)
    }

    // ── Playlists ────────────────────────────────────────────────────────────

    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        queries::playlists::all(&self.conn)
    }

    pub fn playlist_by_id(&self, playlist_id: &str) -> Result<Option<Playlist>> {
        queries::playlists::by_id(&self.conn, playlist_id)
    }

    pub fn playlist_entries(&self, playlist_id: &str) -> Result<Vec<PlaylistEntry>> {
        queries::playlists::entries(&self.conn, playlist_id)
    }

    pub fn track_ids_in_any_playlist(&self) -> Result<Vec<String>> {
        queries::playlists::track_ids_in_any_playlist(&self.conn)
    }

    /// How many distinct playlists hold each track. Tracks in none are absent.
    pub fn playlist_occurrence(&self) -> Result<std::collections::HashMap<String, usize>> {
        queries::playlists::playlist_occurrence(&self.conn)
    }

    // ── History ──────────────────────────────────────────────────────────────

    /// Play sessions Rekordbox has logged, newest first.
    pub fn history_sets(&self) -> Result<Vec<queries::history::HistorySet>> {
        queries::history::sets(&self.conn)
    }

    /// The tracks in one session, in play order.
    pub fn history_entries(&self, history_id: &str) -> Result<Vec<queries::history::HistoryEntry>> {
        queries::history::entries(&self.conn, history_id)
    }

    // ── Health ───────────────────────────────────────────────────────────────

    pub fn duplicate_tracks(&self) -> Result<Vec<DuplicateGroup>> {
        Ok(queries::health::duplicate_tracks(self.tracks()?))
    }

    pub fn fuzzy_duplicate_tracks(&self) -> Result<Vec<DuplicateGroup>> {
        Ok(queries::health::fuzzy_duplicate_tracks(self.tracks()?))
    }

    pub fn audio_fingerprint_duplicates(
        &self,
        fingerprints: &std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<Vec<DuplicateGroup>> {
        queries::health::audio_fingerprint_duplicates(self.tracks()?, fingerprints)
    }

    /// Library-wide duplicate scan combining exact title+artist, fuzzy title,
    /// and audio-fingerprint strategies in a single call.
    pub fn library_duplicate_groups(
        &self,
        fingerprints: &std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<Vec<DuplicateGroup>> {
        queries::health::library_duplicate_groups(self.tracks()?, fingerprints)
    }

    pub fn broken_metadata_report(&self) -> Result<BrokenMetadataReport> {
        queries::health::broken_metadata_report(self.tracks()?)
    }

    pub fn library_analytics(&self) -> Result<crate::types::LibraryAnalytics> {
        queries::analytics::library_analytics(&self.conn)
    }

    // ── Cues ─────────────────────────────────────────────────────────────────

    pub fn hot_cues_for_track(&self, content_id: &str) -> Result<Vec<HotCue>> {
        queries::cues::for_track(&self.conn, content_id)
    }

    pub fn all_hot_cues(&self) -> Result<Vec<HotCue>> {
        queries::cues::all(&self.conn)
    }

    pub fn track_ids_with_cues(&self) -> Result<Vec<String>> {
        queries::cues::track_ids_with_cues(&self.conn)
    }

    // ── Beat grid (ANLZ) ─────────────────────────────────────────────────────

    /// Parse the beat grid for a track from its ANLZ `.DAT` file.
    /// `anlz_dat_path` must be the absolute path to the `ANLZ0000.DAT` file.
    pub fn beat_grid(anlz_dat_path: &Path) -> Result<Vec<BeatGridEntry>> {
        crate::anlz::read_beat_grid(anlz_dat_path)
    }
}

pub(crate) fn apply_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(&format!(
        "PRAGMA key = '{RB_SQLCIPHER_KEY}'; PRAGMA busy_timeout = 5000;"
    ))
    .context("applying SQLCipher pragmas")?;
    Ok(())
}

// ── Safety test ─────────────────────────────────────────────────────────────
// This test verifies that a connection opened via `RekordboxDb::open` is truly
// read-only and will reject any write operation.
#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use rusqlite::Connection;

    /// Create a new, writable SQLCipher database at `path` for use in tests.
    /// Returns the raw rusqlite connection so the caller can set up schema/data.
    pub fn create_test_db(path: &Path) -> Result<Connection> {
        let conn = Connection::open(path)?;
        conn.execute_batch(&format!(
            "PRAGMA key = '{RB_SQLCIPHER_KEY}'; PRAGMA busy_timeout = 5000;"
        ))?;
        Ok(conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn read_only_connection_rejects_writes() -> Result<()> {
        // Create a minimal SQLCipher db, then verify our read-only opener
        // refuses INSERT/UPDATE/CREATE.
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();
        // Drop the file so the path exists but is empty – SQLite will create it.
        drop(tmp);

        {
            let setup = test_helpers::create_test_db(&path)?;
            setup.execute_batch("CREATE TABLE t (x INTEGER);")?;
        }

        let db = RekordboxDb::open(&path)?;
        let result = db
            .conn
            .execute("INSERT INTO t VALUES (1)", rusqlite::params![]);
        assert!(
            result.is_err(),
            "expected write to fail on read-only connection"
        );
        Ok(())
    }
}
