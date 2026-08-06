use crate::migrations;
use anyhow::{Context, Result};
use changes::{ChangeKind, ChangeStatus};
use file_organizer::{PathMapping, PathMappings};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use smartlists::{Clause, Combinator, Smartlist};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Handle to the local SQLite WAL cache database.
pub struct CacheDb {
    pub(crate) conn: Connection,
}

impl CacheDb {
    /// Open (or create) the cache database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("opening cache DB at {}", path.display()))?;
        configure(&conn)?;
        migrations::run(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory cache — useful for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("opening in-memory cache DB")?;
        configure(&conn)?;
        migrations::run(&conn)?;
        Ok(Self { conn })
    }

    /// Load the sqlite-vec extension from the given shared-library path.
    ///
    /// Only needed for Phase 4 (embeddings). The extension must be compiled for
    /// the host platform.  If not called, vector-search operations will fail
    /// gracefully when attempted.
    ///
    /// # Safety
    /// Loading a shared library is inherently unsafe. Only load extensions
    /// from trusted paths.
    pub fn load_vec_extension(&self, lib_path: &Path) -> Result<()> {
        unsafe {
            self.conn.load_extension(lib_path, None).with_context(|| {
                format!("loading sqlite-vec extension from {}", lib_path.display())
            })?;
        }
        tracing::info!("sqlite-vec loaded from {}", lib_path.display());
        Ok(())
    }

    // ── Audio features ───────────────────────────────────────────────────────

    /// Upsert cached audio features for a track.
    pub fn upsert_audio_features(
        &self,
        track_uri: &str,
        analyzer_version: &str,
        bpm: Option<f64>,
        musical_key: Option<&str>,
        energy: Option<f64>,
        features_json: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audio_features
                 (track_uri, analyzer_version, bpm, musical_key, energy, features_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (track_uri, analyzer_version) DO UPDATE SET
                 bpm          = excluded.bpm,
                 musical_key  = excluded.musical_key,
                 energy       = excluded.energy,
                 features_json = excluded.features_json,
                 created_at   = unixepoch()",
            rusqlite::params![
                track_uri,
                analyzer_version,
                bpm,
                musical_key,
                energy,
                features_json
            ],
        )?;
        Ok(())
    }

    /// Retrieve cached audio features for a track, if present.
    pub fn get_audio_features(
        &self,
        track_uri: &str,
        analyzer_version: &str,
    ) -> Result<Option<AudioFeatures>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT bpm, musical_key, energy, features_json
             FROM audio_features
             WHERE track_uri = ?1 AND analyzer_version = ?2",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![track_uri, analyzer_version], |row| {
            Ok(AudioFeatures {
                bpm: row.get(0)?,
                musical_key: row.get(1)?,
                energy: row.get(2)?,
                features_json: row.get(3)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Bulk lookup of the latest cached `energy` for each given `track_uri`.
    ///
    /// Returns a map of `track_uri -> energy` for any URI that has at least one
    /// non-null `energy` row in `audio_features`. If multiple analyzer versions
    /// exist for the same URI, the most recent (highest `created_at`) wins.
    ///
    /// URIs without any cached energy are simply absent from the result map.
    pub fn get_energy_by_uris(
        &self,
        track_uris: &[&str],
    ) -> Result<std::collections::HashMap<String, f64>> {
        let mut out = std::collections::HashMap::new();
        if track_uris.is_empty() {
            return Ok(out);
        }
        for chunk in track_uris.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT track_uri, energy, created_at FROM audio_features
                 WHERE track_uri IN ({placeholders}) AND energy IS NOT NULL
                 ORDER BY created_at DESC"
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let params_iter: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(&*params_iter, |row| {
                let uri: String = row.get(0)?;
                let energy: f64 = row.get(1)?;
                Ok((uri, energy))
            })?;
            for r in rows {
                let (uri, energy) = r?;
                // Descending order means the first row seen for each URI is the
                // newest; `or_insert` keeps it and ignores older duplicates.
                // This stays correct even if a `LIMIT` is added later.
                out.entry(uri).or_insert(energy);
            }
        }
        Ok(out)
    }

    pub fn save_audio_fingerprint(&self, track_uri: &str, chroma_hash: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audio_fingerprints (track_uri, chroma_hash)
             VALUES (?1, ?2)
             ON CONFLICT(track_uri) DO UPDATE SET chroma_hash = excluded.chroma_hash, created_at = unixepoch()",
            rusqlite::params![track_uri, chroma_hash],
        )?;
        Ok(())
    }

