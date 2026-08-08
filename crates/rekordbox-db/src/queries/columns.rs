//! Probing a real `master.db` for the columns it actually has.
//!
//! Rekordbox's schema is not stable across versions, and the libraries people
//! actually own have been migrated forward through several of them. A `SELECT`
//! naming a column that is absent fails the *whole* query, so a query that
//! reaches for a newer field must ask first rather than assume.
//!
//! `cues` needed this first, for `djmdCue`'s renamed columns. `tracks` needs it
//! for the opposite reason: the fields it reads are ones older libraries may not
//! carry at all, and losing every track read to gain a Label column would be a
//! bad trade.

use anyhow::Result;
use rusqlite::Connection;

/// Every column name on `table`, in declaration order.
///
/// An unknown table yields an empty list rather than an error — `PRAGMA
/// table_info` on a missing table returns no rows, and "no columns" is exactly
/// the answer callers want in that case.
pub fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", quote_ident(table)))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// The first candidate that exists, quoted and ready to paste into SQL.
///
/// Case-insensitive, because Rekordbox is inconsistent about it across
/// versions (`ContentID` vs `ContentId`), and SQLite would happily accept
/// either — but only if we spell it the way the table does.
pub fn pick_column(columns: &[String], candidates: &[&str]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        columns
            .iter()
            .find(|column| column.eq_ignore_ascii_case(candidate))
            .map(|column| quote_ident(column))
    })
}

/// A qualified `alias.column` expression, or `NULL` when the column is absent.
///
/// The `NULL` fallback is the point: a library too old to carry `LabelID` still
/// reads, and the field simply comes back empty. That is honest — we do not
/// know the label — and it is what ADR-0008 asks for over inventing a value.
pub fn optional_column(columns: &[String], alias: &str, candidates: &[&str]) -> String {
    match pick_column(columns, candidates) {
        Some(column) => format!("{alias}.{column}"),
        None => "NULL".to_owned(),
    }
}

/// True when `table` exists in this database.
///
/// Joins have to be omitted entirely for a missing table, not just their
/// columns — `LEFT JOIN djmdLabel` against a database without one is a hard
/// error however carefully the SELECT list is written.
pub fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

pub fn missing_column(table: &str, columns: &[String], label: &str) -> anyhow::Error {
    let available = if columns.is_empty() {
        "none".to_owned()
    } else {
        columns.join(", ")
    };
    anyhow::anyhow!("{table} has no {label} column; available columns: {available}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_with(sql: &str) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(sql).unwrap();
        conn
    }

    #[test]
    fn columns_come_back_in_declaration_order() {
        let conn = db_with("CREATE TABLE t (a TEXT, b TEXT, c TEXT);");
        assert_eq!(table_columns(&conn, "t").unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn a_missing_table_has_no_columns_rather_than_erroring() {
        let conn = db_with("CREATE TABLE t (a TEXT);");
        assert!(table_columns(&conn, "nope").unwrap().is_empty());
    }

    #[test]
    fn candidates_match_regardless_of_case() {
        // Rekordbox spells this `ContentID` in some versions and `ContentId` in
        // others; SQLite accepts either spelling but only if the column is there.
        let columns = vec!["ContentId".to_owned()];
        assert_eq!(
            pick_column(&columns, &["ContentID"]),
            Some("\"ContentId\"".to_owned())
        );
    }

    #[test]
    fn the_first_matching_candidate_wins() {
        let columns = vec!["Type".to_owned(), "Kind".to_owned()];
        assert_eq!(
            pick_column(&columns, &["Kind", "Type"]),
            Some("\"Kind\"".to_owned())
        );
    }

    #[test]
    fn an_absent_column_becomes_null_not_an_error() {
        // The whole reason this module exists: a library without `LabelID`
        // must still return its tracks.
        let columns = vec!["Title".to_owned()];
        assert_eq!(optional_column(&columns, "c", &["LabelID"]), "NULL");
        assert_eq!(optional_column(&columns, "c", &["Title"]), "c.\"Title\"");
    }

    #[test]
    fn table_existence_is_reported_without_querying_the_table() {
        let conn = db_with("CREATE TABLE djmdLabel (ID TEXT);");
        assert!(table_exists(&conn, "djmdLabel").unwrap());
        assert!(!table_exists(&conn, "djmdColor").unwrap());
    }

    #[test]
    fn identifiers_with_quotes_are_escaped() {
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }
}
