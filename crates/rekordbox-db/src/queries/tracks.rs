use crate::queries::columns::{optional_column, table_columns, table_exists};
use crate::types::{ArtistCount, GenreCount, Track};
use anyhow::Result;
use rusqlite::{params, Connection};

/// The columns every Rekordbox library has had for as long as we have supported
/// them. Naming one of these unconditionally is safe.
const CORE_SELECT: &str = "
    c.ID,
    COALESCE(c.Title, '') AS Title,
    a.Name              AS Artist,
    al.Name             AS Album,
    g.Name              AS Genre,
    k.ScaleName         AS MusicKey,
    CAST(c.BPM AS REAL) / 100.0 AS BPM,
    CAST(c.Length       AS INTEGER) AS Length,
    CAST(c.Rating       AS INTEGER) AS Rating,
    c.Commnt,
    c.FolderPath,
    c.AnalysisDataPath,
    CAST(c.FileType     AS INTEGER) AS FileType,
    CAST(c.SampleRate   AS INTEGER) AS SampleRate,
    CAST(c.BitRate      AS INTEGER) AS BitRate,
    CAST(c.ReleaseYear  AS INTEGER) AS ReleaseYear,
    CAST(c.DJPlayCount  AS INTEGER) AS DJPlayCount";

/// Core SELECT — reused by every track query.
///
/// Built per connection rather than being a constant, because the five fields
/// after the core set (label, remixer, mix, colour, date added) are not present
/// in every library. Naming an absent column fails the entire query, so each is
/// probed and degrades to `NULL`: an old library keeps returning all its tracks
/// and simply has no label column, which is the honest answer.
///
/// BPM is stored as integer × 100; we convert to actual bpm here.
fn select(conn: &Connection) -> Result<String> {
    let content = table_columns(conn, "djmdContent")?;

    // The label and colour tables are joined only when they exist. Omitting a
    // column is not enough — `LEFT JOIN djmdLabel` against a database that has
    // no such table is a hard error no matter how the SELECT list is written.
    let has_label = table_exists(conn, "djmdLabel")? && !optional_is_null(&content, &["LabelID"]);
    let has_color = table_exists(conn, "djmdColor")? && !optional_is_null(&content, &["ColorID"]);
    let has_remixer = !optional_is_null(&content, &["RemixerID"]);

    let label_expr = if has_label { "lb.Name" } else { "NULL" };
    // `djmdColor` keeps the human-readable name in `Commnt`, not `Name` — a
    // Rekordbox quirk, and the reason this is probed rather than assumed.
    let color_expr = if has_color {
        "COALESCE(col.Commnt, col.Name)"
    } else {
        "NULL"
    };
    let remixer_expr = if has_remixer { "rmx.Name" } else { "NULL" };
    let mix_expr = optional_column(&content, "c", &["Subtitle"]);
    let date_added_expr = optional_column(&content, "c", &["DateCreated", "created_at"]);

    let label_join = if has_label {
        "LEFT JOIN djmdLabel  lb  ON c.LabelID = lb.ID"
    } else {
        ""
    };
    let color_join = if has_color {
        "LEFT JOIN djmdColor  col ON c.ColorID = col.ID"
    } else {
        ""
    };
    let remixer_join = if has_remixer {
        "LEFT JOIN djmdArtist rmx ON c.RemixerID = rmx.ID"
    } else {
        ""
    };

    Ok(format!(
        "
SELECT{CORE_SELECT},
    {label_expr}        AS Label,
    {remixer_expr}      AS Remixer,
    {mix_expr}          AS Mix,
    {color_expr}        AS Color,
    {date_added_expr}   AS DateAdded
FROM djmdContent c
LEFT JOIN djmdArtist a  ON c.ArtistID = a.ID
LEFT JOIN djmdAlbum  al ON c.AlbumID  = al.ID
LEFT JOIN djmdGenre  g  ON c.GenreID  = g.ID
LEFT JOIN djmdKey    k  ON c.KeyID    = k.ID
{label_join}
{color_join}
{remixer_join}
WHERE (c.rb_local_deleted IS NULL OR c.rb_local_deleted = 0)
"
    ))
}

/// Whether `optional_column` would degrade to `NULL` for these candidates.
fn optional_is_null(columns: &[String], candidates: &[&str]) -> bool {
    optional_column(columns, "c", candidates) == "NULL"
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        album: row.get(3)?,
        genre: row.get(4)?,
        musical_key: row.get(5)?,
        bpm: row.get(6)?,
        duration_secs: row.get(7)?,
        rating: row.get(8)?,
        comment: row.get(9)?,
        folder_path: row.get(10)?,
        analysis_data_path: row.get(11)?,
        file_type: row.get(12)?,
        sample_rate: row.get(13)?,
        bit_rate: row.get(14)?,
        release_year: row.get(15)?,
        dj_play_count: row.get(16)?,
        label: row.get(17)?,
        remixer: row.get(18)?,
        mix: row.get(19)?,
        color: row.get(20)?,
        date_added: row.get(21)?,
        energy: None,
    })
}