    pub fn get_all_fingerprints(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT track_uri, chroma_hash FROM audio_fingerprints")?;
        let rows = stmt.query_map([], |row| {
            let track_uri: String = row.get(0)?;
            let chroma_hash: Vec<u8> = row.get(1)?;
            Ok((track_uri, chroma_hash))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Waveform peaks ──────────────────────────────────────────────────────

    /// Persist decoded waveform peaks (one `f32` per visual bar) for a track.
    /// Peaks are stored as a little-endian `f32` blob.
    pub fn set_waveform_peaks(&self, track_uri: &str, peaks: &[f32]) -> Result<()> {
        let mut bytes: Vec<u8> = Vec::with_capacity(peaks.len() * 4);
        for p in peaks {
            bytes.extend_from_slice(&p.to_le_bytes());
        }
        let sample_count = peaks.len() as i64;
        self.conn.execute(
            "INSERT INTO waveform_peaks (track_uri, peaks, sample_count)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(track_uri) DO UPDATE SET
                 peaks = excluded.peaks,
                 sample_count = excluded.sample_count,
                 generated_at = CURRENT_TIMESTAMP",
            rusqlite::params![track_uri, bytes, sample_count],
        )?;
        Ok(())
    }

    /// Load cached waveform peaks for a track, if present.
    pub fn get_waveform_peaks(&self, track_uri: &str) -> Result<Option<Vec<f32>>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT peaks FROM waveform_peaks WHERE track_uri = ?1")?;
        let mut rows = stmt.query(rusqlite::params![track_uri])?;
        if let Some(row) = rows.next()? {
            let bytes: Vec<u8> = row.get(0)?;
            let peaks = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            Ok(Some(peaks))
        } else {
            Ok(None)
        }
    }

    pub fn schema_version(&self) -> Result<u32> {
        migrations::current_version(&self.conn)
    }

    // ── Conversations ───────────────────────────────────────────────────────

    pub fn create_conversation(
        &self,
        library_path: Option<&str>,
        title: &str,
    ) -> Result<Conversation> {
        let id = new_id("conv");
        self.conn.execute(
            "INSERT INTO conversations (id, library_path, title)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![id, library_path, title],
        )?;
        self.load_conversation_metadata(&id)?
            .context("created conversation was not found")
    }

    pub fn list_conversations(&self, library_path: Option<&str>) -> Result<Vec<Conversation>> {
        if let Some(path) = library_path {
            let mut stmt = self.conn.prepare(
                "SELECT id, library_path, title, created_at, updated_at
                 FROM conversations
                 WHERE library_path = ?1
                 ORDER BY updated_at DESC, created_at DESC",
            )?;
            let rows = stmt.query_map(rusqlite::params![path], row_to_conversation)?;
            return rows
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into);
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, library_path, title, created_at, updated_at
             FROM conversations
             ORDER BY updated_at DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_conversation)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn load_conversation(&self, id: &str) -> Result<Option<ConversationWithMessages>> {
        let Some(conversation) = self.load_conversation_metadata(id)? else {
            return Ok(None);
        };
        let mut stmt = self.conn.prepare(
            "SELECT id, conversation_id, role, content_json, created_at
             FROM conversation_messages
             WHERE conversation_id = ?1
             ORDER BY created_at, rowid",
        )?;
        let rows = stmt.query_map(rusqlite::params![id], row_to_message)?;
        // Skip individual messages with malformed JSON rather than failing the
        // whole conversation load — better to surface partial history than
        // none. Genuine SQLite errors (locked DB, missing column) still bubble.
        let mut messages = Vec::new();
        let mut skipped = 0usize;
        for row in rows {
            match row {
                Ok(msg) => messages.push(msg),
                Err(rusqlite::Error::FromSqlConversionFailure(_, _, _)) => {
                    skipped += 1;
                }
                Err(e) => return Err(anyhow::Error::from(e)),
            }
        }
        if skipped > 0 {
            tracing::warn!(
                conversation_id = id,
                skipped,
                "load_conversation: dropped messages with unparseable content_json",
            );
        }
        Ok(Some(ConversationWithMessages {
            conversation,
            messages,
        }))
    }

    pub fn append_conversation_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: serde_json::Value,
    ) -> Result<ConversationMessage> {
        let id = new_id("msg");
        let content_json = serde_json::to_string(&content)?;
        self.conn.execute(
            "INSERT INTO conversation_messages (id, conversation_id, role, content_json)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, conversation_id, role, content_json],
        )?;
        self.conn.execute(
            "UPDATE conversations SET updated_at = unixepoch() WHERE id = ?1",
            rusqlite::params![conversation_id],
        )?;
        self.load_message(&id)?
            .context("created conversation message was not found")
    }

    pub fn rename_conversation(&self, id: &str, title: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE conversations
             SET title = ?2, updated_at = unixepoch()
             WHERE id = ?1",
            rusqlite::params![id, title],
        )?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    fn load_conversation_metadata(&self, id: &str) -> Result<Option<Conversation>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, library_path, title, created_at, updated_at
             FROM conversations
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], row_to_conversation)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn load_message(&self, id: &str) -> Result<Option<ConversationMessage>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, conversation_id, role, content_json, created_at
             FROM conversation_messages
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], row_to_message)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // ── Staged changes ─────────────────────────────────────────────────────

    pub fn stage_change(&self, input: NewStagedChange) -> Result<StagedChangeRecord> {
        let id = new_id("change");
        let kind = kind_to_db(&input.kind)?;
        let old_value = optional_json_to_db(&input.old_value)?;
        let new_value = optional_json_to_db(&input.new_value)?;
        self.conn.execute(
            "INSERT INTO staged_changes
                (id, library_path, kind, target_id, field, old_value, new_value, reason, confidence, status)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'Proposed')",
            rusqlite::params![
                id,
                input.library_path,
                kind,
                input.target_id,
                input.field,
                old_value,
                new_value,
                input.reason,
                input.confidence
            ],
        )?;
        self.load_change(&id)?
            .context("created staged change was not found")
    }

    pub fn list_changes(&self, library_path: Option<&str>) -> Result<Vec<StagedChangeRecord>> {
        if let Some(path) = library_path {
            let mut stmt = self.conn.prepare(
                "SELECT id, library_path, kind, target_id, field, old_value, new_value,
                        reason, confidence, status, created_at, updated_at
                 FROM staged_changes
                 WHERE library_path = ?1
                 ORDER BY updated_at DESC, created_at DESC",
            )?;
            let rows = stmt.query_map(rusqlite::params![path], row_to_change)?;
            return rows
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into);
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, library_path, kind, target_id, field, old_value, new_value,
                    reason, confidence, status, created_at, updated_at
             FROM staged_changes
             ORDER BY updated_at DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_change)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn accept_change(&self, id: &str) -> Result<StagedChangeRecord> {
        self.transition_change(id, ChangeStatus::Accepted)
    }

    pub fn reject_change(&self, id: &str) -> Result<StagedChangeRecord> {
        self.transition_change(id, ChangeStatus::Rejected)
    }

    pub fn mark_change_exported(&self, id: &str) -> Result<StagedChangeRecord> {
        self.transition_change(id, ChangeStatus::Exported)
    }

    pub fn accept_all_safe(&self, library_path: Option<&str>) -> Result<Vec<StagedChangeRecord>> {
        let proposed = self
            .list_changes(library_path)?
            .into_iter()
            .filter(|change| change.status == ChangeStatus::Proposed)
            .filter(|change| changes::is_safe_batch_kind(&change.kind))
            .collect::<Vec<_>>();
        let mut accepted = Vec::with_capacity(proposed.len());
        for change in proposed {
            accepted.push(self.accept_change(&change.id)?);
        }
        Ok(accepted)
    }

    pub fn reject_all(&self, library_path: Option<&str>) -> Result<Vec<StagedChangeRecord>> {
        let proposed = self
            .list_changes(library_path)?
            .into_iter()
            .filter(|change| change.status == ChangeStatus::Proposed)
            .collect::<Vec<_>>();
        let mut rejected = Vec::with_capacity(proposed.len());
        for change in proposed {
            rejected.push(self.reject_change(&change.id)?);
        }
        Ok(rejected)
    }

    fn load_change(&self, id: &str) -> Result<Option<StagedChangeRecord>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, library_path, kind, target_id, field, old_value, new_value,
                    reason, confidence, status, created_at, updated_at
             FROM staged_changes
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], row_to_change)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn transition_change(&self, id: &str, to: ChangeStatus) -> Result<StagedChangeRecord> {
        let change = self
            .load_change(id)?
            .with_context(|| format!("change {id} was not found"))?;
        changes::ensure_transition(&change.status, &to)?;
        self.conn.execute(
            "UPDATE staged_changes
             SET status = ?2, updated_at = unixepoch()
             WHERE id = ?1",
            rusqlite::params![id, status_to_db(&to)?],
        )?;
        self.load_change(id)?
            .context("updated staged change was not found")
    }
}

/// Cached audio analysis results for one track.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioFeatures {
    pub bpm: Option<f64>,
    pub musical_key: Option<String>,
    pub energy: Option<f64>,
    pub features_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub library_path: Option<String>,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: serde_json::Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationWithMessages {
    pub conversation: Conversation,
    pub messages: Vec<ConversationMessage>,
}

pub type NewStagedChange = changes::NewChange;
pub type StagedChangeRecord = changes::StagedChange;

fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous  = NORMAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )
    .context("configuring SQLite pragmas")?;
    Ok(())
}

fn row_to_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get(0)?,
        library_path: row.get(1)?,
        title: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationMessage> {
    let content_json: String = row.get(3)?;
    let content = serde_json::from_str(&content_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(ConversationMessage {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content,
        created_at: row.get(4)?,
    })
}

fn row_to_change(row: &rusqlite::Row<'_>) -> rusqlite::Result<StagedChangeRecord> {
    let kind: String = row.get(2)?;
    let old_value: Option<String> = row.get(5)?;
    let new_value: Option<String> = row.get(6)?;
    let status: String = row.get(9)?;
    Ok(StagedChangeRecord {
        id: row.get(0)?,
        library_path: row.get(1)?,
        kind: parse_string_enum(2, &kind)?,
        target_id: row.get(3)?,
        field: row.get(4)?,
        old_value: parse_optional_json(5, old_value)?,
        new_value: parse_optional_json(6, new_value)?,
        reason: row.get(7)?,
        confidence: row.get(8)?,
        status: parse_string_enum(9, &status)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn kind_to_db(kind: &ChangeKind) -> Result<String> {
    enum_to_db(kind)
}

fn status_to_db(status: &ChangeStatus) -> Result<String> {
    enum_to_db(status)
}

fn enum_to_db<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_value(value)?
        .as_str()
        .map(ToOwned::to_owned)
        .context("enum did not serialize as string")
}

fn optional_json_to_db(value: &Option<serde_json::Value>) -> Result<Option<String>> {
    value
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(Into::into)
}

fn parse_optional_json(
    column: usize,
    value: Option<String>,
) -> rusqlite::Result<Option<serde_json::Value>> {
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    column,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })
        .transpose()
}

fn parse_string_enum<T: for<'de> Deserialize<'de>>(
    column: usize,
    value: &str,
) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(e))
    })
}

