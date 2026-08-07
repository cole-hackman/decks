//! Share / export (Epic 6) — the IPC surface for `crates/share`.
//!
//! Per `docs/lexicon/08-streaming.md §Share / export`. The spec is explicit
//! that this is **not** Sync: sharing produces a file, syncing updates a DJ
//! app. Nothing here writes to `master.db` or stages a change.
//!
//! Rendering happens in Rust rather than the renderer so the same export is
//! available to the CLI and the MCP server, and so CSV escaping has exactly
//! one implementation.

use std::path::Path;

use decks_core::rekordbox_db::RekordboxDb;
use serde::{Deserialize, Serialize};
use share::{csv, html, m3u, quick_copy, Column};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareFormat {
    QuickCopy,
    QuickCopyNumbered,
    Csv,
    M3u,
    Html,
}

impl ShareFormat {
    /// Extension for the save dialog. Quick copy has none — it goes to the
    /// clipboard, and the renderer never reaches the save path for it.
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::M3u => "m3u8",
            Self::Html => "html",
            Self::QuickCopy | Self::QuickCopyNumbered => "txt",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ShareExport {
    pub content: String,
    /// Suggested filename, sanitised — a playlist called `Friday 8/6` must not
    /// become a path.
    pub filename: String,
    pub track_count: usize,
    /// Titles the format could not carry. Only M3U produces these, and only
    /// for tracks with no file path; handing back a quietly short playlist is
    /// how a set goes missing on the night.
    pub skipped: Vec<String>,
}

/// Strip anything that would make the name a path or upset a filesystem.
fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "playlist".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Render a playlist in one of the share formats.
///
/// `columns` is what the caller is showing, in the order it is showing them —
/// the spec's "exactly the columns selected, in the order shown". Ignored by
/// the formats that have no columns.
#[tauri::command]
pub async fn share_playlist(
    path: String,
    playlist_id: String,
    format: ShareFormat,
    columns: Vec<Column>,
) -> Result<ShareExport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = RekordboxDb::open(Path::new(&path)).map_err(|e| e.to_string())?;
        let Some(playlist) = db.playlist_by_id(&playlist_id).map_err(|e| e.to_string())? else {
            return Err(format!("playlist not found: {playlist_id}"));
        };

        let mut entries = db
            .playlist_entries(&playlist_id)
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|e| e.track_no.unwrap_or(i64::MAX));
        let ids: Vec<String> = entries.into_iter().map(|e| e.content_id).collect();

        // `tracks_by_ids` returns database order, not playlist order. The
        // whole point of exporting a playlist is its order, so it is restored
        // here rather than trusted.
        let fetched = db.tracks_by_ids(&ids).map_err(|e| e.to_string())?;
        let by_id: std::collections::HashMap<&str, _> =
            fetched.iter().map(|t| (t.id.as_str(), t)).collect();
        let tracks: Vec<_> = ids
            .iter()
            .filter_map(|id| by_id.get(id.as_str()).map(|t| (*t).clone()))
            .collect();

        let columns = if columns.is_empty() {
            share::default_columns()
        } else {
            columns
        };

        let (content, skipped) = match format {
            ShareFormat::QuickCopy => (quick_copy(&tracks, false), Vec::new()),
            ShareFormat::QuickCopyNumbered => (quick_copy(&tracks, true), Vec::new()),
            ShareFormat::Csv => (csv(&tracks, &columns), Vec::new()),
            ShareFormat::Html => (html(&tracks, &columns, &playlist.name), Vec::new()),
            ShareFormat::M3u => {
                let out = m3u(&tracks);
                (out.content, out.skipped)
            }
        };

        Ok(ShareExport {
            content,
            filename: format!("{}.{}", safe_filename(&playlist.name), format.extension()),
            track_count: tracks.len(),
            skipped,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Write an export to disk. Separate from rendering so the renderer can put
/// Quick Copy on the clipboard without a file ever existing.
#[tauri::command]
pub fn write_share_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("could not write {path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playlist_name_cannot_become_a_path() {
        assert_eq!(safe_filename("Friday 8/6"), "Friday 8-6");
        // Separators become dashes and the leading dots go with the trim, so
        // the result is neither a path nor a hidden file.
        assert_eq!(safe_filename("../../etc/passwd"), "-..-etc-passwd");
        assert_eq!(safe_filename(".hidden"), "hidden");
        assert_eq!(safe_filename("C:\\evil"), "C--evil");
    }

    #[test]
    fn a_name_that_sanitises_to_nothing_gets_a_fallback() {
        // Better a file called "playlist" than one called "" or ".".
        assert_eq!(safe_filename(""), "playlist");
        assert_eq!(safe_filename("   "), "playlist");
        assert_eq!(safe_filename("..."), "playlist");
    }

    #[test]
    fn a_normal_name_is_left_alone() {
        assert_eq!(safe_filename("Peak Time Techno"), "Peak Time Techno");
    }

    #[test]
    fn formats_deserialise_from_the_names_the_ui_sends() {
        for (json, expected) in [
            ("\"quick_copy\"", ShareFormat::QuickCopy),
            ("\"quick_copy_numbered\"", ShareFormat::QuickCopyNumbered),
            ("\"csv\"", ShareFormat::Csv),
            ("\"m3u\"", ShareFormat::M3u),
            ("\"html\"", ShareFormat::Html),
        ] {
            assert_eq!(
                serde_json::from_str::<ShareFormat>(json).unwrap(),
                expected,
                "{json}"
            );
        }
    }

    #[test]
    fn each_format_gets_the_extension_its_file_needs() {
        assert_eq!(ShareFormat::Csv.extension(), "csv");
        assert_eq!(ShareFormat::M3u.extension(), "m3u8");
        assert_eq!(ShareFormat::Html.extension(), "html");
    }
}
