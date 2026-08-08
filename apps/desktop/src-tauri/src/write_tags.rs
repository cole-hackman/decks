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
use changes::field_mappings::{FieldMappings, MappingInput};
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
    /// Non-fatal notes — e.g. a mapping onto a field audio files do not have.
    /// Surfaced rather than swallowed, so a mapping that never applies is
    /// visible instead of mysteriously absent.
    pub warnings: Vec<String>,
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

/// Targets a field mapping can write to when the destination is an audio file.
///
/// Limited to what `audio_tags` can actually write — a mapping onto a frame we
/// cannot set would silently do nothing, which is worse than refusing it.
const MAPPABLE_TAG_TARGETS: &[&str] = &["Title", "Artist", "Album", "Genre", "Key", "Comment"];

/// Apply field mappings on top of the payload.
///
/// Mappings project fields Rekordbox has no frame for — energy, custom tags —
/// into ones it does. They run *after* the per-field selection, and only on
/// targets that selection did not already claim: a mapping quietly overwriting
/// a field the user explicitly ticked would be a nasty surprise.
fn apply_mappings(
    fields: &mut TagWriteFields,
    mappings: &FieldMappings,
    input: &MappingInput,
    existing: &std::collections::BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) {
    if mappings.is_empty() {
        return;
    }
    for target in mappings.targets() {
        if !MAPPABLE_TAG_TARGETS.contains(&target) {
            warnings.push(format!(
                "'{target}' cannot be written to an audio file; mapping skipped"
            ));
        }
    }

    for (target, value) in mappings.project(input, existing) {
        let slot = match target.as_str() {
            "Title" => &mut fields.title,
            "Artist" => &mut fields.artist,
            "Album" => &mut fields.album,
            "Genre" => &mut fields.genre,
            "Key" => &mut fields.musical_key,
            "Comment" => &mut fields.comment,
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some(value);
        }
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
        let cache = crate::cache_db(&app)?;
        let field_mappings = cache
            .list_field_mappings(cache::store::ID3_PROFILE)
            .unwrap_or_default();
        // `list_track_tags_map` returns tag *ids*; mappings write names and
        // filter by category name, so resolve both once for the whole batch
        // rather than per track.
        let tags_by_track = cache.list_track_tags_map(&library_path).unwrap_or_default();
        let categories: std::collections::HashMap<String, String> = cache
            .list_tag_categories()
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect();
        let tag_index: std::collections::HashMap<String, (String, String)> = cache
            .list_tags(None)
            .unwrap_or_default()
            .into_iter()
            .map(|t| {
                let category = categories.get(&t.category_id).cloned().unwrap_or_default();
                (t.id, (category, t.name))
            })
            .collect();

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

            let mut fields = payload(&track, &selection);

            // Mappings project fields the file format has no frame for, and
            // run only on targets the per-field selection did not claim.
            let existing: std::collections::BTreeMap<String, String> = [
                ("Comment", track.comment.clone()),
                ("Genre", track.genre.clone()),
            ]
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k.to_string(), v)))
            .collect();
            let input = MappingInput {
                energy: track.energy.map(|e| (e * 10.0).round() as u8),
                tags: tags_by_track
                    .get(&id)
                    .map(|ids| {
                        ids.iter()
                            .filter_map(|tag_id| tag_index.get(tag_id).cloned())
                            .collect()
                    })
                    .unwrap_or_default(),
                // Left unset until `Track` carried a colour, which meant the
                // `Colour` mapping the settings UI offered produced nothing at
                // all — a control that did not do what it said.
                colour_name: track.color.clone(),
                ..Default::default()
            };
            apply_mappings(
                &mut fields,
                &field_mappings,
                &input,
                &existing,
                &mut result.warnings,
            );

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

// ── Field mappings ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct FieldMappingRow {
    pub id: String,
    pub source: changes::field_mappings::MappingSource,
    pub target: String,
    pub overwrite: bool,
}

/// The targets a mapping can write to when the destination is an audio file.
#[tauri::command]
pub fn mappable_tag_targets() -> Vec<String> {
    MAPPABLE_TAG_TARGETS.iter().map(|s| s.to_string()).collect()
}