/// Current wall-clock time in whole seconds since the epoch.
fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

fn new_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{prefix}_{nanos}")
}

// ── Custom Tags ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagCategory {
    pub id: String,
    pub name: String,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tag {
    pub id: String,
    pub category_id: String,
    pub name: String,
    pub seq: i64,
    /// Number of `track_tags` rows pointing at this tag, summed across every
    /// library. Used by the UI to render a "(7)" usage badge without an extra
    /// round-trip per tag. `u32` is safe — `COUNT(*)` for tag membership can
    /// never exceed total library track count, which fits comfortably.
    #[serde(default)]
    pub usage_count: u32,
}

impl CacheDb {
    pub fn list_tag_categories(&self) -> Result<Vec<TagCategory>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, seq FROM tag_categories ORDER BY seq, name")?;
        let rows = stmt.query_map([], |r| {
            Ok(TagCategory {
                id: r.get(0)?,
                name: r.get(1)?,
                seq: r.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn create_tag_category(&self, name: &str) -> Result<TagCategory> {
        let id = uuid::Uuid::new_v4().to_string();
        let seq: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM tag_categories",
            [],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO tag_categories (id, name, seq) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, name, seq],
        )?;
        Ok(TagCategory {
            id,
            name: name.to_owned(),
            seq,
        })
    }

    pub fn rename_tag_category(&self, id: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tag_categories SET name = ?1 WHERE id = ?2",
            rusqlite::params![name, id],
        )?;
        Ok(())
    }

    pub fn delete_tag_category(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM tag_categories WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn list_tags(&self, category_id: Option<&str>) -> Result<Vec<Tag>> {
        let mut tags = Vec::new();
        let with_count = "SELECT t.id, t.category_id, t.name, t.seq, \
             (SELECT COUNT(*) FROM track_tags tt WHERE tt.tag_id = t.id) AS usage_count \
             FROM tags t";
        if let Some(cat_id) = category_id {
            let sql = format!("{with_count} WHERE t.category_id = ?1 ORDER BY t.seq, t.name");
            let mut stmt = self.conn.prepare(&sql)?;
            let mut rows = stmt.query(rusqlite::params![cat_id])?;
            while let Some(r) = rows.next()? {
                tags.push(Tag {
                    id: r.get(0)?,
                    category_id: r.get(1)?,
                    name: r.get(2)?,
                    seq: r.get(3)?,
                    usage_count: r.get::<_, u32>(4)?,
                });
            }
        } else {
            let sql = format!("{with_count} ORDER BY t.category_id, t.seq, t.name");
            let mut stmt = self.conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            while let Some(r) = rows.next()? {
                tags.push(Tag {
                    id: r.get(0)?,
                    category_id: r.get(1)?,
                    name: r.get(2)?,
                    seq: r.get(3)?,
                    usage_count: r.get::<_, u32>(4)?,
                });
            }
        }
        Ok(tags)
    }

    pub fn create_tag(&self, category_id: &str, name: &str) -> Result<Tag> {
        let id = uuid::Uuid::new_v4().to_string();
        let seq: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM tags WHERE category_id = ?1",
            rusqlite::params![category_id],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO tags (id, category_id, name, seq) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, category_id, name, seq],
        )?;
        Ok(Tag {
            id,
            category_id: category_id.to_owned(),
            name: name.to_owned(),
            seq,
            usage_count: 0,
        })
    }

