use crate::queries::columns::{missing_column, pick_column, table_columns};
use crate::types::{CueKind, HotCue};
use anyhow::Result;
use rusqlite::{params, Connection};

struct CueSelect {
    select_sql: String,
    content_expr: String,
    in_expr: String,
}

fn row_to_cue(row: &rusqlite::Row<'_>) -> rusqlite::Result<HotCue> {
    let kind_raw = row.get::<_, Option<i64>>(4)?.unwrap_or(0);
    Ok(HotCue {
        id: row.get(0)?,
        content_id: row.get(1)?,
        in_msec: row.get(2)?,
        out_msec: row.get(3)?,
        kind: CueKind::from_db(kind_raw),
        color: row.get(5)?,
        comment: row.get(6)?,
    })
}

pub fn for_track(conn: &Connection, content_id: &str) -> Result<Vec<HotCue>> {
    let cue_select = cue_select(conn)?;
    let sql = format!(
        "{} WHERE {} = ?1 ORDER BY {}",
        cue_select.select_sql, cue_select.content_expr, cue_select.in_expr
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![content_id], row_to_cue)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn all(conn: &Connection) -> Result<Vec<HotCue>> {
    let cue_select = cue_select(conn)?;
    let sql = format!(
        "{} ORDER BY {}, {}",
        cue_select.select_sql, cue_select.content_expr, cue_select.in_expr
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_cue)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Return the distinct set of track/content IDs that have at least one cue.
/// Used by the UI to power a "has cues / no cues" filter without fetching
/// the full cue rows.
pub fn track_ids_with_cues(conn: &Connection) -> Result<Vec<String>> {
    let cue_select = cue_select(conn)?;
    let sql = format!(
        "SELECT DISTINCT {} FROM djmdCue WHERE {} IS NOT NULL",
        cue_select.content_expr, cue_select.content_expr
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn cue_select(conn: &Connection) -> Result<CueSelect> {
    let columns = table_columns(conn, "djmdCue")?;
    let content_expr = pick_column(&columns, &["ContentID", "ContentId", "TrackID", "TrackId"])
        .ok_or_else(|| missing_column("djmdCue", &columns, "track/content id"))?;
    let in_expr = pick_column(&columns, &["InMsec", "InMS", "InMs"])
        .ok_or_else(|| missing_column("djmdCue", &columns, "cue start time"))?;

    let id_expr = pick_column(&columns, &["ID", "Id", "CueID", "CueId"])
        .unwrap_or_else(|| "CAST(rowid AS TEXT)".to_owned());
    let out_expr =
        pick_column(&columns, &["OutMsec", "OutMS", "OutMs"]).unwrap_or_else(|| "NULL".to_owned());
    let kind_expr = pick_column(&columns, &["Kind", "Type"]).unwrap_or_else(|| "0".to_owned());
    let color_expr = pick_column(&columns, &["Color", "ColorID", "ColorId"])
        .unwrap_or_else(|| "NULL".to_owned());
    let comment_expr =
        pick_column(&columns, &["Commnt", "Comment", "Name"]).unwrap_or_else(|| "NULL".to_owned());

    Ok(CueSelect {
        select_sql: format!(
            "SELECT {id_expr} AS id, {content_expr} AS content_id, \
             {in_expr} AS in_msec, {out_expr} AS out_msec, \
             {kind_expr} AS kind, {color_expr} AS color, {comment_expr} AS comment \
             FROM djmdCue"
        ),
        content_expr,
        in_expr,
    })
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
    fn hot_cues_for_track() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        // seed gives track "1" two cues
        let cues = for_track(&conn, "1").unwrap();
        assert_eq!(cues.len(), 2);
        // ordered by InMsec ascending
        assert!(cues[0].in_msec <= cues[1].in_msec);
    }

    #[test]
    fn cue_kinds() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let cues = for_track(&conn, "1").unwrap();
        assert_eq!(cues[0].kind, CueKind::MemoryCue);
        assert_eq!(cues[1].kind, CueKind::HotCue(1));
    }

    #[test]
    fn all_cues_count() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        // seed inserts 3 cues total
        let cues = all(&conn).unwrap();
        assert_eq!(cues.len(), 3);
    }

    #[test]
    fn no_cues_for_unknown_track() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        assert!(for_track(&conn, "9999").unwrap().is_empty());
    }

    #[test]
    fn track_ids_with_cues_distinct() {
        let path = make_db();
        let conn = create_test_db(&path).unwrap();
        let ids = track_ids_with_cues(&conn).unwrap();
        // Result must be non-empty and distinct.
        assert!(!ids.is_empty(), "seed should have at least one cue");
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "result must be distinct");
        // Track "1" has cues in the seed.
        assert!(ids.iter().any(|id| id == "1"));
    }

    #[test]
    fn supports_real_library_cue_column_variants() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE djmdCue (
                Id TEXT,
                TrackID TEXT,
                InMS INTEGER,
                OutMS INTEGER,
                Type INTEGER,
                ColorID INTEGER,
                Comment TEXT
            );
            INSERT INTO djmdCue
                (Id, TrackID, InMS, OutMS, Type, ColorID, Comment)
            VALUES
                ('cue-1', 'track-1', 12345, NULL, 0, 2, 'Alt schema cue');
            ",
        )
        .unwrap();

        let cues = for_track(&conn, "track-1").unwrap();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].id, "cue-1");
        assert_eq!(cues[0].content_id, "track-1");
        assert_eq!(cues[0].in_msec, Some(12345));
        assert_eq!(cues[0].comment.as_deref(), Some("Alt schema cue"));
    }
}
