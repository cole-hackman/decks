/// Schema migrations. Each entry is (target_version, sql).
/// Versions are sequential from 1; the migration at index i brings the DB
/// from version i to version i+1.
pub const MIGRATIONS: &[(u32, &str)] = &[
    // v0 → v1: initial schema
    (
        1,
        "
        CREATE TABLE IF NOT EXISTS audio_features (
            track_uri       TEXT    NOT NULL,
            analyzer_version TEXT   NOT NULL,
            bpm             REAL,
            musical_key     TEXT,
            energy          REAL,
            features_json   TEXT,
            created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (track_uri, analyzer_version)
        );
        ",
    ),
    (
        2,
        "
        CREATE TABLE IF NOT EXISTS conversations (
            id           TEXT PRIMARY KEY,
            library_path TEXT,
            title        TEXT NOT NULL,
            created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
            updated_at   INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE TABLE IF NOT EXISTS conversation_messages (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role            TEXT NOT NULL,
            content_json    TEXT NOT NULL,
            created_at      INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE INDEX IF NOT EXISTS idx_conversation_messages_conversation
            ON conversation_messages(conversation_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_conversations_library_updated
            ON conversations(library_path, updated_at DESC);
        ",
    ),
    (
        3,
        "
        CREATE TABLE IF NOT EXISTS staged_changes (
            id           TEXT PRIMARY KEY,
            library_path TEXT,
            kind         TEXT NOT NULL,
            target_id    TEXT,
            field        TEXT,
            old_value    TEXT,
            new_value    TEXT,
            reason       TEXT,
            confidence   REAL,
            status       TEXT NOT NULL,
            created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
            updated_at   INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE INDEX IF NOT EXISTS idx_staged_changes_library_status
            ON staged_changes(library_path, status, updated_at DESC);
        ",
    ),
    (
        4,
        "
        CREATE TABLE IF NOT EXISTS audio_fingerprints (
            track_uri       TEXT    PRIMARY KEY,
            chroma_hash     BLOB    NOT NULL,
            created_at      INTEGER NOT NULL DEFAULT (unixepoch())
        );
        ",
    ),
    (
        5,
        "
        -- Custom Tags (Feature 1)
        CREATE TABLE tag_categories (
          id   TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          seq  INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE tags (
          id          TEXT PRIMARY KEY,
          category_id TEXT NOT NULL REFERENCES tag_categories(id) ON DELETE CASCADE,
          name        TEXT NOT NULL,
          seq         INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE track_tags (
          library_path TEXT NOT NULL,
          track_id     TEXT NOT NULL,
          tag_id       TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
          PRIMARY KEY (library_path, track_id, tag_id)
        );
        CREATE INDEX idx_track_tags_tag ON track_tags(tag_id);

        -- Incoming / Archive (Feature 5)
        CREATE TABLE incoming_watermark (
          library_path TEXT PRIMARY KEY,
          cleared_at   INTEGER NOT NULL
        );
        CREATE TABLE archived_tracks (
          library_path TEXT NOT NULL,
          track_id     TEXT NOT NULL,
          archived_at  INTEGER NOT NULL,
          PRIMARY KEY (library_path, track_id)
        );

        -- Smart Fixes / Sync config (Features 3 & 4)
        CREATE TABLE common_text_blocklist (
          id      INTEGER PRIMARY KEY AUTOINCREMENT,
          pattern TEXT NOT NULL UNIQUE
        );
        CREATE TABLE field_mappings (
          library_path TEXT NOT NULL,
          source_field TEXT NOT NULL,
          target_column TEXT NOT NULL,
          PRIMARY KEY (library_path, source_field)
        );
        CREATE TABLE sync_runs (
          id           TEXT PRIMARY KEY,
          library_path TEXT NOT NULL,
          mode         TEXT NOT NULL,
          tracks_written INTEGER NOT NULL,
          errors_json  TEXT,
          backup_path  TEXT,
          ran_at       INTEGER NOT NULL
        );
        ",
    ),
    (
        6,
        "
        CREATE TABLE waveform_peaks (
          track_uri    TEXT PRIMARY KEY,
          peaks        BLOB NOT NULL,
          sample_count INTEGER NOT NULL,
          generated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        ",
    ),
    (
        7,
        "
        -- Smartlists (Epic 1). Rules are stored as a JSON document rather than
        -- normalised into clause/rule tables: evaluation is in-memory (ADR-0013),
        -- so no query ever filters on an individual rule, and `staged_changes`
        -- already sets the precedent for JSON payloads in this database.
        --
        -- `parent_folder_id` doubles as the generator ledger. Generated
        -- smartlists live in the reserved `Lexicon` folder; moving one out (by
        -- changing this column) is what makes the generator recreate it.
        CREATE TABLE smartlists (
          id               TEXT PRIMARY KEY,
          library_path     TEXT NOT NULL,
          name             TEXT NOT NULL,
          parent_folder_id TEXT,
          combinator       TEXT NOT NULL,
          clauses_json     TEXT NOT NULL,
          seq              INTEGER NOT NULL DEFAULT 0,
          created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
          updated_at       INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE INDEX idx_smartlists_library ON smartlists(library_path, seq, name);
        ",
    ),
    (
        8,
        "
        -- Local Path Mappings (Epic 4). Per-computer prefix rewrites, so a
        -- database restored on a second machine finds its music without a bulk
        -- relocate.
        --
        -- Deliberately NOT keyed by library_path: the mapping describes where
        -- this *computer* keeps its music, and it must apply the moment a
        -- library is opened, before anything has been recorded against that
        -- library's path. Never staged, exported or synced.
        CREATE TABLE path_mappings (
          id           TEXT PRIMARY KEY,
          from_prefix  TEXT NOT NULL,
          to_prefix    TEXT NOT NULL,
          seq          INTEGER NOT NULL DEFAULT 0,
          created_at   INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE INDEX idx_path_mappings_seq ON path_mappings(seq);
        ",
    ),
    (
        9,
        "
        -- Quick move destinations (Epic 4). Recently-used folders, with a
        -- favourite flag; favourites get hotkeys 1-9 in the picker.
        --
        -- Like path_mappings and for the same reason, not keyed by
        -- library_path: 'the folder I file house tracks into' is a fact about
        -- this computer's disk, not about one Rekordbox database.
        CREATE TABLE quick_move_folders (
          id           TEXT PRIMARY KEY,
          path         TEXT NOT NULL UNIQUE,
          favourite    INTEGER NOT NULL DEFAULT 0,
          last_used_at INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE INDEX idx_quick_move_recent ON quick_move_folders(favourite DESC, last_used_at DESC);
        ",
    ),
];

pub fn current_version(conn: &rusqlite::Connection) -> anyhow::Result<u32> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

pub fn set_version(conn: &rusqlite::Connection, v: u32) -> anyhow::Result<()> {
    conn.execute_batch(&format!("PRAGMA user_version = {v};"))?;
    Ok(())
}

pub fn run(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let mut version = current_version(conn)?;
    for &(target, sql) in MIGRATIONS {
        if version < target {
            conn.execute_batch(sql)?;
            set_version(conn, target)?;
            version = target;
            tracing::debug!("cache DB migrated to schema v{target}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrations_run_idempotently() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        let v1 = current_version(&conn).unwrap();
        // Running again must be a no-op.
        run(&conn).unwrap();
        let v2 = current_version(&conn).unwrap();
        assert_eq!(v1, v2);
        assert!(v1 >= 1);
    }

    #[test]
    fn schema_version_increases() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(current_version(&conn).unwrap(), 0);
        run(&conn).unwrap();
        assert_eq!(
            current_version(&conn).unwrap(),
            MIGRATIONS.last().unwrap().0
        );
    }

    #[test]
    fn audio_features_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        // Should not error.
        conn.execute_batch(
            "INSERT INTO audio_features (track_uri, analyzer_version, bpm)
             VALUES ('file:///test.mp3', 'v1', 128.0);",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audio_features", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn conversation_tables_exist_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO conversations (id, library_path, title)
             VALUES ('c1', '/db', 'Test');
             INSERT INTO conversation_messages (id, conversation_id, role, content_json)
             VALUES ('m1', 'c1', 'user', '{\"text\":\"hello\"}');",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM conversation_messages", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn audio_fingerprints_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO audio_fingerprints (track_uri, chroma_hash)
             VALUES ('file:///test.mp3', x'00112233');",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audio_fingerprints", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn staged_changes_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO staged_changes
                (id, library_path, kind, target_id, field, old_value, new_value, status)
             VALUES
                ('ch1', '/db', 'TrackMetadataEdit', 't1', 'genre', '\"House\"', '\"Deep House\"', 'Proposed');",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM staged_changes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn quick_move_folders_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute(
            "INSERT INTO quick_move_folders (id, path) VALUES ('q1', '/Music/House')",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM quick_move_folders", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn path_mappings_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute(
            "INSERT INTO path_mappings (id, from_prefix, to_prefix)
             VALUES ('m1', 'D:\\Music', '/Users/me/Music')",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM path_mappings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn smartlists_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO smartlists
                (id, library_path, name, parent_folder_id, combinator, clauses_json)
             VALUES
                ('s1', '/db', 'Peak time', 'Lexicon', 'all', '[]');",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM smartlists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