    pub fn rename_tag(&self, id: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tags SET name = ?1 WHERE id = ?2",
            rusqlite::params![name, id],
        )?;
        Ok(())
    }

    pub fn delete_tag(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM tags WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn move_tag(&self, id: &str, new_category_id: &str) -> Result<()> {
        let seq: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM tags WHERE category_id = ?1",
            rusqlite::params![new_category_id],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "UPDATE tags SET category_id = ?1, seq = ?2 WHERE id = ?3",
            rusqlite::params![new_category_id, seq, id],
        )?;
        Ok(())
    }

    pub fn get_track_tags(&self, library_path: &str, track_id: &str) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.category_id, t.name, t.seq,
                    (SELECT COUNT(*) FROM track_tags tt2 WHERE tt2.tag_id = t.id) AS usage_count
             FROM tags t
             JOIN track_tags tt ON t.id = tt.tag_id
             WHERE tt.library_path = ?1 AND tt.track_id = ?2
             ORDER BY t.category_id, t.seq, t.name",
        )?;
        let rows = stmt.query_map(rusqlite::params![library_path, track_id], |r| {
            Ok(Tag {
                id: r.get(0)?,
                category_id: r.get(1)?,
                name: r.get(2)?,
                seq: r.get(3)?,
                usage_count: r.get::<_, u32>(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn set_track_tags(
        &self,
        library_path: &str,
        track_id: &str,
        tag_ids: &[String],
    ) -> Result<()> {
        self.conn.execute("BEGIN IMMEDIATE", [])?;
        let res: Result<(), rusqlite::Error> = (|| {
            self.conn.execute(
                "DELETE FROM track_tags WHERE library_path = ?1 AND track_id = ?2",
                rusqlite::params![library_path, track_id],
            )?;

            let mut stmt = self.conn.prepare(
                "INSERT INTO track_tags (library_path, track_id, tag_id) VALUES (?1, ?2, ?3)",
            )?;
            for tag_id in tag_ids {
                stmt.execute(rusqlite::params![library_path, track_id, tag_id])?;
            }
            Ok(())
        })();

        if res.is_ok() {
            self.conn.execute("COMMIT", [])?;
        } else {
            self.conn.execute("ROLLBACK", [])?;
            res?;
        }
        Ok(())
    }

    pub fn add_track_tag(&self, library_path: &str, track_id: &str, tag_id: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO track_tags (library_path, track_id, tag_id) VALUES (?1, ?2, ?3)",
            rusqlite::params![library_path, track_id, tag_id],
        )?;
        Ok(())
    }

    pub fn remove_track_tag(&self, library_path: &str, track_id: &str, tag_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM track_tags WHERE library_path = ?1 AND track_id = ?2 AND tag_id = ?3",
            rusqlite::params![library_path, track_id, tag_id],
        )?;
        Ok(())
    }

    /// Map every track in the given library to the list of tag IDs assigned to
    /// it. Used by the frontend to populate the in-memory tag filter index so
    /// `applyFilters` can evaluate tag predicates without per-track IPC.
    pub fn list_track_tags_map(
        &self,
        library_path: &str,
    ) -> Result<std::collections::HashMap<String, Vec<String>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT track_id, tag_id FROM track_tags WHERE library_path = ?1")?;
        let rows = stmt.query_map(rusqlite::params![library_path], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for row in rows {
            let (track_id, tag_id) = row?;
            out.entry(track_id).or_default().push(tag_id);
        }
        Ok(out)
    }

    pub fn search_tracks_by_tags(
        &self,
        library_path: &str,
        tag_ids: &[String],
        match_all: bool,
    ) -> Result<Vec<String>> {
        if tag_ids.is_empty() {
            return Ok(Vec::new());
        }

        let in_clause = tag_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");

        let sql = if match_all {
            format!(
                "SELECT track_id FROM track_tags 
                 WHERE library_path = ? AND tag_id IN ({}) 
                 GROUP BY track_id 
                 HAVING COUNT(DISTINCT tag_id) = ?",
                in_clause
            )
        } else {
            format!(
                "SELECT DISTINCT track_id FROM track_tags 
                 WHERE library_path = ? AND tag_id IN ({})",
                in_clause
            )
        };

        let mut params: Vec<&dyn rusqlite::ToSql> = vec![&library_path];
        for id in tag_ids {
            params.push(id);
        }
        let count = tag_ids.len() as i64;
        if match_all {
            params.push(&count);
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params), |r| {
            r.get::<_, String>(0)
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    // ── Incoming / Archive ──────────────────────────────────────────────────

    /// Returns the per-library "incoming cleared at" watermark as unix epoch
    /// seconds. None means the user has never cleared the inbox.
    pub fn get_incoming_watermark(&self, library_path: &str) -> Result<Option<i64>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT cleared_at FROM incoming_watermark WHERE library_path = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![library_path], |r| r.get::<_, i64>(0))?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Set the watermark to the given unix epoch seconds. Upserts.
    pub fn set_incoming_watermark(&self, library_path: &str, cleared_at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO incoming_watermark (library_path, cleared_at)
             VALUES (?1, ?2)
             ON CONFLICT(library_path) DO UPDATE SET cleared_at = excluded.cleared_at",
            rusqlite::params![library_path, cleared_at],
        )?;
        Ok(())
    }

    pub fn list_archived(&self, library_path: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT track_id FROM archived_tracks WHERE library_path = ?1")?;
        let rows = stmt.query_map(rusqlite::params![library_path], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn archive_tracks(&self, library_path: &str, track_ids: &[String]) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        for id in track_ids {
            self.conn.execute(
                "INSERT OR IGNORE INTO archived_tracks (library_path, track_id, archived_at)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![library_path, id, now],
            )?;
        }
        Ok(())
    }

    // ── Smart Fixes config ──────────────────────────────────────────────────

    pub fn list_common_text_patterns(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT pattern FROM common_text_blocklist ORDER BY id ASC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn add_common_text_pattern(&self, pattern: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO common_text_blocklist (pattern) VALUES (?1)",
            rusqlite::params![pattern],
        )?;
        Ok(())
    }

    pub fn remove_common_text_pattern(&self, pattern: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM common_text_blocklist WHERE pattern = ?1",
            rusqlite::params![pattern],
        )?;
        Ok(())
    }

    pub fn unarchive_tracks(&self, library_path: &str, track_ids: &[String]) -> Result<()> {
        for id in track_ids {
            self.conn.execute(
                "DELETE FROM archived_tracks WHERE library_path = ?1 AND track_id = ?2",
                rusqlite::params![library_path, id],
            )?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Smartlists (Epic 1)
// ---------------------------------------------------------------------------

impl CacheDb {
    /// Every smartlist for a library, ordered for display in the playlist tree.
    pub fn list_smartlists(&self, library_path: &str) -> Result<Vec<Smartlist>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_folder_id, combinator, clauses_json, created_at, updated_at
             FROM smartlists WHERE library_path = ?1 ORDER BY seq, name",
        )?;
        let mut rows = stmt.query(rusqlite::params![library_path])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(row_to_smartlist(r)?);
        }
        Ok(out)
    }

    pub fn get_smartlist(&self, library_path: &str, id: &str) -> Result<Option<Smartlist>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_folder_id, combinator, clauses_json, created_at, updated_at
             FROM smartlists WHERE library_path = ?1 AND id = ?2",
        )?;
        let mut rows = stmt.query(rusqlite::params![library_path, id])?;
        match rows.next()? {
            Some(r) => Ok(Some(row_to_smartlist(r)?)),
            None => Ok(None),
        }
    }

    /// Persist a new smartlist. Validates the rule model first so malformed
    /// rules never reach storage.
    pub fn create_smartlist(
        &self,
        library_path: &str,
        name: &str,
        parent_folder_id: Option<&str>,
        combinator: Combinator,
        clauses: &[Clause],
    ) -> Result<Smartlist> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_secs();
        let list = Smartlist {
            id: id.clone(),
            name: name.to_owned(),
            parent_folder_id: parent_folder_id.map(str::to_owned),
            combinator,
            clauses: clauses.to_vec(),
            created_at: now,
            updated_at: now,
        };
        list.validate().map_err(anyhow::Error::msg)?;

        let seq: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM smartlists WHERE library_path = ?1",
            rusqlite::params![library_path],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO smartlists
                (id, library_path, name, parent_folder_id, combinator, clauses_json, seq, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            rusqlite::params![
                id,
                library_path,
                name,
                parent_folder_id,
                combinator_to_str(combinator),
                serde_json::to_string(clauses)?,
                seq,
                now,
            ],
        )?;
        Ok(list)
    }

    /// Replace a smartlist's name, folder and rules.
    pub fn update_smartlist(
        &self,
        library_path: &str,
        id: &str,
        name: &str,
        parent_folder_id: Option<&str>,
        combinator: Combinator,
        clauses: &[Clause],
    ) -> Result<Smartlist> {
        let existing = self
            .get_smartlist(library_path, id)?
            .ok_or_else(|| anyhow::anyhow!("smartlist {id} not found"))?;
        let now = now_secs();
        let updated = Smartlist {
            id: id.to_owned(),
            name: name.to_owned(),
            parent_folder_id: parent_folder_id.map(str::to_owned),
            combinator,
            clauses: clauses.to_vec(),
            created_at: existing.created_at,
            updated_at: now,
        };
        updated.validate().map_err(anyhow::Error::msg)?;

        self.conn.execute(
            "UPDATE smartlists
             SET name = ?1, parent_folder_id = ?2, combinator = ?3, clauses_json = ?4, updated_at = ?5
             WHERE library_path = ?6 AND id = ?7",
            rusqlite::params![
                name,
                parent_folder_id,
                combinator_to_str(combinator),
                serde_json::to_string(clauses)?,
                now,
                library_path,
                id,
            ],
        )?;
        Ok(updated)
    }

    pub fn delete_smartlist(&self, library_path: &str, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM smartlists WHERE library_path = ?1 AND id = ?2",
            rusqlite::params![library_path, id],
        )?;
        Ok(())
    }

    /// Smartlists currently sitting in the reserved generator folder. This is
    /// the generator's idempotency ledger — see `smartlists::generator`.
    pub fn list_generated_smartlists(&self, library_path: &str) -> Result<Vec<Smartlist>> {
        Ok(self
            .list_smartlists(library_path)?
            .into_iter()
            .filter(|s| s.parent_folder_id.as_deref() == Some(smartlists::LEXICON_FOLDER))
            .collect())
    }
}

fn combinator_to_str(c: Combinator) -> &'static str {
    match c {
        Combinator::All => "all",
        Combinator::Any => "any",
    }
}

fn combinator_from_str(s: &str) -> Combinator {
    // Unknown values fall back to `All`, which is the stricter reading — a
    // corrupted row yields fewer tracks rather than the whole library.
    match s {
        "any" => Combinator::Any,
        _ => Combinator::All,
    }
}

