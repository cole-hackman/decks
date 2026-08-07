//! Bulk Write Tags (Epic 4).
//!
//! Projects the library's own values back into the audio files' ID3/Vorbis/MP4
//! tags, so the files look right in any other program. Distinct from Sync,
//! which updates Rekordbox's database — a user whose music is also in a plain
//! music player needs both.
//!
//! Per `docs/lexicon/06-files.md §Write Tags (ID3)`.

use std::path::Path;

use audio_tags::TagWriteFields;
use serde::{Deserialize, Serialize};

/// Which fields to write. Everything unselected is left untouched in the file —
/// `write_tag_fields` only writes the `Some` values, so an unselected field is
/// genuinely not written rather than written as empty.
///
/// Per-field selection is the point of the feature: "write only titles and
/// leave everything else alone" is the common case for a library where the
/// files' own tags are better than the database's for some fields.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
pub struct TagFieldSelection {
    #[serde(default)]
    pub title: bool,
    #[serde(default)]
    pub artist: bool,
    #[serde(default)]
    pub album: bool,
    #[serde(default)]
    pub genre: bool,
    #[serde(default)]
    pub bpm: bool,
    #[serde(default)]
    pub musical_key: bool,
    #[serde(default)]
    pub comment: bool,
    #[serde(default)]
    pub year: bool,
}

impl TagFieldSelection {
    pub fn any(&self) -> bool {
        self.title
            || self.artist
            || self.album
            || self.genre
            || self.bpm
            || self.musical_key
            || self.comment
            || self.year
    }
}

#[derive(Debug, Default, Serialize)]
pub struct WriteTagsResult {
    pub written: Vec<String>,
    /// `(track_id, reason)`. One unwritable file must not abandon the batch.
    pub failed: Vec<(String, String)>,
    /// Tracks skipped because every selected field was empty in the library.
    /// Writing them would blank good tags in the file with nothing.
    pub skipped: Vec<String>,
}

/// Build the write payload, taking only the selected fields that actually have
/// a value.
///
/// An empty library value is *not* written even when its field is selected.
/// Blanking a file's real artist because the database happens not to know it is
/// the one outcome this feature must not have.
fn payload(
    track: &decks_core::rekordbox_db::Track,
    selection: &TagFieldSelection,
) -> TagWriteFields {
    fn text(selected: bool, value: Option<&str>) -> Option<String> {
        if !selected {
            return None;
        }
        value
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    }

    TagWriteFields {
        title: text(selection.title, Some(track.title.as_str())),
        artist: text(selection.artist, track.artist.as_deref()),
        album: text(selection.album, track.album.as_deref()),
        genre: text(selection.genre, track.genre.as_deref()),
        bpm: if selection.bpm { track.bpm } else { None },
        musical_key: text(selection.musical_key, track.musical_key.as_deref()),
        comment: text(selection.comment, track.comment.as_deref()),
        year: if selection.year {
            track.release_year.and_then(|y| u32::try_from(y).ok())
        } else {
            None
        },
    }
}

fn is_empty(fields: &TagWriteFields) -> bool {
    fields.title.is_none()
        && fields.artist.is_none()
        && fields.album.is_none()
        && fields.genre.is_none()
        && fields.bpm.is_none()
        && fields.musical_key.is_none()
        && fields.comment.is_none()
        && fields.year.is_none()
}

/// Write the selected fields from the library into each track's file.
///
/// Writes to the files only — `master.db` is not touched, and this is not part
/// of the staged-change pipeline because the files are not the database.
#[tauri::command]
pub async fn write_tags_bulk(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
    selection: TagFieldSelection,
) -> Result<WriteTagsResult, String> {
    if !selection.any() {
        return Err("select at least one field to write".into());
    }
    // Write to where the file actually is on this machine.
    let mappings = crate::organizer::path_mappings(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        let mut result = WriteTagsResult::default();
        for id in track_ids {
            let track = match db.track_by_id(&id) {
                Ok(Some(t)) => t,
                Ok(None) => {
                    result.failed.push((id, "track not found".into()));
                    continue;
                }
                Err(e) => {
                    result.failed.push((id, e.to_string()));
                    continue;
                }
            };
            let Some(path) = track.folder_path.clone() else {
                result.failed.push((id, "track has no file path".into()));
                continue;
            };

            let fields = payload(&track, &selection);
            if is_empty(&fields) {
                result.skipped.push(id);
                continue;
            }
            let path = mappings.resolve(&path);
            match audio_tags::write_tag_fields(&path, &fields) {
                Ok(()) => result.written.push(id),
                Err(e) => result.failed.push((id, e.to_string())),
            }
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> decks_core::rekordbox_db::Track {
        decks_core::rekordbox_db::Track {
            id: "t1".into(),
            title: "Get Lucky".into(),
            artist: Some("Daft Punk".into()),
            album: None,
            genre: Some("  ".into()),
            musical_key: Some("12M".into()),
            bpm: Some(128.0),
            duration_secs: Some(300),
            rating: None,
            comment: None,
            folder_path: Some("/m/a.mp3".into()),
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: Some(2013),
            dj_play_count: None,
            energy: None,
        }
    }

    fn all() -> TagFieldSelection {
        TagFieldSelection {
            title: true,
            artist: true,
            album: true,
            genre: true,
            bpm: true,
            musical_key: true,
            comment: true,
            year: true,
        }
    }

    #[test]
    fn unselected_fields_are_left_untouched() {
        let selection = TagFieldSelection {
            title: true,
            ..Default::default()
        };
        let p = payload(&track(), &selection);
        assert_eq!(p.title.as_deref(), Some("Get Lucky"));
        assert!(p.artist.is_none());
        assert!(p.bpm.is_none());
        assert!(p.year.is_none());
    }

    #[test]
    fn a_selected_but_empty_field_is_not_written_over_a_good_tag() {
        // The library's genre here is whitespace. Writing it would blank a real
        // genre in the file with nothing.
        let p = payload(&track(), &all());
        assert!(p.genre.is_none());
        assert!(p.album.is_none(), "an absent album must not be written");
    }

    #[test]
    fn selected_fields_with_values_come_through() {
        let p = payload(&track(), &all());
        assert_eq!(p.artist.as_deref(), Some("Daft Punk"));
        assert_eq!(p.bpm, Some(128.0));
        assert_eq!(p.musical_key.as_deref(), Some("12M"));
        assert_eq!(p.year, Some(2013));
    }

    #[test]
    fn a_payload_with_nothing_in_it_is_recognised_as_a_skip() {
        let mut t = track();
        t.genre = None;
        let selection = TagFieldSelection {
            genre: true,
            ..Default::default()
        };
        assert!(is_empty(&payload(&t, &selection)));
        assert!(!is_empty(&payload(&t, &all())));
    }

    #[test]
    fn selecting_nothing_is_an_error_rather_than_a_silent_no_op() {
        assert!(!TagFieldSelection::default().any());
        assert!(all().any());
    }

    #[test]
    fn a_negative_release_year_is_dropped_rather_than_wrapping() {
        let mut t = track();
        t.release_year = Some(-1);
        assert!(payload(&t, &all()).year.is_none());
    }
}
