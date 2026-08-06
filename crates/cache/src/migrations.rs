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
    (
        10,
        "
        -- Watch folders (Epic 4) and the files the user has finished with.
        --
        -- `watch_dismissed` is what stops a file the user chose not to import
        -- from being offered again on every scan. Keyed on a normalised path
        -- (lower-cased, forward slashes) for the same reason the unused-file
        -- sweep normalises: the filesystem and the library do not reliably
        -- agree on case.
        CREATE TABLE watch_folders (
          id         TEXT PRIMARY KEY,
          path       TEXT NOT NULL UNIQUE,
          created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE TABLE watch_dismissed (
          path_key     TEXT PRIMARY KEY,
          path         TEXT NOT NULL,
          dismissed_at INTEGER NOT NULL DEFAULT (unixepoch())
        );
        ",
    ),
    (
        11,
        "
        -- Field Mappings (Epic 4). Replaces the `field_mappings` table from v5,
        -- which was never read or written by anything and could not express the
        -- feature: its (library_path, source_field) primary key allows one
        -- target per source, while the spec requires *several sources per
        -- target* combining into one value.
        --
        -- Scoped by `profile` rather than library_path: mappings are configured
        -- separately per DJ app and again for ID3 tag writing, which is a
        -- property of the destination, not of one database.
        CREATE TABLE field_mapping_rules (
          id         TEXT PRIMARY KEY,
          profile    TEXT NOT NULL,
          source_json TEXT NOT NULL,
          target     TEXT NOT NULL,
          overwrite  INTEGER NOT NULL DEFAULT 0,
          seq        INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_field_mapping_profile ON field_mapping_rules(profile, seq);

        DROP TABLE IF EXISTS field_mappings;
        ",
    ),
    (
        12,
        "
        -- Per-track incoming review state (Epic 4).
        --
        -- The existing `incoming_watermark` is all-or-nothing: it answers 'what
        -- arrived since I last cleared', which cannot express 'I have dealt
        -- with these three'. Marking one track done must not hide the rest, so
        -- reviewed tracks are recorded individually and filtered out alongside
        -- archived ones.
        CREATE TABLE incoming_reviewed (
          library_path TEXT NOT NULL,
          track_id     TEXT NOT NULL,
          reviewed_at  INTEGER NOT NULL DEFAULT (unixepoch()),
          PRIMARY KEY (library_path, track_id)
        );
        ",
    ),
    (
        13,
        "
        -- Undo history (Epic 5).
        --
        -- One row per Sync run, plus the inverse of every change that run
        -- applied. The inverses are computed and stored at apply time rather
        -- than derived on demand, because `staged_changes` rows can be cleared
        -- and a run has to stay undoable after its originals are gone.
        --
        -- Lexicon drops its undo history after 60 minutes or on restart. We
        -- keep it: the cache is already persistent, and a DJ who notices a bad
        -- sync the next morning has more use for an undo than one who notices
        -- within the hour. Bounded by pruning, not by a clock — see
        -- `prune_undo_runs`.
        CREATE TABLE undo_runs (
          id           TEXT PRIMARY KEY,
          library_path TEXT NOT NULL,
          applied_at   INTEGER NOT NULL DEFAULT (unixepoch()),
          -- Set when the run's inverses have been staged, so a run cannot be
          -- undone twice into a double-staged pile.
          undone_at    INTEGER
        );

        -- `blocked_reason IS NULL` is the reversible half. The unreversible
        -- entries are stored too, so the UI can say what an undo will *not*
        -- put back rather than silently restoring a subset.
        CREATE TABLE undo_entries (
          id               TEXT PRIMARY KEY,
          run_id           TEXT NOT NULL REFERENCES undo_runs(id) ON DELETE CASCADE,
          seq              INTEGER NOT NULL,
          source_change_id TEXT NOT NULL,
          kind             TEXT,
          target_id        TEXT,
          field            TEXT,
          old_value        TEXT,
          new_value        TEXT,
          description      TEXT NOT NULL,
          blocked_reason   TEXT
        );

        CREATE INDEX idx_undo_runs_library ON undo_runs(library_path, applied_at DESC);
        CREATE INDEX idx_undo_entries_run ON undo_entries(run_id, seq);
        ",
    ),
    (
        14,
        "
        -- Genre / Artist Cleanup state (Epic 5).
        --
        -- A lock marks a value the user has decided is correct, so a stray
        -- Cmd+A or shift-click cannot sweep it into a rename. Scoped by `kind`
        -- ('genre' | 'artist') because the same string can be a good genre and
        -- a misspelt artist.
        --
        -- Not scoped by library_path: a value the user has declared canonical
        -- is canonical for them, not for one database, and re-locking the same
        -- fifty genres per library would defeat the point.
        CREATE TABLE cleanup_locks (
          kind  TEXT NOT NULL,
          value TEXT NOT NULL,
          PRIMARY KEY (kind, value)
        );

        -- Pinned letters for alphabet navigation, persisted across sessions
        -- per the spec.
        CREATE TABLE cleanup_pinned_letters (
          kind   TEXT NOT NULL,
          letter TEXT NOT NULL,
          PRIMARY KEY (kind, letter)
        );
        ",
    ),
    (
        15,
        "
        -- Mixable Tracks option sets (Epic 6).
        --
        -- The spec calls these templates: 'option sets are saveable and
        -- reusable'. Stored as a JSON document rather than a column per rule,
        -- for the same reason smartlists are (v7): nothing queries an
        -- individual rule, the set is always loaded whole, and a rule added
        -- later must not require a migration.
        --
        -- Not scoped by library_path. 'my peak-time rules' is a statement about
        -- how someone mixes, not about one database, and re-entering it per
        -- library would defeat the point — same reasoning as cleanup_locks.
        CREATE TABLE mixable_templates (
          id         TEXT PRIMARY KEY,
          name       TEXT NOT NULL UNIQUE,
          options    TEXT NOT NULL,
          created_at INTEGER NOT NULL
        );
        ",
    ),
    (
        16,
        "
        -- Favourite playlists (Epic 6).
        --
        -- Starred playlists pin above the track browser as a fast filing
        -- system; `seq` is the hotkey position, so favourite 1 is always the
        -- same playlist between sessions. A hotkey that moves when the list
        -- re-sorts is worse than no hotkey.
        --
        -- Scoped by library_path, unlike cleanup_locks and mixable_templates:
        -- a playlist id only means anything inside the database it came from.
        CREATE TABLE favourite_playlists (
          library_path TEXT NOT NULL,
          playlist_id  TEXT NOT NULL,
          seq          INTEGER NOT NULL,
          PRIMARY KEY (library_path, playlist_id)
        );
        CREATE INDEX idx_favourite_playlists ON favourite_playlists(library_path, seq);
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
    fn incoming_reviewed_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute(
            "INSERT INTO incoming_reviewed (library_path, track_id) VALUES ('/db', 't1')",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM incoming_reviewed", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn field_mapping_rules_replace_the_dead_v5_table() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute(
            "INSERT INTO field_mapping_rules (id, profile, source_json, target, overwrite, seq)
             VALUES ('r1', 'id3', '{\"kind\":\"energy\"}', 'Comment', 1, 0)",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM field_mapping_rules", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // The v5 table is gone: nothing ever read or wrote it, and its
        // one-target-per-source key cannot express combining.
        let dead: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='field_mappings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dead, 0);
    }

    #[test]
    fn watch_tables_exist_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO watch_folders (id, path) VALUES ('w1', '/Music/Watch');
             INSERT INTO watch_dismissed (path_key, path)
                VALUES ('/music/watch/a.mp3', '/Music/Watch/a.mp3');",
        )
        .unwrap();
        let folders: i64 = conn
            .query_row("SELECT COUNT(*) FROM watch_folders", [], |r| r.get(0))
            .unwrap();
        let dismissed: i64 = conn
            .query_row("SELECT COUNT(*) FROM watch_dismissed", [], |r| r.get(0))
            .unwrap();
        assert_eq!((folders, dismissed), (1, 1));
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
    fn cleanup_tables_exist_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO cleanup_locks (kind, value) VALUES ('genre', 'Ambient');
             INSERT INTO cleanup_pinned_letters (kind, letter) VALUES ('artist', 'D');",
        )
        .unwrap();
        let locks: i64 = conn
            .query_row("SELECT COUNT(*) FROM cleanup_locks", [], |r| r.get(0))
            .unwrap();
        let letters: i64 = conn
            .query_row("SELECT COUNT(*) FROM cleanup_pinned_letters", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!((locks, letters), (1, 1));
    }

    #[test]
    fn undo_tables_exist_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO undo_runs (id, library_path, applied_at)
                VALUES ('r1', '/lib.db', 100);
             INSERT INTO undo_entries
                (id, run_id, seq, source_change_id, kind, description)
                VALUES ('e1', 'r1', 0, 'c1', 'TrackMetadataEdit', 'Title: a → b');",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM undo_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn deleting_an_undo_run_takes_its_entries_with_it() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute_batch(
            "INSERT INTO undo_runs (id, library_path, applied_at)
                VALUES ('r1', '/lib.db', 100);
             INSERT INTO undo_entries
                (id, run_id, seq, source_change_id, description)
                VALUES ('e1', 'r1', 0, 'c1', 'x');
             DELETE FROM undo_runs WHERE id = 'r1';",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM undo_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
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

    #[test]
    fn favourite_playlists_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO favourite_playlists (library_path, playlist_id, seq)
             VALUES ('/db', 'p1', 1);",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM favourite_playlists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn a_playlist_cannot_be_favourited_twice_in_one_library() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO favourite_playlists (library_path, playlist_id, seq)
             VALUES ('/db', 'p1', 1);",
        )
        .unwrap();
        assert!(conn
            .execute_batch(
                "INSERT INTO favourite_playlists (library_path, playlist_id, seq)
                 VALUES ('/db', 'p1', 2);",
            )
            .is_err());
        // ...but the same playlist id in a different library is a different row.
        conn.execute_batch(
            "INSERT INTO favourite_playlists (library_path, playlist_id, seq)
             VALUES ('/other', 'p1', 1);",
        )
        .unwrap();
    }

    #[test]
    fn mixable_templates_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO mixable_templates (id, name, options, created_at)
             VALUES ('t1', 'Peak time', '{}', 1770000000);",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM mixable_templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn mixable_template_names_are_unique() {
        // Saving over a template by name is the intended workflow, so the
        // uniqueness constraint has to be there for the upsert to land on it.
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO mixable_templates (id, name, options, created_at)
             VALUES ('t1', 'Peak time', '{}', 1770000000);",
        )
        .unwrap();
        let err = conn.execute_batch(
            "INSERT INTO mixable_templates (id, name, options, created_at)
             VALUES ('t2', 'Peak time', '{}', 1770000000);",
        );
        assert!(err.is_err());
    }
}