/// Local Path Mappings (Epic 4).
///
/// Deliberately not keyed by `library_path`: a mapping says where *this
/// computer* keeps its music, and it has to apply the moment any library is
/// opened. It is local state only — never staged, exported or synced.
impl CacheDb {
    pub fn list_path_mappings(&self) -> Result<PathMappings> {
        let mut stmt = self
            .conn
            .prepare("SELECT from_prefix, to_prefix FROM path_mappings ORDER BY seq, rowid")?;
        let mut rows = stmt.query([])?;
        let mut mappings = Vec::new();
        while let Some(r) = rows.next()? {
            mappings.push(PathMapping {
                from: r.get(0)?,
                to: r.get(1)?,
            });
        }
        Ok(PathMappings::new(mappings))
    }

    /// Add a mapping. Both prefixes must be non-empty — an empty `from` would
    /// match every path in the library and rewrite all of it.
    pub fn create_path_mapping(&self, from: &str, to: &str) -> Result<String> {
        let from = from.trim();
        let to = to.trim();
        if from.is_empty() || to.is_empty() {
            anyhow::bail!("both prefixes are required");
        }
        let id = uuid::Uuid::new_v4().to_string();
        let seq: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(seq), -1) + 1 FROM path_mappings",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO path_mappings (id, from_prefix, to_prefix, seq, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, from, to, seq, now_secs()],
        )?;
        Ok(id)
    }

    pub fn delete_path_mapping(&self, id: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM path_mappings WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(n > 0)
    }

    /// The mappings with their ids, for the settings list.
    pub fn list_path_mappings_with_ids(&self) -> Result<Vec<(String, PathMapping)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, from_prefix, to_prefix FROM path_mappings ORDER BY seq, rowid")?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push((
                r.get(0)?,
                PathMapping {
                    from: r.get(1)?,
                    to: r.get(2)?,
                },
            ));
        }
        Ok(out)
    }
}

/// A folder under observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchFolder {
    pub id: String,
    pub path: String,
}

/// Watch folders and the arrivals the user has finished with (Epic 4).
///
/// Computer-scoped for the same reason as path mappings and quick-move
/// destinations: "the folder my downloads land in" is a fact about this disk.
impl CacheDb {
    pub fn list_watch_folders(&self) -> Result<Vec<WatchFolder>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM watch_folders ORDER BY created_at, rowid")?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(WatchFolder {
                id: r.get(0)?,
                path: r.get(1)?,
            });
        }
        Ok(out)
    }

    pub fn add_watch_folder(&self, path: &str) -> Result<String> {
        let path = path.trim();
        if path.is_empty() {
            anyhow::bail!("a folder path is required");
        }
        if let Ok(id) = self.conn.query_row(
            "SELECT id FROM watch_folders WHERE path = ?1",
            rusqlite::params![path],
            |r| r.get::<_, String>(0),
        ) {
            return Ok(id);
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO watch_folders (id, path, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, path, now_secs()],
        )?;
        Ok(id)
    }

    pub fn remove_watch_folder(&self, id: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM watch_folders WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(n > 0)
    }

    /// Paths the user has already dealt with, so a scan stops offering them.
    pub fn dismissed_watch_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT path FROM watch_dismissed")?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(r.get(0)?);
        }
        Ok(out)
    }

    /// Mark paths as dealt with. Idempotent — dismissing twice is not an error,
    /// and two scans racing to dismiss the same file must not fail one of them.
    pub fn dismiss_watch_paths(&self, paths: &[String]) -> Result<usize> {
        let mut n = 0;
        for path in paths {
            let key = path.replace('\\', "/").to_lowercase();
            n += self.conn.execute(
                "INSERT INTO watch_dismissed (path_key, path, dismissed_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(path_key) DO UPDATE SET dismissed_at = excluded.dismissed_at",
                rusqlite::params![key, path, now_secs()],
            )?;
        }
        Ok(n)
    }

    /// Un-dismiss everything, so a folder can be triaged again from scratch.
    pub fn clear_dismissed_watch_paths(&self) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM watch_dismissed", [])?)
    }
}

/// A remembered quick-move destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickMoveFolder {
    pub id: String,
    pub path: String,
    pub favourite: bool,
    pub last_used_at: i64,
}

/// Quick move destinations (Epic 4).
///
/// Favourites first, then most-recently-used. Like path mappings, these belong
/// to the computer rather than to a library — "the folder I file house tracks
/// into" is a fact about this disk.
impl CacheDb {
    pub fn list_quick_move_folders(&self) -> Result<Vec<QuickMoveFolder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, favourite, last_used_at FROM quick_move_folders
             ORDER BY favourite DESC, last_used_at DESC",
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(QuickMoveFolder {
                id: r.get(0)?,
                path: r.get(1)?,
                favourite: r.get::<_, i64>(2)? != 0,
                last_used_at: r.get(3)?,
            });
        }
        Ok(out)
    }

    /// Remember a destination, or refresh its recency if already known.
    ///
    /// Upsert rather than insert: using the same folder twice should move it up
    /// the list, not create a duplicate entry.
    pub fn record_quick_move_folder(&self, path: &str) -> Result<String> {
        let path = path.trim();
        if path.is_empty() {
            anyhow::bail!("a folder path is required");
        }
        if let Ok(id) = self.conn.query_row(
            "SELECT id FROM quick_move_folders WHERE path = ?1",
            rusqlite::params![path],
            |r| r.get::<_, String>(0),
        ) {
            self.conn.execute(
                "UPDATE quick_move_folders SET last_used_at = ?1 WHERE id = ?2",
                rusqlite::params![now_secs(), id],
            )?;
            return Ok(id);
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO quick_move_folders (id, path, favourite, last_used_at)
             VALUES (?1, ?2, 0, ?3)",
            rusqlite::params![id, path, now_secs()],
        )?;
        Ok(id)
    }

    /// Toggle the favourite flag, returning its new state.
    pub fn toggle_quick_move_favourite(&self, id: &str) -> Result<bool> {
        let current: i64 = self.conn.query_row(
            "SELECT favourite FROM quick_move_folders WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )?;
        let next = if current == 0 { 1 } else { 0 };
        self.conn.execute(
            "UPDATE quick_move_folders SET favourite = ?1 WHERE id = ?2",
            rusqlite::params![next, id],
        )?;
        Ok(next == 1)
    }

    pub fn delete_quick_move_folder(&self, id: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM quick_move_folders WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(n > 0)
    }
}

