//! MyTags — Rekordbox's own tag system, which `decks` imports into Custom Tags.
//!
//! Per `docs/lexicon/02-library.md §Custom Tags`: "Rekordbox 6/7 MyTags import
//! automatically."
//!
//! Rekordbox stores categories and tags in the **same** table, distinguished by
//! `Attribute` and linked by `ParentID` — the same self-referencing shape as
//! playlists and folders. A category's parent is the root; a tag's parent is a
//! category.
//!
//! Read-only, like everything else that touches `master.db`.

use anyhow::Result;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

/// `djmdMyTag.Attribute` for a category row.
const ATTRIBUTE_CATEGORY: i64 = 0;

/// One MyTag category, with the tags beneath it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyTagCategory {
    pub id: String,
    pub name: String,
    pub tags: Vec<MyTag>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyTag {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawMyTag {
    id: String,
    name: String,
    attribute: i64,
    parent_id: Option<String>,
    seq: Option<i64>,
}

fn read_row(row: &Row) -> rusqlite::Result<RawMyTag> {
    Ok(RawMyTag {
        id: row.get(0)?,
        name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        attribute: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
        parent_id: row.get(3)?,
        seq: row.get(4)?,
    })
}

/// Every MyTag category and its tags, in Rekordbox's own order.
///
/// Soft-deleted rows are skipped on **both** levels: a deleted tag disappears,
/// and a deleted category takes its tags with it — importing tags whose
/// category the user has thrown away would recreate exactly what they removed.
///
/// A category with no tags is still returned. It is a real thing the user made,
/// and dropping it would silently lose structure on import.
pub fn my_tags(conn: &Connection) -> Result<Vec<MyTagCategory>> {
    let mut stmt = conn.prepare(
        "SELECT ID, Name, Attribute, ParentID, Seq
           FROM djmdMyTag
          WHERE COALESCE(rb_local_deleted, 0) = 0
          ORDER BY COALESCE(Seq, 0), Name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], read_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let categories: Vec<&RawMyTag> = rows
        .iter()
        .filter(|r| r.attribute == ATTRIBUTE_CATEGORY)
        .collect();

    Ok(categories
        .iter()
        .map(|cat| MyTagCategory {
            id: cat.id.clone(),
            name: cat.name.clone(),
            tags: rows
                .iter()
                .filter(|r| {
                    r.attribute != ATTRIBUTE_CATEGORY
                        && r.parent_id.as_deref() == Some(cat.id.as_str())
                })
                .map(|t| MyTag {
                    id: t.id.clone(),
                    name: t.name.clone(),
                })
                .collect(),
        })
        .collect())
}

/// Track id → the MyTag ids applied to it.
///
/// Batched rather than per-track: an import walks the whole library, and a
/// query per track over a few thousand tracks is the difference between an
/// import that feels instant and one that does not.
///
/// Links to soft-deleted tags are skipped, so a tag the user removed in
/// Rekordbox does not come back through the join.
pub fn my_tags_by_track(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT s.ContentID, s.MyTagID
           FROM djmdSongMyTag s
           JOIN djmdMyTag t ON t.ID = s.MyTagID
          WHERE COALESCE(s.rb_local_deleted, 0) = 0
            AND COALESCE(t.rb_local_deleted, 0) = 0
            AND s.ContentID IS NOT NULL
          ORDER BY s.ContentID",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../sql/schema.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../sql/seed.sql")).unwrap();
        conn
    }

    #[test]
    fn categories_come_back_with_their_tags_nested() {
        let got = my_tags(&seeded()).unwrap();
        let names: Vec<&str> = got.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Genre", "Vocals"]);

        let genre = &got[0];
        assert_eq!(
            genre
                .tags
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Techno", "House"]
        );
    }

    #[test]
    fn a_soft_deleted_tag_is_not_imported() {
        // "Deleted Tag" is `rb_local_deleted = 1`; importing it would recreate
        // exactly what the user removed in Rekordbox.
        let got = my_tags(&seeded()).unwrap();
        let all: Vec<&str> = got
            .iter()
            .flat_map(|c| c.tags.iter().map(|t| t.name.as_str()))
            .collect();
        assert!(!all.contains(&"Deleted Tag"), "got {all:?}");
    }

    #[test]
    fn a_category_with_no_tags_still_comes_back() {
        // It is a real thing the user made; dropping it loses structure.
        let conn = seeded();
        conn.execute_batch(
            "INSERT INTO djmdMyTag (ID, Seq, Name, Attribute, ParentID, rb_local_deleted)
             VALUES ('mt-empty', 9, 'Empty', 0, 'root', 0);",
        )
        .unwrap();
        let got = my_tags(&conn).unwrap();
        let empty = got.iter().find(|c| c.name == "Empty").unwrap();
        assert!(empty.tags.is_empty());
    }

    #[test]
    fn tags_are_ordered_by_rekordbox_seq() {
        // House has Seq 2 and sorts after Techno despite being alphabetically
        // first — the user's own ordering is the one that means something.
        let got = my_tags(&seeded()).unwrap();
        assert_eq!(got[0].tags[0].name, "Techno");
    }

    #[test]
    fn track_links_skip_deleted_tags_and_deleted_links() {
        let pairs = my_tags_by_track(&seeded()).unwrap();
        // smt4 points at the deleted tag; smt5 is itself deleted.
        assert!(!pairs.iter().any(|(_, tag)| tag == "mt-gone"));
        assert!(!pairs.contains(&("3".to_string(), "mt-techno".to_string())));
    }

    #[test]
    fn a_track_can_carry_tags_from_more_than_one_category() {
        let pairs = my_tags_by_track(&seeded()).unwrap();
        let for_one: Vec<&str> = pairs
            .iter()
            .filter(|(track, _)| track == "1")
            .map(|(_, tag)| tag.as_str())
            .collect();
        assert_eq!(for_one, vec!["mt-techno", "mt-novox"]);
    }
}