pub fn all(conn: &Connection) -> Result<Vec<Track>> {
    let sql = format!("{} ORDER BY a.Name, c.Title", select(conn)?);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_track)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn by_id(conn: &Connection, id: &str) -> Result<Option<Track>> {
    let sql = format!("{} AND c.ID = ?1", select(conn)?);
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map(params![id], row_to_track)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Case-insensitive substring search across title, artist, album, genre, comment.
pub fn search(conn: &Connection, query: &str) -> Result<Vec<Track>> {
    let pattern = format!("%{query}%");
    let sql = format!(
        "{}
         AND (
             c.Title  LIKE ?1 OR
             a.Name   LIKE ?1 OR
             al.Name  LIKE ?1 OR
             g.Name   LIKE ?1 OR
             c.Commnt LIKE ?1
         )
         ORDER BY a.Name, c.Title",
        select(conn)?
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![pattern], row_to_track)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn list_genres(conn: &Connection) -> Result<Vec<GenreCount>> {
    let sql = "
        SELECT g.Name, COUNT(c.ID) as Count
        FROM djmdContent c
        JOIN djmdGenre g ON c.GenreID = g.ID
        WHERE (c.rb_local_deleted IS NULL OR c.rb_local_deleted = 0)
        GROUP BY g.Name
        ORDER BY Count DESC, g.Name ASC
    ";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(GenreCount {
            genre: row.get(0)?,
            count: row.get(1)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn list_artists(conn: &Connection) -> Result<Vec<ArtistCount>> {
    let sql = "
        SELECT a.Name, COUNT(c.ID) as Count
        FROM djmdContent c
        JOIN djmdArtist a ON c.ArtistID = a.ID
        WHERE (c.rb_local_deleted IS NULL OR c.rb_local_deleted = 0)
        GROUP BY a.Name
        ORDER BY Count DESC, a.Name ASC
    ";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(ArtistCount {
            artist: row.get(0)?,
            count: row.get(1)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn by_genre(conn: &Connection, genre: &str) -> Result<Vec<Track>> {
    let sql = format!("{} AND g.Name = ?1 ORDER BY a.Name, c.Title", select(conn)?);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![genre], row_to_track)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn by_artist(conn: &Connection, artist: &str) -> Result<Vec<Track>> {
    let sql = format!("{} AND a.Name = ?1 ORDER BY a.Name, c.Title", select(conn)?);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![artist], row_to_track)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Tracks whose `DateCreated` is strictly greater than the given ISO 8601
/// string. Used by the Incoming sub-view: pass the user's `cleared_at`
/// watermark.
///
/// DateCreated is compared lexicographically; ISO 8601 with consistent
/// width orders correctly that way (and Rekordbox writes it that way).
pub fn added_since(conn: &Connection, watermark_iso: &str) -> Result<Vec<Track>> {
    let sql = format!(
        "{} AND COALESCE(c.DateCreated, '') > ?1 ORDER BY c.DateCreated DESC",
        select(conn)?
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![watermark_iso], row_to_track)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Lookup by a set of IDs, used for the Archive sub-view.
/// Splits into chunks of 500 to avoid SQL parameter limits.
pub fn by_ids(conn: &Connection, ids: &[String]) -> Result<Vec<Track>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(500) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!("{} AND c.ID IN ({placeholders})", select(conn)?);
        let mut stmt = conn.prepare(&sql)?;
        let params_iter: Vec<&dyn rusqlite::ToSql> =
            chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(&*params_iter, row_to_track)?;
        for r in rows {
            out.push(r?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::test_helpers::create_test_db;
    use tempfile::NamedTempFile;

    fn make_db() -> (tempfile::TempPath, Connection) {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.into_temp_path();
        let conn = create_test_db(&path).unwrap();
        conn.execute_batch(include_str!("../sql/schema.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../sql/seed.sql")).unwrap();
        (path, conn)
    }

    #[test]
    fn all_returns_non_deleted_tracks() {
        let (_path, conn) = make_db();
        let tracks = all(&conn).unwrap();
        assert_eq!(tracks.len(), 3, "seed has 3 live tracks");
        assert!(tracks.iter().all(|t| !t.id.is_empty()));
    }

    #[test]
    fn by_id_found() {
        let (_path, conn) = make_db();
        let t = by_id(&conn, "1").unwrap();
        assert!(t.is_some());
        assert_eq!(t.unwrap().title, "Test Track Alpha");
    }

    #[test]
    fn by_id_not_found() {
        let (_path, conn) = make_db();
        assert!(by_id(&conn, "9999").unwrap().is_none());
    }

    #[test]
    fn search_by_title() {
        let (_path, conn) = make_db();
        let results = search(&conn, "beta").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test Track Beta");
    }

    #[test]
    fn bpm_converted_from_integer_x100() {
        let (_path, conn) = make_db();
        let tracks = all(&conn).unwrap();
        let alpha = tracks
            .iter()
            .find(|t| t.title == "Test Track Alpha")
            .unwrap();
        // seed.sql inserts BPM = 13200 → 132.00
        assert!((alpha.bpm.unwrap() - 132.0).abs() < 0.001);
    }

    #[test]
    fn added_since_filters_by_date_created() {
        let (_path, conn) = make_db();
        let recent = added_since(&conn, "2025-12-31T00:00:00Z").unwrap();
        // Gamma (2026-05-19) is the only live track strictly after the watermark.
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].title, "Test Track Gamma");
    }

    #[test]
    fn by_ids_returns_requested_tracks() {
        let (_path, conn) = make_db();
        let got = by_ids(&conn, &["1".into(), "3".into()]).unwrap();
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn deleted_tracks_excluded() {
        let (_path, conn) = make_db();
        let tracks = all(&conn).unwrap();
        assert!(tracks.iter().all(|t| t.title != "Deleted Track"));
    }

    fn by_title<'a>(tracks: &'a [Track], title: &str) -> &'a Track {
        tracks.iter().find(|t| t.title == title).unwrap()
    }

    #[test]
    fn label_remixer_and_mix_come_back_joined() {
        let (_path, conn) = make_db();
        let tracks = all(&conn).unwrap();
        let alpha = by_title(&tracks, "Test Track Alpha");
        assert_eq!(alpha.label.as_deref(), Some("Drumcode"));
        assert_eq!(alpha.mix.as_deref(), Some("Extended Mix"));
        // RemixerID 2 is the second seeded artist, not the track's own artist.
        assert_eq!(alpha.remixer.as_deref(), Some("Artist Two"));
        assert_ne!(alpha.remixer, alpha.artist);
    }

    #[test]
    fn colour_reads_from_commnt_and_falls_back_to_name() {
        // `djmdColor` keeps the colour name in `Commnt`. The seed leaves `Name`
        // null on one row and `Commnt` null on the other, so both halves of the
        // COALESCE are exercised — this is the quirk most likely to regress.
        let (_path, conn) = make_db();
        let tracks = all(&conn).unwrap();
        assert_eq!(
            by_title(&tracks, "Test Track Alpha").color.as_deref(),
            Some("Red")
        );
        assert_eq!(
            by_title(&tracks, "Test Track Beta").color.as_deref(),
            Some("Blue")
        );
    }

    #[test]
    fn a_track_with_none_of_the_new_fields_is_still_returned() {
        let (_path, conn) = make_db();
        let tracks = all(&conn).unwrap();
        let gamma = by_title(&tracks, "Test Track Gamma");
        assert_eq!(gamma.label, None);
        assert_eq!(gamma.remixer, None);
        assert_eq!(gamma.mix, None);
        assert_eq!(gamma.color, None);
    }

    #[test]
    fn date_added_is_the_stored_string_not_a_reformatted_one() {
        // The column's format varies between libraries; reformatting it here
        // would lose information we cannot get back.
        let (_path, conn) = make_db();
        let tracks = all(&conn).unwrap();
        assert_eq!(
            by_title(&tracks, "Test Track Alpha").date_added.as_deref(),
            Some("2025-01-01T00:00:00Z")
        );
    }

    /// The guard that makes this change safe to ship.
    ///
    /// A library predating these columns must keep returning every track. If
    /// the SELECT named `LabelID` unconditionally, this query would fail
    /// outright and the browser would show an empty library — a far worse
    /// outcome than a missing Label column.
    #[test]
    fn a_library_without_the_new_columns_still_returns_its_tracks() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE djmdArtist (ID TEXT PRIMARY KEY, Name TEXT);
            CREATE TABLE djmdAlbum  (ID TEXT PRIMARY KEY, Name TEXT);
            CREATE TABLE djmdGenre  (ID TEXT PRIMARY KEY, Name TEXT);
            CREATE TABLE djmdKey    (ID TEXT PRIMARY KEY, ScaleName TEXT);
            CREATE TABLE djmdContent (
                ID TEXT PRIMARY KEY, Title TEXT, ArtistID TEXT, AlbumID TEXT,
                GenreID TEXT, KeyID TEXT, BPM INTEGER, Length INTEGER,
                Rating INTEGER, Commnt TEXT, FolderPath TEXT,
                AnalysisDataPath TEXT, FileType INTEGER, SampleRate INTEGER,
                BitRate INTEGER, ReleaseYear INTEGER, DJPlayCount INTEGER,
                rb_local_deleted INTEGER DEFAULT 0
            );
            INSERT INTO djmdArtist (ID, Name) VALUES ('1', 'Old Artist');
            INSERT INTO djmdContent (ID, Title, ArtistID, BPM, rb_local_deleted)
            VALUES ('1', 'Ancient Track', '1', 12800, 0);
            ",
        )
        .unwrap();

        let tracks = all(&conn).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Ancient Track");
        assert_eq!(tracks[0].label, None);
        assert_eq!(tracks[0].color, None);
        assert_eq!(tracks[0].date_added, None);
    }
}