fn row_to_smartlist(r: &rusqlite::Row<'_>) -> Result<Smartlist> {
    let clauses_json: String = r.get(4)?;
    let clauses: Vec<Clause> = serde_json::from_str(&clauses_json).unwrap_or_default();
    let combinator_raw: String = r.get(3)?;
    Ok(Smartlist {
        id: r.get(0)?,
        name: r.get(1)?,
        parent_folder_id: r.get(2)?,
        combinator: combinator_from_str(&combinator_raw),
        clauses,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use smartlists::{Field, Operator, Rule, Value};

    #[test]
    fn adding_the_same_watch_folder_twice_is_idempotent() {
        let db = CacheDb::open_in_memory().unwrap();
        let a = db.add_watch_folder("/Music/Watch").unwrap();
        let b = db.add_watch_folder("/Music/Watch").unwrap();
        assert_eq!(a, b);
        assert_eq!(db.list_watch_folders().unwrap().len(), 1);
    }

    #[test]
    fn watch_folders_can_be_removed() {
        let db = CacheDb::open_in_memory().unwrap();
        let id = db.add_watch_folder("/Music/Watch").unwrap();
        assert!(db.remove_watch_folder(&id).unwrap());
        assert!(!db.remove_watch_folder(&id).unwrap());
        assert!(db.list_watch_folders().unwrap().is_empty());
    }

    #[test]
    fn an_empty_watch_folder_path_is_refused() {
        let db = CacheDb::open_in_memory().unwrap();
        assert!(db.add_watch_folder("   ").is_err());
    }

    #[test]
    fn dismissing_the_same_path_twice_is_not_an_error() {
        // Two scans racing to dismiss the same file must not fail one of them.
        let db = CacheDb::open_in_memory().unwrap();
        let paths = vec!["/Music/Watch/a.mp3".to_string()];
        db.dismiss_watch_paths(&paths).unwrap();
        db.dismiss_watch_paths(&paths).unwrap();
        assert_eq!(db.dismissed_watch_paths().unwrap().len(), 1);
    }

    #[test]
    fn dismissal_is_keyed_case_and_separator_insensitively() {
        let db = CacheDb::open_in_memory().unwrap();
        db.dismiss_watch_paths(&["/Music/Watch/A.mp3".to_string()])
            .unwrap();
        db.dismiss_watch_paths(&["\\music\\watch\\a.mp3".to_string()])
            .unwrap();
        assert_eq!(db.dismissed_watch_paths().unwrap().len(), 1);
    }

    #[test]
    fn dismissals_can_be_cleared_to_triage_a_folder_again() {
        let db = CacheDb::open_in_memory().unwrap();
        db.dismiss_watch_paths(&["/a.mp3".to_string(), "/b.mp3".to_string()])
            .unwrap();
        assert_eq!(db.clear_dismissed_watch_paths().unwrap(), 2);
        assert!(db.dismissed_watch_paths().unwrap().is_empty());
    }

    #[test]
    fn recording_the_same_quick_move_folder_twice_does_not_duplicate_it() {
        let db = CacheDb::open_in_memory().unwrap();
        let first = db.record_quick_move_folder("/Music/House").unwrap();
        let second = db.record_quick_move_folder("/Music/House").unwrap();
        assert_eq!(first, second);
        assert_eq!(db.list_quick_move_folders().unwrap().len(), 1);
    }

    #[test]
    fn quick_move_favourites_sort_ahead_of_plain_recents() {
        let db = CacheDb::open_in_memory().unwrap();
        db.record_quick_move_folder("/Music/House").unwrap();
        let techno = db.record_quick_move_folder("/Music/Techno").unwrap();
        assert!(db.toggle_quick_move_favourite(&techno).unwrap());

        let list = db.list_quick_move_folders().unwrap();
        assert_eq!(list[0].path, "/Music/Techno");
        assert!(list[0].favourite);
        assert!(!list[1].favourite);
    }

    #[test]
    fn toggling_a_favourite_twice_returns_it_to_plain() {
        let db = CacheDb::open_in_memory().unwrap();
        let id = db.record_quick_move_folder("/Music/House").unwrap();
        assert!(db.toggle_quick_move_favourite(&id).unwrap());
        assert!(!db.toggle_quick_move_favourite(&id).unwrap());
    }

    #[test]
    fn quick_move_folders_can_be_forgotten() {
        let db = CacheDb::open_in_memory().unwrap();
        let id = db.record_quick_move_folder("/Music/House").unwrap();
        assert!(db.delete_quick_move_folder(&id).unwrap());
        assert!(!db.delete_quick_move_folder(&id).unwrap());
        assert!(db.list_quick_move_folders().unwrap().is_empty());
    }

    #[test]
    fn an_empty_quick_move_path_is_refused() {
        let db = CacheDb::open_in_memory().unwrap();
        assert!(db.record_quick_move_folder("   ").is_err());
    }

    #[test]
    fn path_mappings_round_trip_and_delete() {
        let db = CacheDb::open_in_memory().unwrap();
        assert!(db.list_path_mappings().unwrap().is_empty());

        let id = db
            .create_path_mapping("D:\\Music", "/Users/me/Music")
            .unwrap();
        let mappings = db.list_path_mappings().unwrap();
        assert_eq!(mappings.mappings.len(), 1);
        assert_eq!(
            mappings.resolve("D:\\Music\\a.mp3"),
            std::path::PathBuf::from("/Users/me/Music/a.mp3")
        );

        assert!(db.delete_path_mapping(&id).unwrap());
        assert!(!db.delete_path_mapping(&id).unwrap());
        assert!(db.list_path_mappings().unwrap().is_empty());
    }

    #[test]
    fn a_mapping_with_an_empty_prefix_is_refused() {
        // An empty `from` would match every path and rewrite the whole library.
        let db = CacheDb::open_in_memory().unwrap();
        assert!(db.create_path_mapping("  ", "/Users/me/Music").is_err());
        assert!(db.create_path_mapping("/Music", "").is_err());
    }

    #[test]
    fn mappings_come_back_in_insertion_order_with_their_ids() {
        let db = CacheDb::open_in_memory().unwrap();
        let first = db.create_path_mapping("/Music", "/a").unwrap();
        let second = db.create_path_mapping("/Music/Live", "/b").unwrap();
        let rows = db.list_path_mappings_with_ids().unwrap();
        assert_eq!(
            rows.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
            vec![first, second]
        );
    }

    #[test]
    fn open_in_memory_succeeds() {
        let db = CacheDb::open_in_memory().unwrap();
        assert!(db.schema_version().unwrap() >= 1);
    }

    #[test]
    fn open_file_db_applies_migrations() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        let db = CacheDb::open(&path).unwrap();
        assert!(db.schema_version().unwrap() >= 1);
    }

    #[test]
    fn waveform_peaks_round_trips() {
        let cache = CacheDb::open_in_memory().unwrap();
        let peaks: Vec<f32> = vec![0.1, 0.2, -0.3, 0.5];
        cache.set_waveform_peaks("file:///t1.mp3", &peaks).unwrap();
        let loaded = cache.get_waveform_peaks("file:///t1.mp3").unwrap();
        assert_eq!(loaded.unwrap(), peaks);
    }

    #[test]
    fn waveform_peaks_returns_none_for_unknown() {
        let cache = CacheDb::open_in_memory().unwrap();
        assert!(cache
            .get_waveform_peaks("file:///nope.mp3")
            .unwrap()
            .is_none());
    }

    #[test]
    fn waveform_peaks_overwrite_replaces_data() {
        let cache = CacheDb::open_in_memory().unwrap();
        cache
            .set_waveform_peaks("file:///t.mp3", &[0.1, 0.2, 0.3])
            .unwrap();
        cache.set_waveform_peaks("file:///t.mp3", &[0.9]).unwrap();
        let loaded = cache.get_waveform_peaks("file:///t.mp3").unwrap().unwrap();
        assert_eq!(loaded, vec![0.9_f32]);
    }

    #[test]
    fn upsert_and_get_audio_features() {
        let db = CacheDb::open_in_memory().unwrap();
        db.upsert_audio_features(
            "file:///test.mp3",
            "v1",
            Some(132.0),
            Some("8A"),
            Some(0.8),
            None,
        )
        .unwrap();
        let feat = db
            .get_audio_features("file:///test.mp3", "v1")
            .unwrap()
            .expect("should be cached");
        assert!((feat.bpm.unwrap() - 132.0).abs() < 0.001);
        assert_eq!(feat.musical_key.as_deref(), Some("8A"));
    }

    #[test]
    fn upsert_overwrites_existing() {
        let db = CacheDb::open_in_memory().unwrap();
        db.upsert_audio_features("file:///t.mp3", "v1", Some(128.0), None, None, None)
            .unwrap();
        db.upsert_audio_features("file:///t.mp3", "v1", Some(130.0), Some("11B"), None, None)
            .unwrap();
        let feat = db
            .get_audio_features("file:///t.mp3", "v1")
            .unwrap()
            .unwrap();
        assert!((feat.bpm.unwrap() - 130.0).abs() < 0.001);
        assert_eq!(feat.musical_key.as_deref(), Some("11B"));
    }

    #[test]
    fn get_missing_returns_none() {
        let db = CacheDb::open_in_memory().unwrap();
        assert!(db
            .get_audio_features("file:///nope.mp3", "v1")
            .unwrap()
            .is_none());
    }

    #[test]
    fn different_analyzer_versions_are_independent() {
        let db = CacheDb::open_in_memory().unwrap();
        db.upsert_audio_features("file:///t.mp3", "v1", Some(128.0), None, None, None)
            .unwrap();
        db.upsert_audio_features("file:///t.mp3", "v2", Some(130.0), None, None, None)
            .unwrap();
        let v1 = db
            .get_audio_features("file:///t.mp3", "v1")
            .unwrap()
            .unwrap();
        let v2 = db
            .get_audio_features("file:///t.mp3", "v2")
            .unwrap()
            .unwrap();
        assert!((v1.bpm.unwrap() - 128.0).abs() < 0.001);
        assert!((v2.bpm.unwrap() - 130.0).abs() < 0.001);
    }

    #[test]
    fn get_energy_by_uris_returns_newest_when_multiple_rows_exist() {
        let db = CacheDb::open_in_memory().unwrap();
        // Two analyzer versions for the same URI; insert the older one second
        // to ensure ordering is driven by `created_at`, not insertion order.
        db.conn
            .execute(
                "INSERT INTO audio_features (track_uri, analyzer_version, energy, created_at)
                 VALUES (?1, 'v2', 0.91, 2000)",
                rusqlite::params!["file:///newest.mp3"],
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO audio_features (track_uri, analyzer_version, energy, created_at)
                 VALUES (?1, 'v1', 0.10, 1000)",
                rusqlite::params!["file:///newest.mp3"],
            )
            .unwrap();
        let map = db.get_energy_by_uris(&["file:///newest.mp3"]).unwrap();
        let energy = map.get("file:///newest.mp3").copied().expect("present");
        assert!(
            (energy - 0.91).abs() < 1e-9,
            "expected newest energy, got {energy}"
        );
    }

    #[test]
    fn wal_mode_is_set() {
        let db = CacheDb::open_in_memory().unwrap();
        let mode: String = db
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        // In-memory always reports "memory" regardless; file DBs report "wal".
        // Just verify the pragma executes without error.
        assert!(!mode.is_empty());
    }

    #[test]
    fn conversation_crud_roundtrip() {
        let db = CacheDb::open_in_memory().unwrap();
        let conversation = db
            .create_conversation(Some("/library/master.db"), "Library audit")
            .unwrap();
        db.append_conversation_message(
            &conversation.id,
            "user",
            serde_json::json!({"text": "audit my playlists"}),
        )
        .unwrap();
        db.append_conversation_message(
            &conversation.id,
            "assistant",
            serde_json::json!({"blocks": [{"type": "text", "text": "Done"}]}),
        )
        .unwrap();

        let loaded = db.load_conversation(&conversation.id).unwrap().unwrap();
        assert_eq!(loaded.conversation.title, "Library audit");
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].role, "user");

        let list = db.list_conversations(Some("/library/master.db")).unwrap();
        assert_eq!(list.len(), 1);

        db.delete_conversation(&conversation.id).unwrap();
        assert!(db.load_conversation(&conversation.id).unwrap().is_none());
    }

    #[test]
    fn load_conversation_skips_malformed_messages() {
        let db = CacheDb::open_in_memory().unwrap();
        let conversation = db
            .create_conversation(Some("/library/master.db"), "Corrupted")
            .unwrap();
        db.append_conversation_message(
            &conversation.id,
            "user",
            serde_json::json!({"text": "first"}),
        )
        .unwrap();
        db.append_conversation_message(
            &conversation.id,
            "assistant",
            serde_json::json!({"blocks": []}),
        )
        .unwrap();

        // Simulate corruption: overwrite one row's content_json with non-JSON.
        db.conn
            .execute(
                "UPDATE conversation_messages SET content_json = 'not json {' \
                 WHERE conversation_id = ?1 AND role = 'user'",
                rusqlite::params![&conversation.id],
            )
            .unwrap();

        // Should still load the surviving message rather than failing the call.
        let loaded = db.load_conversation(&conversation.id).unwrap().unwrap();
        assert_eq!(
            loaded.messages.len(),
            1,
            "the assistant message must survive even though the user message is corrupted",
        );
        assert_eq!(loaded.messages[0].role, "assistant");
    }

    #[test]
    fn staged_change_crud_roundtrip() {
        let db = CacheDb::open_in_memory().unwrap();
        let change = db
            .stage_change(NewStagedChange {
                library_path: Some("/library/master.db".to_owned()),
                kind: ChangeKind::TrackMetadataEdit,
                target_id: Some("track-1".to_owned()),
                field: Some("genre".to_owned()),
                old_value: Some(serde_json::json!("House")),
                new_value: Some(serde_json::json!("Deep House")),
                reason: Some("Normalize genre".to_owned()),
                confidence: Some(0.88),
            })
            .unwrap();
        assert_eq!(change.status, ChangeStatus::Proposed);

        let list = db.list_changes(Some("/library/master.db")).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, ChangeKind::TrackMetadataEdit);

        let accepted = db.accept_change(&change.id).unwrap();
        assert_eq!(accepted.status, ChangeStatus::Accepted);
        let exported = db.mark_change_exported(&change.id).unwrap();
        assert_eq!(exported.status, ChangeStatus::Exported);
    }

    #[test]
    fn staged_change_rejects_invalid_transitions() {
        let db = CacheDb::open_in_memory().unwrap();
        let change = db
            .stage_change(NewStagedChange {
                library_path: None,
                kind: ChangeKind::TrackMetadataEdit,
                target_id: Some("track-1".to_owned()),
                field: Some("genre".to_owned()),
                old_value: None,
                new_value: Some(serde_json::json!("Techno")),
                reason: None,
                confidence: None,
            })
            .unwrap();
        db.accept_change(&change.id).unwrap();
        assert!(db.reject_change(&change.id).is_err());
    }

    #[test]
    fn staged_change_batch_operations_scope_by_library() {
        let db = CacheDb::open_in_memory().unwrap();
        db.stage_change(NewStagedChange {
            library_path: Some("/a.db".to_owned()),
            kind: ChangeKind::TrackMetadataEdit,
            target_id: Some("track-1".to_owned()),
            field: Some("genre".to_owned()),
            old_value: None,
            new_value: Some(serde_json::json!("House")),
            reason: None,
            confidence: None,
        })
        .unwrap();
        db.stage_change(NewStagedChange {
            library_path: Some("/b.db".to_owned()),
            kind: ChangeKind::TrackMetadataEdit,
            target_id: Some("track-2".to_owned()),
            field: Some("genre".to_owned()),
            old_value: None,
            new_value: Some(serde_json::json!("Techno")),
            reason: None,
            confidence: None,
        })
        .unwrap();

        let accepted = db.accept_all_safe(Some("/a.db")).unwrap();
        assert_eq!(accepted.len(), 1);
        assert_eq!(
            db.list_changes(Some("/b.db")).unwrap()[0].status,
            ChangeStatus::Proposed
        );
    }

    #[test]
    fn list_tags_reports_usage_count() {
        let db = CacheDb::open_in_memory().unwrap();
        let cat = db.create_tag_category("Mood").unwrap();
        let chill = db.create_tag(&cat.id, "Chill").unwrap();
        let hype = db.create_tag(&cat.id, "Hype").unwrap();

        // Fresh tags have a zero usage count.
        let tags = db.list_tags(Some(&cat.id)).unwrap();
        assert_eq!(
            tags.iter().find(|t| t.id == chill.id).unwrap().usage_count,
            0
        );
        assert_eq!(
            tags.iter().find(|t| t.id == hype.id).unwrap().usage_count,
            0
        );

        // Assign `chill` to two tracks, `hype` to one.
        db.add_track_tag("/lib.db", "track-1", &chill.id).unwrap();
        db.add_track_tag("/lib.db", "track-2", &chill.id).unwrap();
        db.add_track_tag("/lib.db", "track-1", &hype.id).unwrap();

        let tags = db.list_tags(Some(&cat.id)).unwrap();
        let chill_row = tags.iter().find(|t| t.id == chill.id).unwrap();
        let hype_row = tags.iter().find(|t| t.id == hype.id).unwrap();
        assert_eq!(chill_row.usage_count, 2);
        assert_eq!(hype_row.usage_count, 1);

        // Removing a binding decrements the count.
        db.remove_track_tag("/lib.db", "track-2", &chill.id)
            .unwrap();
        let tags = db.list_tags(None).unwrap();
        assert_eq!(
            tags.iter().find(|t| t.id == chill.id).unwrap().usage_count,
            1
        );
    }

    #[test]
    fn list_track_tags_map_groups_by_track() {
        let db = CacheDb::open_in_memory().unwrap();
        let cat = db.create_tag_category("Mood").unwrap();
        let a = db.create_tag(&cat.id, "A").unwrap();
        let b = db.create_tag(&cat.id, "B").unwrap();
        db.add_track_tag("/lib.db", "t1", &a.id).unwrap();
        db.add_track_tag("/lib.db", "t1", &b.id).unwrap();
        db.add_track_tag("/lib.db", "t2", &b.id).unwrap();
        // Different library — must be excluded.
        db.add_track_tag("/other.db", "t1", &a.id).unwrap();

        let map = db.list_track_tags_map("/lib.db").unwrap();
        let mut t1 = map.get("t1").cloned().unwrap_or_default();
        t1.sort();
        let mut expected = vec![a.id.clone(), b.id.clone()];
        expected.sort();
        let t2 = map.get("t2").cloned().unwrap_or_default();
        assert_eq!(t1, expected);
        assert_eq!(t2, vec![b.id.clone()]);
        // /other.db rows must be excluded — t1 in /lib.db should only have two tags.
        assert_eq!(map.get("t1").map(|v| v.len()), Some(2));
    }

    // -- smartlists ------------------------------------------------------

    fn house_clause() -> Vec<Clause> {
        vec![Clause::single(Rule::new(
            Field::Genre,
            Operator::Equals,
            Value::Text("House".into()),
        ))]
    }

    #[test]
    fn smartlist_crud_roundtrip() {
        let db = CacheDb::open_in_memory().unwrap();
        let created = db
            .create_smartlist(
                "/lib.db",
                "Peak time",
                None,
                Combinator::All,
                &house_clause(),
            )
            .unwrap();
        assert_eq!(created.name, "Peak time");

        let listed = db.list_smartlists("/lib.db").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].clauses, house_clause());

        let fetched = db.get_smartlist("/lib.db", &created.id).unwrap().unwrap();
        assert_eq!(fetched, created);

        db.delete_smartlist("/lib.db", &created.id).unwrap();
        assert!(db.list_smartlists("/lib.db").unwrap().is_empty());
    }

    #[test]
    fn smartlists_are_scoped_by_library() {
        let db = CacheDb::open_in_memory().unwrap();
        db.create_smartlist("/a.db", "A", None, Combinator::All, &house_clause())
            .unwrap();
        db.create_smartlist("/b.db", "B", None, Combinator::All, &house_clause())
            .unwrap();
        assert_eq!(db.list_smartlists("/a.db").unwrap().len(), 1);
        assert_eq!(db.list_smartlists("/b.db").unwrap()[0].name, "B");
    }

    #[test]
    fn update_smartlist_replaces_rules_and_preserves_created_at() {
        let db = CacheDb::open_in_memory().unwrap();
        let created = db
            .create_smartlist("/lib.db", "Old", None, Combinator::All, &house_clause())
            .unwrap();
        let new_clauses = vec![Clause::single(Rule::new(
            Field::Bpm,
            Operator::Between,
            Value::Range(120.0, 130.0),
        ))];
        let updated = db
            .update_smartlist(
                "/lib.db",
                &created.id,
                "New",
                Some("Lexicon"),
                Combinator::Any,
                &new_clauses,
            )
            .unwrap();
        assert_eq!(updated.name, "New");
        assert_eq!(updated.combinator, Combinator::Any);
        assert_eq!(updated.clauses, new_clauses);
        assert_eq!(updated.created_at, created.created_at);

        let reloaded = db.get_smartlist("/lib.db", &created.id).unwrap().unwrap();
        assert_eq!(reloaded.clauses, new_clauses);
        assert_eq!(reloaded.parent_folder_id.as_deref(), Some("Lexicon"));
    }

    #[test]
    fn update_missing_smartlist_errors() {
        let db = CacheDb::open_in_memory().unwrap();
        let err = db.update_smartlist(
            "/lib.db",
            "nope",
            "X",
            None,
            Combinator::All,
            &house_clause(),
        );
        assert!(err.is_err());
    }

    #[test]
    fn create_rejects_invalid_rules() {
        let db = CacheDb::open_in_memory().unwrap();
        // `Between` on a text field is not a valid combination.
        let bad = vec![Clause::single(Rule::new(
            Field::Title,
            Operator::Between,
            Value::Range(1.0, 2.0),
        ))];
        assert!(db
            .create_smartlist("/lib.db", "Bad", None, Combinator::All, &bad)
            .is_err());
        assert!(db.list_smartlists("/lib.db").unwrap().is_empty());
    }

    #[test]
    fn create_rejects_empty_name() {
        let db = CacheDb::open_in_memory().unwrap();
        assert!(db
            .create_smartlist("/lib.db", "   ", None, Combinator::All, &house_clause())
            .is_err());
    }

    #[test]
    fn list_generated_returns_only_the_lexicon_folder() {
        let db = CacheDb::open_in_memory().unwrap();
        db.create_smartlist("/lib.db", "Mine", None, Combinator::All, &house_clause())
            .unwrap();
        db.create_smartlist(
            "/lib.db",
            "House",
            Some(smartlists::LEXICON_FOLDER),
            Combinator::All,
            &house_clause(),
        )
        .unwrap();
        let generated = db.list_generated_smartlists("/lib.db").unwrap();
        assert_eq!(generated.len(), 1);
        assert_eq!(generated[0].name, "House");
    }

    #[test]
    fn corrupt_clauses_json_degrades_to_empty_rather_than_erroring() {
        let db = CacheDb::open_in_memory().unwrap();
        let created = db
            .create_smartlist("/lib.db", "X", None, Combinator::All, &house_clause())
            .unwrap();
        db.conn
            .execute(
                "UPDATE smartlists SET clauses_json = 'not json' WHERE id = ?1",
                rusqlite::params![created.id],
            )
            .unwrap();
        let reloaded = db.get_smartlist("/lib.db", &created.id).unwrap().unwrap();
        // An empty rule set matches nothing, so a corrupt row is inert rather
        // than accidentally selecting the entire library.
        assert!(reloaded.clauses.is_empty());
    }
}