#[tauri::command]
pub fn list_field_mappings(app: tauri::AppHandle) -> Result<Vec<FieldMappingRow>, String> {
    Ok(crate::cache_db(&app)?
        .list_field_mapping_rows(cache::store::ID3_PROFILE)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(id, m)| FieldMappingRow {
            id,
            source: m.source,
            target: m.target,
            overwrite: m.overwrite,
        })
        .collect())
}

#[tauri::command]
pub fn create_field_mapping(
    app: tauri::AppHandle,
    source: changes::field_mappings::MappingSource,
    target: String,
    overwrite: bool,
) -> Result<String, String> {
    crate::cache_db(&app)?
        .create_field_mapping(
            cache::store::ID3_PROFILE,
            &changes::field_mappings::FieldMapping {
                source,
                target,
                overwrite,
            },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_field_mapping(app: tauri::AppHandle, id: String) -> Result<bool, String> {
    crate::cache_db(&app)?
        .delete_field_mapping(&id)
        .map_err(|e| e.to_string())
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
            label: None,
            remixer: None,
            mix: None,
            color: None,
            date_added: None,
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

    use changes::field_mappings::{FieldMapping, MappingSource};
    use std::collections::BTreeMap;

    fn mappings(rules: Vec<FieldMapping>) -> FieldMappings {
        FieldMappings::new(rules)
    }

    fn mapping_input() -> MappingInput {
        MappingInput {
            energy: Some(8),
            tags: vec![("Genre".into(), "Techno".into())],
            ..Default::default()
        }
    }

    #[test]
    fn a_mapping_fills_a_field_the_selection_did_not_claim() {
        let mut fields = payload(
            &track(),
            &TagFieldSelection {
                title: true,
                ..Default::default()
            },
        );
        let mut warnings = Vec::new();
        apply_mappings(
            &mut fields,
            &mappings(vec![FieldMapping {
                source: MappingSource::Energy,
                target: "Comment".into(),
                overwrite: true,
            }]),
            &mapping_input(),
            &BTreeMap::new(),
            &mut warnings,
        );
        assert_eq!(fields.comment.as_deref(), Some("Energy 08"));
        assert_eq!(fields.title.as_deref(), Some("Get Lucky"));
    }

    #[test]
    fn a_mapping_never_overwrites_a_field_the_user_explicitly_ticked() {
        // Quietly replacing a ticked field would be a nasty surprise.
        let mut fields = payload(&track(), &all());
        let before = fields.comment.clone();
        assert!(before.is_none(), "fixture has no comment");
        fields.comment = Some("user chose this".into());

        let mut warnings = Vec::new();
        apply_mappings(
            &mut fields,
            &mappings(vec![FieldMapping {
                source: MappingSource::Energy,
                target: "Comment".into(),
                overwrite: true,
            }]),
            &mapping_input(),
            &BTreeMap::new(),
            &mut warnings,
        );
        assert_eq!(fields.comment.as_deref(), Some("user chose this"));
    }

    #[test]
    fn a_mapping_onto_a_field_audio_files_lack_warns_rather_than_vanishing() {
        let mut fields = payload(&track(), &TagFieldSelection::default());
        let mut warnings = Vec::new();
        apply_mappings(
            &mut fields,
            &mappings(vec![FieldMapping {
                source: MappingSource::Energy,
                target: "Rating".into(),
                overwrite: true,
            }]),
            &mapping_input(),
            &BTreeMap::new(),
            &mut warnings,
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Rating"), "got: {}", warnings[0]);
    }

    #[test]
    fn custom_tags_map_into_a_writable_field() {
        let mut fields = payload(&track(), &TagFieldSelection::default());
        let mut warnings = Vec::new();
        apply_mappings(
            &mut fields,
            &mappings(vec![FieldMapping {
                source: MappingSource::AllCustomTags,
                target: "Comment".into(),
                overwrite: true,
            }]),
            &mapping_input(),
            &BTreeMap::new(),
            &mut warnings,
        );
        assert_eq!(fields.comment.as_deref(), Some("#Techno"));
    }

    #[test]
    fn no_mappings_changes_nothing() {
        let mut fields = payload(&track(), &TagFieldSelection::default());
        let mut warnings = Vec::new();
        apply_mappings(
            &mut fields,
            &FieldMappings::default(),
            &mapping_input(),
            &BTreeMap::new(),
            &mut warnings,
        );
        assert!(is_empty(&fields));
        assert!(warnings.is_empty());
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
