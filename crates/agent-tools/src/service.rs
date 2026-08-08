use crate::ToolRequest;
use anyhow::{bail, Context, Result};
use decks_core::rekordbox_db::{Playlist, RekordboxDb, Track};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct AgentToolService {
    pub cache_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct PlaylistDetail {
    playlist: Playlist,
    tracks: Vec<Track>,
}

/// Binds `enrichment`'s cache trait to `enrichment_cache`.
///
/// A mutex because `CacheDb` holds a rusqlite `Connection` — `Send` but not
/// `Sync` — while the trait must be `Sync`. Duplicated in the desktop crate for
/// the same reason; a shared adapter would mean this crate depending on the
/// desktop app, which is backwards.
struct SharedCache(std::sync::Mutex<cache::CacheDb>);

/// Thirty days, matching the desktop side. Release metadata barely moves, and
/// the two surfaces sharing a window keeps one from silently re-fetching what
/// the other just cached.
const ENRICH_CACHE_MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;

impl enrichment::ResponseCache for SharedCache {
    fn get(&self, key: &str) -> Option<Vec<enrichment::Candidate>> {
        let (provider, rest) = key.split_once('\u{1}')?;
        let json = self
            .0
            .lock()
            .ok()?
            .enrichment_cached(provider, rest, ENRICH_CACHE_MAX_AGE_SECS)
            .ok()??;
        serde_json::from_str(&json).ok()
    }

    fn put(&self, key: &str, candidates: &[enrichment::Candidate]) {
        let Some((provider, rest)) = key.split_once('\u{1}') else {
            return;
        };
        if let (Ok(json), Ok(db)) = (serde_json::to_string(candidates), self.0.lock()) {
            let _ = db.enrichment_put(provider, rest, &json);
        }
    }
}

impl AgentToolService {
    /// The cache DB, or a message naming what is missing.
    ///
    /// Undo history lives in the cache rather than in `master.db`, so a service
    /// configured without one has nothing to answer with — and saying so beats
    /// returning an empty list that reads as "you have never synced".
    fn open_undo_cache(&self) -> Result<cache::CacheDb> {
        let path = self
            .cache_path
            .as_deref()
            .context("cache_path is required for undo tools")?;
        cache::CacheDb::open(path)
    }

    pub fn execute(&self, request: ToolRequest) -> Result<Value> {
        match request {
            ToolRequest::LibrarySearch {
                library_path,
                query,
                limit,
            } => {
                let db = open_library(&library_path)?;
                let mut results = db.search_tracks(&query)?;
                if let Some(limit) = limit {
                    results.truncate(limit);
                }
                to_value(results)
            }
            ToolRequest::LibraryBulkAddIntroCues {
                library_path,
                track_ids,
            } => {
                let cache_path = self
                    .cache_path
                    .as_deref()
                    .context("cache_path is required for intro cues")?;
                let cache = cache::CacheDb::open(cache_path)?;

                let db = open_library(&library_path)?;
                let lib_dir = std::path::Path::new(&library_path)
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(""));

                let mut results = Vec::new();

                for track_id in track_ids {
                    if let Ok(Some(track)) = db.track_by_id(&track_id) {
                        if let Some(analysis_path) = track.analysis_data_path {
                            if let Some(resolved) =
                                decks_core::rekordbox_db::anlz::resolve_anlz_path(
                                    lib_dir,
                                    &analysis_path,
                                )
                            {
                                if let Ok(beat_grid) =
                                    decks_core::rekordbox_db::anlz::read_beat_grid(&resolved)
                                {
                                    if let Some(first_beat) = beat_grid
                                        .iter()
                                        .find(|b| b.beat_number == 1)
                                        .or_else(|| beat_grid.first())
                                    {
                                        let start_sec = first_beat.time_ms as f64 / 1000.0;
                                        let bpm = first_beat.bpm();
                                        if bpm > 0.0 {
                                            let beat_duration = 60.0 / bpm;
                                            let loop_duration = beat_duration * 16.0;

                                            let cue_mark = decks_core::rekordbox_xml::PositionMark {
                                                name: None,
                                                mark_type: decks_core::rekordbox_xml::PositionMarkType::Cue,
                                                start: start_sec,
                                                end: None,
                                                num: -1,
                                            };

                                            if let Ok(record) =
                                                cache.stage_change(cache::NewStagedChange {
                                                    library_path: Some(library_path.clone()),
                                                    kind: changes::ChangeKind::TrackAddCue,
                                                    target_id: Some(track_id.clone()),
                                                    field: None,
                                                    old_value: None,
                                                    new_value: Some(
                                                        serde_json::to_value(cue_mark).unwrap(),
                                                    ),
                                                    reason: Some(
                                                        "Auto-generated intro cue".to_string(),
                                                    ),
                                                    confidence: Some(1.0),
                                                })
                                            {
                                                results.push(record);
                                            }

                                            let loop_mark = decks_core::rekordbox_xml::PositionMark {
                                                name: None,
                                                mark_type: decks_core::rekordbox_xml::PositionMarkType::Loop,
                                                start: start_sec,
                                                end: Some(start_sec + loop_duration),
                                                num: -1,
                                            };

                                            if let Ok(record) =
                                                cache.stage_change(cache::NewStagedChange {
                                                    library_path: Some(library_path.clone()),
                                                    kind: changes::ChangeKind::TrackAddCue,
                                                    target_id: Some(track_id.clone()),
                                                    field: None,
                                                    old_value: None,
                                                    new_value: Some(
                                                        serde_json::to_value(loop_mark).unwrap(),
                                                    ),
                                                    reason: Some(
                                                        "Auto-generated intro loop (4 bars)"
                                                            .to_string(),
                                                    ),
                                                    confidence: Some(1.0),
                                                })
                                            {
                                                results.push(record);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                to_value(results)
            }
            ToolRequest::LibraryGetTrack { library_path, id } => {
                let db = open_library(&library_path)?;
                to_value(db.track_by_id(&id)?)
            }
            ToolRequest::LibraryListPlaylists { library_path } => {
                let db = open_library(&library_path)?;
                to_value(db.playlists()?)
            }
            ToolRequest::LibraryGetPlaylist { library_path, id } => {
                let db = open_library(&library_path)?;
                let Some(playlist) = db.playlist_by_id(&id)? else {
                    return to_value(Option::<PlaylistDetail>::None);
                };

                let tracks = db
                    .playlist_entries(&id)?
                    .into_iter()
                    .filter_map(|entry| db.track_by_id(&entry.content_id).transpose())
                    .collect::<Result<Vec<_>>>()?;

                to_value(Some(PlaylistDetail { playlist, tracks }))
            }
            ToolRequest::LibraryListCues {
                library_path,
                track_id,
            } => {
                let db = open_library(&library_path)?;
                to_value(db.hot_cues_for_track(&track_id)?)
            }
            ToolRequest::HealthOrphanScan { library_path } => {
                let db = open_library(&library_path)?;
                let tracks = db.tracks()?;
                to_value(
                    tracks
                        .into_iter()
                        .filter(|track| {
                            track
                                .folder_path
                                .as_deref()
                                .map(|path| !Path::new(path).exists())
                                .unwrap_or(false)
                        })
                        .collect::<Vec<_>>(),
                )
            }
            ToolRequest::HealthDuplicateScan { library_path } => {
                let db = open_library(&library_path)?;
                to_value(db.duplicate_tracks()?)
            }
            ToolRequest::HealthFuzzyDuplicateScan { library_path } => {
                let db = open_library(&library_path)?;
                to_value(db.fuzzy_duplicate_tracks()?)
            }
            ToolRequest::HealthBrokenLinkScan { library_path } => {
                let db = open_library(&library_path)?;
                to_value(db.broken_metadata_report()?)
            }
            ToolRequest::StagingListChanges { library_path } => {
                let cache_path = self
                    .cache_path
                    .as_deref()
                    .context("cache_path is required for staging tools")?;
                let db = cache::CacheDb::open(cache_path)?;
                to_value(db.list_changes(library_path.as_deref())?)
            }
            ToolRequest::ExportAcceptedChanges { .. } => {
                bail!("export.accepted_changes not implemented in shared service yet")
            }

            ToolRequest::LibraryReadFileTags {
                library_path,
                track_id,
            } => {
                let db = open_library(&library_path)?;
                let track = db
                    .track_by_id(&track_id)?
                    .with_context(|| format!("track {track_id} not found"))?;
                let file_path = track
                    .folder_path
                    .as_deref()
                    .context("track has no folder_path")?;
                let file_tags = audio_tags::read_tags(std::path::Path::new(file_path))
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                to_value(serde_json::json!({
                    "track_id": track_id,
                    "file_path": file_path,
                    "db": {
                        "title": track.title,
                        "artist": track.artist,
                        "album": track.album,
                        "genre": track.genre,
                        "bpm": track.bpm,
                        "musical_key": track.musical_key,
                        "comment": track.comment,
                    },
                    "file": file_tags,
                }))
            }

            ToolRequest::LibraryAnalyzeTrack {
                library_path,
                track_id,
            } => {
                let cache_path = self
                    .cache_path
                    .as_deref()
                    .context("cache_path is required for analyze_track")?;
                let db = open_library(&library_path)?;
                let track = db
                    .track_by_id(&track_id)?
                    .with_context(|| format!("track {track_id} not found"))?;
                let file_path = track
                    .folder_path
                    .as_deref()
                    .context("track has no folder_path")?
                    .to_string();
                let cache = cache::CacheDb::open(cache_path)?;
                let result = audio_analysis::analyze_file_cached(
                    std::path::Path::new(&file_path),
                    &file_path,
                    &cache,
                )
                .map_err(|e| anyhow::anyhow!("{e}"))?;
                to_value(result)
            }

            ToolRequest::LibraryFindTags {
                library_path,
                track_id,
                original_release,
                discogs_token,
            } => {
                let cache_path = self
                    .cache_path
                    .as_deref()
                    .context("cache_path is required for find_tags")?;
                let db = open_library(&library_path)?;
                let track = db
                    .track_by_id(&track_id)?
                    .with_context(|| format!("track {track_id} not found"))?;

                let q = enrichment::Query::new(track.artist.as_deref(), &track.title);
                let q = if original_release {
                    q.original_release()
                } else {
                    q
                };
                let existing = enrichment::Existing {
                    genre: track.genre.clone(),
                    year: track.release_year.map(|y| y as i32),
                    label: track.label.clone(),
                    album: track.album.clone(),
                };
                let providers = enrichment::Providers {
                    discogs_token,
                    album_art: false,
                };

                // `execute` is synchronous and the lookup is not. Running the
                // future on a runtime owned by a *fresh thread* rather than
                // with `Runtime::block_on` here is deliberate: this service is
                // called from inside a Tokio runtime in the MCP HTTP server,
                // and `block_on` inside a runtime context panics. A thread with
                // no ambient runtime cannot nest, so this is correct from both
                // the CLI (no runtime) and the server (runtime).
                let cache_path = cache_path.to_path_buf();
                let (candidates, errors) = std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    let http = enrichment::reqwest_http::ReqwestHttp::new()
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    // The MCP surface uses the same on-disk cache as the app,
                    // so a lookup made from the chat panel and one made from
                    // the CLI never pay for the same question twice.
                    let db = cache::CacheDb::open(&cache_path)?;
                    let svc =
                        enrichment::Service::new(http, SharedCache(std::sync::Mutex::new(db)));
                    Ok::<_, anyhow::Error>(rt.block_on(svc.lookup(&q, &providers)))
                })
                .join()
                .map_err(|_| anyhow::anyhow!("enrichment lookup thread panicked"))??;

                let proposal = enrichment::merge::build(&track_id, &existing, &candidates);
                to_value(serde_json::json!({
                    "track_id": track_id,
                    "proposals": proposal.proposals,
                    "tags": proposal.tags,
                    "no_match": proposal.no_match,
                    "errors": errors,
                }))
            }

            ToolRequest::LibraryScanAndProposeMissing {
                library_path,
                fields,
                limit,
            } => {
                let cache_path = self
                    .cache_path
                    .as_deref()
                    .context("cache_path is required for scan_and_propose_missing")?;
                let limit = limit.unwrap_or(20);
                let want_bpm = fields.is_empty() || fields.iter().any(|f| f == "bpm");
                let want_key = fields.is_empty() || fields.iter().any(|f| f == "key");

                let db = open_library(&library_path)?;
                let cache = cache::CacheDb::open(cache_path)?;

                // Collect candidates: tracks missing the requested fields.
                let candidates: Vec<Track> = db
                    .tracks()?
                    .into_iter()
                    .filter(|t| {
                        t.folder_path.is_some()
                            && ((want_bpm && t.bpm.is_none())
                                || (want_key && t.musical_key.is_none()))
                    })
                    .take(limit)
                    .collect();

                let mut analyzed = 0usize;
                let mut proposed = 0usize;
                let mut errors: Vec<serde_json::Value> = Vec::new();

                for track in &candidates {
                    let file_path = track.folder_path.as_deref().unwrap();
                    match audio_analysis::analyze_file_cached(
                        std::path::Path::new(file_path),
                        file_path,
                        &cache,
                    ) {
                        Ok(result) => {
                            analyzed += 1;
                            if want_bpm && track.bpm.is_none() {
                                cache
                                    .stage_change(cache::NewStagedChange {
                                        library_path: Some(library_path.clone()),
                                        kind: changes::ChangeKind::TrackMetadataEdit,
                                        target_id: Some(track.id.clone()),
                                        field: Some("bpm".to_string()),
                                        old_value: None,
                                        new_value: Some(serde_json::json!(result.bpm)),
                                        reason: Some(format!(
                                            "BPM detected by stratum-dsp (confidence {:.0}%)",
                                            result.bpm_confidence * 100.0
                                        )),
                                        confidence: Some(result.bpm_confidence),
                                    })
                                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                                proposed += 1;
                            }
                            if want_key && track.musical_key.is_none() {
                                cache
                                    .stage_change(cache::NewStagedChange {
                                        library_path: Some(library_path.clone()),
                                        kind: changes::ChangeKind::TrackMetadataEdit,
                                        target_id: Some(track.id.clone()),
                                        field: Some("musical_key".to_string()),
                                        old_value: None,
                                        new_value: Some(serde_json::json!(result.musical_key)),
                                        reason: Some(format!(
                                            "Key detected by stratum-dsp (confidence {:.0}%)",
                                            result.key_confidence * 100.0
                                        )),
                                        confidence: Some(result.key_confidence),
                                    })
                                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                                proposed += 1;
                            }
                        }
                        Err(e) => {
                            errors.push(serde_json::json!({
                                "track_id": track.id,
                                "file_path": file_path,
                                "error": e.to_string(),
                            }));
                        }
                    }
                }

                to_value(serde_json::json!({
                    "candidates": candidates.len(),
                    "analyzed": analyzed,
                    "proposed": proposed,
                    "errors": errors,
                }))
            }

            ToolRequest::MixableTracks {
                library_path,
                track_id,
                bpm_tolerance_pct,
                key_mixing_mode,
                match_key,
                include_half_double,
                must_have_cues,
                genres,
                limit,
            } => {
                use scoring::{find_mixable, KeyMixingMode, MixableContext, MixableOptions};

                let db = open_library(&library_path)?;
                let Some(source) = db.track_by_id(&track_id)? else {
                    anyhow::bail!("track not found: {track_id}");
                };
                let tracks = db.tracks()?;

                let defaults = MixableOptions::basic();
                let opts = MixableOptions {
                    // A caller asking for `0` means "any tempo"; omitting the
                    // field means "use the default", which is not the same
                    // thing and must not collapse into it.
                    bpm_tolerance_pct: match bpm_tolerance_pct {
                        Some(p) if p <= 0.0 => None,
                        Some(p) => Some(p),
                        None => defaults.bpm_tolerance_pct,
                    },
                    key_mixing_mode: match key_mixing_mode.as_deref() {
                        Some("fuzzy") => KeyMixingMode::Fuzzy,
                        _ => KeyMixingMode::HarmonicallyCompatible,
                    },
                    match_key: match_key.unwrap_or(defaults.match_key),
                    include_half_double: include_half_double
                        .unwrap_or(defaults.include_half_double),
                    must_have_cues: must_have_cues.unwrap_or(defaults.must_have_cues),
                    genres,
                    limit: limit.unwrap_or(defaults.limit),
                    ..defaults
                };

                let ctx = MixableContext {
                    tracks_with_cues: db.track_ids_with_cues()?.into_iter().collect(),
                    ..Default::default()
                };

                let matches = find_mixable(&source, &tracks, &opts, &ctx);
                to_value(serde_json::json!({
                    "source": { "id": source.id, "title": source.title,
                                "artist": source.artist, "bpm": source.bpm,
                                "key": source.musical_key },
                    "considered": tracks.len().saturating_sub(1),
                    "matches": matches.iter().map(|m| serde_json::json!({
                        "id": m.track.id,
                        "title": m.track.title,
                        "artist": m.track.artist,
                        "bpm": m.track.bpm,
                        "key": m.track.musical_key,
                        "score": m.score,
                        "reasons": m.reasons,
                        "bpm_relation": m.bpm_relation,
                    })).collect::<Vec<_>>(),
                }))
            }

            ToolRequest::HealthPlayableScan {
                library_path,
                depth,
            } => {
                use audio_analysis::playable::{verify_playable, CheckDepth};
                // Default to the cheap check: an agent that asks for "broken
                // tracks" should not silently spend an hour decoding a library.
                let depth = match depth.as_deref() {
                    Some("full") => CheckDepth::Full,
                    _ => CheckDepth::Header,
                };
                let db = open_library(&library_path)?;
                let mut broken = Vec::new();
                for track in db.tracks()? {
                    let Some(path) = track.folder_path.as_deref() else {
                        continue;
                    };
                    let status = verify_playable(std::path::Path::new(path), depth);
                    if status.is_broken() {
                        broken.push(serde_json::json!({
                            "id": track.id,
                            "title": track.title,
                            "artist": track.artist,
                            "path": path,
                            "status": status,
                        }));
                    }
                }
                to_value(broken)
            }
            ToolRequest::UndoList { library_path } => {
                let cache = self.open_undo_cache()?;
                to_value(cache.list_undo_runs(&library_path)?)
            }
            ToolRequest::UndoEntries { run_id } => {
                let cache = self.open_undo_cache()?;
                to_value(cache.undo_run_entries(&run_id)?)
            }
            ToolRequest::UndoRun {
                library_path,
                run_id,
            } => {
                let cache = self.open_undo_cache()?;
                to_value(cache.stage_undo_run(&library_path, &run_id)?)
            }
            ToolRequest::SmartlistList { library_path } => {
                let cache_path = self
                    .cache_path
                    .as_deref()
                    .context("cache_path is required for smartlist tools")?;
                let db = cache::CacheDb::open(cache_path)?;
                to_value(db.list_smartlists(&library_path)?)
            }
            ToolRequest::SmartlistEvaluate { library_path, id } => {
                let cache_path = self
                    .cache_path
                    .as_deref()
                    .context("cache_path is required for smartlist tools")?;
                let cache = cache::CacheDb::open(cache_path)?;
                let list = cache
                    .get_smartlist(&library_path, &id)?
                    .with_context(|| format!("smartlist {id} not found"))?;

                let db = open_library(&library_path)?;
                let tracks = db.tracks()?;

                let needs_missing = list
                    .clauses
                    .iter()
                    .flat_map(|c| c.rules.iter())
                    .any(|r| r.field == smartlists::Field::IsFileMissing);

                let mut ctx = smartlists::EvalContext {
                    tracks_with_cues: db.track_ids_with_cues()?.into_iter().collect(),
                    tracks_in_any_playlist: db.track_ids_in_any_playlist()?.into_iter().collect(),
                    ..Default::default()
                };
                if needs_missing {
                    ctx.tracks_with_missing_files = tracks
                        .iter()
                        .filter(|t| {
                            t.folder_path
                                .as_deref()
                                .map(|p| !Path::new(p).exists())
                                .unwrap_or(false)
                        })
                        .map(|t| t.id.clone())
                        .collect();
                }
                if let Ok(ids) = cache.list_archived(&library_path) {
                    ctx.archived_tracks = ids.into_iter().collect();
                }
                if let Ok(map) = cache.list_track_tags_map(&library_path) {
                    ctx.tags_by_track = map
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().collect()))
                        .collect();
                }

                let matched: std::collections::HashSet<String> =
                    smartlists::evaluate(&list, &tracks, &ctx)
                        .into_iter()
                        .collect();
                let out: Vec<Track> = tracks
                    .into_iter()
                    .filter(|t| matched.contains(&t.id))
                    .collect();
                to_value(out)
            }
            ToolRequest::RelocateScan {
                library_path,
                search_roots,
            } => {
                let db = open_library(&library_path)?;
                let all_tracks = db.tracks()?;
                let mut orphans = Vec::new();
                for track in all_tracks {
                    if let Some(ref folder_path) = track.folder_path {
                        if !std::path::Path::new(folder_path).exists() {
                            orphans.push(track);
                        }
                    }
                }

                if orphans.is_empty() {
                    return to_value(Vec::<serde_json::Value>::new());
                }

                let relocator = relocate::Relocator::new(&search_roots)
                    .map_err(|e| anyhow::anyhow!("Failed to initialize relocator: {}", e))?;

                let mut candidates = Vec::new();
                for track in orphans {
                    if let Some(orig_path) = track.folder_path {
                        let info = relocate::TrackInfo {
                            id: track.id,
                            original_path: orig_path,
                            duration_secs: track.duration_secs,
                            title: track.title,
                            artist: track.artist,
                        };
                        let candidate = relocator.scan_track(&info);
                        if !candidate.matches.is_empty() {
                            candidates.push(candidate);
                        }
                    }
                }
                to_value(candidates)
            }

            ToolRequest::RelocateApply {
                library_path,
                track_id,
                new_path,
            } => {
                let cache_path = self
                    .cache_path
                    .as_deref()
                    .context("cache_path is required for relocate_apply")?;
                let cache = cache::CacheDb::open(cache_path)?;

                let record = cache
                    .stage_change(cache::NewStagedChange {
                        library_path: Some(library_path),
                        kind: changes::ChangeKind::TrackMetadataEdit,
                        target_id: Some(track_id),
                        field: Some("folder_path".to_string()),
                        old_value: None,
                        new_value: Some(serde_json::json!(new_path)),
                        reason: Some("Relocated missing file via agent".to_string()),
                        confidence: Some(1.0),
                    })
                    .map_err(|e| anyhow::anyhow!("{e}"))?;

                to_value(record)
            }
        }
    }
}

fn open_library(library_path: &str) -> Result<RekordboxDb> {
    RekordboxDb::open(Path::new(library_path))
}

fn to_value(value: impl Serialize) -> Result<Value> {
    serde_json::to_value(value).context("serializing tool response")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::path::Path;
    use tempfile::{NamedTempFile, TempPath};

    const RB_KEY: &str = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";
    const SCHEMA: &str = include_str!("../../rekordbox-db/src/sql/schema.sql");
    const SEED: &str = include_str!("../../rekordbox-db/src/sql/seed.sql");

    fn make_fixture_db() -> TempPath {
        make_fixture_db_with_extra("")
    }

    fn make_fixture_db_with_extra(extra_sql: &str) -> TempPath {
        let tmp = NamedTempFile::new().expect("tempfile");
        let path = tmp.into_temp_path();
        {
            let conn = writable_cipher_conn(&path);
            conn.execute_batch(SCHEMA).expect("schema");
            conn.execute_batch(SEED).expect("seed");
            conn.execute_batch(extra_sql).expect("extra sql");
        }
        path
    }

    fn writable_cipher_conn(path: &Path) -> Connection {
        let conn = Connection::open(path).expect("open writable");
        conn.execute_batch(&format!(
            "PRAGMA key = '{RB_KEY}'; PRAGMA busy_timeout = 5000;"
        ))
        .expect("pragmas");
        conn
    }

    fn service() -> AgentToolService {
        AgentToolService::default()
    }

    #[test]
    fn library_search_returns_seeded_tracks() {
        let library_path = make_fixture_db();

        let value = service()
            .execute(ToolRequest::LibrarySearch {
                library_path: library_path.display().to_string(),
                query: "Beta".to_owned(),
                limit: Some(10),
            })
            .expect("search");

        assert_eq!(value[0]["title"], "Test Track Beta");
    }

    #[test]
    fn library_get_playlist_returns_playlist_with_ordered_tracks() {
        let library_path = make_fixture_db();

        let value = service()
            .execute(ToolRequest::LibraryGetPlaylist {
                library_path: library_path.display().to_string(),
                id: "2".to_owned(),
            })
            .expect("playlist");

        assert_eq!(value["playlist"]["name"], "Techno Set");
        assert_eq!(value["tracks"][0]["id"], "1");
        assert_eq!(value["tracks"][1]["id"], "2");
    }

    #[test]
    fn library_list_cues_returns_track_cues() {
        let library_path = make_fixture_db();

        let value = service()
            .execute(ToolRequest::LibraryListCues {
                library_path: library_path.display().to_string(),
                track_id: "1".to_owned(),
            })
            .expect("cues");

        assert_eq!(value.as_array().expect("array").len(), 2);
        assert_eq!(value[0]["content_id"], "1");
        assert!(value[0]["in_msec"].as_i64() <= value[1]["in_msec"].as_i64());
    }

    #[test]
    fn health_duplicate_scan_returns_duplicate_groups() {
        let library_path = make_fixture_db_with_extra(
            "
            INSERT INTO djmdContent
                (ID, Title, ArtistID, AlbumID, GenreID, KeyID, BPM, Length, Rating, Commnt,
                 FolderPath, AnalysisDataPath, rb_local_deleted)
            VALUES
                ('dup-1', 'Test Track Alpha', 1, 1, 1, 1, 13200, 360, 4, 'duplicate',
                 '/music/alpha-copy.mp3', NULL, 0);
            ",
        );

        let value = service()
            .execute(ToolRequest::HealthDuplicateScan {
                library_path: library_path.display().to_string(),
            })
            .expect("duplicate scan");

        assert_eq!(value.as_array().expect("array").len(), 1);
        assert_eq!(value[0]["title"], "Test Track Alpha");
        assert_eq!(value[0]["tracks"].as_array().expect("tracks").len(), 2);
    }

    /// Build a minimal valid ANLZ .DAT blob with a PQTZ section.
    /// Mirrors `crates/rekordbox-db/src/anlz.rs` test helper.
    fn make_anlz_dat(beats: &[(u16, u16, u32)]) -> Vec<u8> {
        let beat_count = beats.len() as u32;
        let section_header_len: u32 = 24;
        let section_total_len = section_header_len + beat_count * 8;
        let file_header_len: u32 = 28;
        let file_total_len = file_header_len + section_total_len;

        let mut buf = Vec::new();
        buf.extend_from_slice(b"PMAI");
        buf.extend_from_slice(&file_header_len.to_be_bytes());
        buf.extend_from_slice(&file_total_len.to_be_bytes());
        buf.extend_from_slice(&[0u8; 16]);
        buf.extend_from_slice(b"PQTZ");
        buf.extend_from_slice(&section_header_len.to_be_bytes());
        buf.extend_from_slice(&section_total_len.to_be_bytes());
        buf.extend_from_slice(&[0x00, 0x80, 0x00, 0x00]);
        buf.extend_from_slice(&beat_count.to_be_bytes());
        buf.extend_from_slice(&[0u8; 4]);
        for &(beat_num, tempo, time_ms) in beats {
            buf.extend_from_slice(&beat_num.to_be_bytes());
            buf.extend_from_slice(&tempo.to_be_bytes());
            buf.extend_from_slice(&time_ms.to_be_bytes());
        }
        buf
    }

    #[test]
    fn library_bulk_add_intro_cues_stages_cue_and_loop_at_first_downbeat() {
        use std::io::Write;

        // BPM 120.00, downbeat at 4000 ms. 4-bar loop at 120 BPM
        // = 16 beats × (60/120) s = 8.0 s, so loop ends at 12.0 s.
        let anlz_dir = tempfile::tempdir().expect("tempdir");
        let anlz_path = anlz_dir.path().join("ANLZ0000.DAT");
        let blob = make_anlz_dat(&[
            (1, 12000, 4000),
            (2, 12000, 4500),
            (3, 12000, 5000),
            (4, 12000, 5500),
        ]);
        std::fs::File::create(&anlz_path)
            .unwrap()
            .write_all(&blob)
            .unwrap();

        // AnalysisDataPath stores the absolute temp path; the service's
        // `lib_dir.join(...)` is a no-op for absolute paths.
        let absolute_anlz = anlz_path.display().to_string().replace('\'', "''");
        let extra = format!(
            "INSERT INTO djmdContent
                (ID, Title, ArtistID, AlbumID, GenreID, KeyID, BPM, Length, Rating, Commnt,
                 FolderPath, AnalysisDataPath, rb_local_deleted)
             VALUES
                ('intro-1', 'Intro Cue Test', 1, 1, 1, 1, 12000, 240, 0, NULL,
                 '/music/intro.mp3', '{absolute_anlz}', 0);",
        );
        let library_path = make_fixture_db_with_extra(&extra);

        let cache_file = NamedTempFile::new().expect("cache tempfile");
        let service = AgentToolService {
            cache_path: Some(cache_file.path().to_path_buf()),
        };

        let value = service
            .execute(ToolRequest::LibraryBulkAddIntroCues {
                library_path: library_path.display().to_string(),
                track_ids: vec!["intro-1".to_string()],
            })
            .expect("intro cues");

        let records = value.as_array().expect("array of records");
        assert_eq!(
            records.len(),
            2,
            "expected one cue + one loop change, got {records:?}"
        );

        let cue_value = &records[0]["new_value"];
        assert_eq!(cue_value["mark_type"], "Cue");
        assert!((cue_value["start"].as_f64().unwrap() - 4.0).abs() < 1e-6);
        assert!(cue_value["end"].is_null());

        let loop_value = &records[1]["new_value"];
        assert_eq!(loop_value["mark_type"], "Loop");
        assert!((loop_value["start"].as_f64().unwrap() - 4.0).abs() < 1e-6);
        assert!((loop_value["end"].as_f64().unwrap() - 12.0).abs() < 1e-6);
    }

    #[test]
    fn library_bulk_add_intro_cues_skips_tracks_without_analysis_path() {
        // Seed track "3" has AnalysisDataPath = NULL.
        let library_path = make_fixture_db();
        let cache_file = NamedTempFile::new().expect("cache tempfile");
        let service = AgentToolService {
            cache_path: Some(cache_file.path().to_path_buf()),
        };

        let value = service
            .execute(ToolRequest::LibraryBulkAddIntroCues {
                library_path: library_path.display().to_string(),
                track_ids: vec!["3".to_string()],
            })
            .expect("intro cues");

        assert_eq!(value.as_array().expect("array").len(), 0);
    }

    #[test]
    fn staging_list_changes_without_library_path_returns_all_changes() {
        let cache_file = NamedTempFile::new().expect("cache tempfile");
        let cache = cache::CacheDb::open(cache_file.path()).expect("cache open");
        cache
            .stage_change(cache::NewStagedChange {
                library_path: Some("/library/a.db".to_owned()),
                kind: changes::ChangeKind::TrackMetadataEdit,
                target_id: Some("track-1".to_owned()),
                field: Some("genre".to_owned()),
                old_value: None,
                new_value: Some(serde_json::json!("House")),
                reason: None,
                confidence: None,
            })
            .expect("stage a");
        cache
            .stage_change(cache::NewStagedChange {
                library_path: Some("/library/b.db".to_owned()),
                kind: changes::ChangeKind::TrackMetadataEdit,
                target_id: Some("track-2".to_owned()),
                field: Some("genre".to_owned()),
                old_value: None,
                new_value: Some(serde_json::json!("Techno")),
                reason: None,
                confidence: None,
            })
            .expect("stage b");

        let value = AgentToolService {
            cache_path: Some(cache_file.path().to_path_buf()),
        }
        .execute(ToolRequest::StagingListChanges { library_path: None })
        .expect("list changes");

        assert_eq!(value.as_array().expect("array").len(), 2);
    }

    #[test]
    fn library_get_track_returns_full_track_payload() {
        let library_path = make_fixture_db();
        let value = service()
            .execute(ToolRequest::LibraryGetTrack {
                library_path: library_path.display().to_string(),
                id: "1".to_owned(),
            })
            .expect("get track");
        assert_eq!(value["id"], "1");
        assert_eq!(value["title"], "Test Track Alpha");
        assert_eq!(value["musical_key"], "8A");
        // BPM stored as 13200 → 132.0 after the queries::tracks conversion.
        assert!((value["bpm"].as_f64().unwrap() - 132.0).abs() < 0.001);
    }

    #[test]
    fn library_list_playlists_returns_seeded_playlists() {
        let library_path = make_fixture_db();
        let value = service()
            .execute(ToolRequest::LibraryListPlaylists {
                library_path: library_path.display().to_string(),
            })
            .expect("playlists");
        let arr = value.as_array().expect("array");
        let names: Vec<&str> = arr.iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"Root Folder"));
        assert!(names.contains(&"Techno Set"));
        assert!(names.contains(&"House Vibes"));
    }

    #[test]
    fn health_orphan_scan_returns_tracks_with_unresolvable_paths() {
        // Seed has tracks with /music/alpha.mp3 etc. that don't exist on disk → all are orphans.
        let library_path = make_fixture_db();
        let value = service()
            .execute(ToolRequest::HealthOrphanScan {
                library_path: library_path.display().to_string(),
            })
            .expect("orphans");
        let arr = value.as_array().expect("array");
        // Seed has 3 live tracks all with /music/*.mp3 paths that don't exist on disk.
        assert!(
            arr.len() >= 3,
            "expected at least 3 orphans, got {}",
            arr.len()
        );
    }

    #[test]
    fn health_broken_link_scan_returns_categorized_buckets() {
        // Returns an object with one array per problem category. We just assert the
        // expected keys exist and that each is an array — the per-bucket content is
        // exercised by rekordbox-db's own tests.
        let library_path = make_fixture_db();
        let value = service()
            .execute(ToolRequest::HealthBrokenLinkScan {
                library_path: library_path.display().to_string(),
            })
            .expect("broken");
        for key in [
            "missing_artist",
            "missing_bpm",
            "missing_genre",
            "missing_key",
            "suspicious",
        ] {
            assert!(
                value[key].is_array(),
                "expected `{key}` to be an array in {value}"
            );
        }
    }

    #[test]
    fn health_fuzzy_duplicate_scan_returns_array() {
        let library_path = make_fixture_db();
        let value = service()
            .execute(ToolRequest::HealthFuzzyDuplicateScan {
                library_path: library_path.display().to_string(),
            })
            .expect("fuzzy");
        assert!(value.is_array());
    }

    #[test]
    fn relocate_scan_finds_existing_audio_files() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::TempDir;

        let library_path = make_fixture_db();

        // Stage a search root containing a file with the same basename as one of the
        // seeded missing tracks. Seed track id=1 has folder_path "/music/alpha.mp3";
        // we plant "alpha.mp3" inside a temp dir and confirm relocate_scan finds it.
        let scan_dir = TempDir::new().expect("tmpdir");
        let target = scan_dir.path().join("alpha.mp3");
        File::create(&target)
            .expect("create file")
            .write_all(b"dummy")
            .expect("write");

        let value = service()
            .execute(ToolRequest::RelocateScan {
                library_path: library_path.display().to_string(),
                search_roots: vec![scan_dir.path().to_string_lossy().to_string()],
            })
            .expect("relocate scan");

        // Expect at least one candidate for track id=1 (alpha.mp3).
        let arr = value.as_array().expect("array");
        let found = arr.iter().any(|c| {
            c["track_id"] == "1"
                && c["matches"]
                    .as_array()
                    .map(|m| !m.is_empty())
                    .unwrap_or(false)
        });
        assert!(
            found,
            "expected a relocate candidate for track 1; got {value}"
        );
    }

    #[test]
    fn mixable_tracks_filters_by_key_and_tempo() {
        // Seed: 1 = 132 BPM / 8A, 2 = 128 BPM / 11B, 3 = 140 BPM / 8A.
        // Out of track 1, only Gamma is harmonically compatible; Beta is three
        // steps around the wheel, so the key rule drops it even though its
        // tempo is the closest of the three.
        let library_path = make_fixture_db();

        let value = service()
            .execute(ToolRequest::MixableTracks {
                library_path: library_path.display().to_string(),
                track_id: "1".to_owned(),
                bpm_tolerance_pct: Some(10.0),
                key_mixing_mode: None,
                match_key: None,
                include_half_double: None,
                must_have_cues: None,
                genres: vec![],
                limit: None,
            })
            .expect("mixable");

        let ids: Vec<&str> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .filter_map(|m| m["id"].as_str())
            .collect();
        assert_eq!(ids, vec!["3"]);
        assert_eq!(value["considered"], 2);
        assert_eq!(value["source"]["id"], "1");
    }

    #[test]
    fn mixable_tracks_without_the_key_rule_keeps_the_clashing_track() {
        let library_path = make_fixture_db();

        let value = service()
            .execute(ToolRequest::MixableTracks {
                library_path: library_path.display().to_string(),
                track_id: "1".to_owned(),
                bpm_tolerance_pct: Some(10.0),
                key_mixing_mode: None,
                match_key: Some(false),
                include_half_double: None,
                must_have_cues: None,
                genres: vec![],
                limit: None,
            })
            .expect("mixable");

        let mut ids: Vec<&str> = value["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .filter_map(|m| m["id"].as_str())
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["2", "3"]);
    }

    #[test]
    fn a_zero_bpm_tolerance_means_any_tempo_not_the_default() {
        // 0 is the caller saying "ignore tempo". Collapsing it into the default
        // would silently reinstate a 6% window they asked to remove.
        let library_path = make_fixture_db();

        let value = service()
            .execute(ToolRequest::MixableTracks {
                library_path: library_path.display().to_string(),
                track_id: "2".to_owned(),
                bpm_tolerance_pct: Some(0.0),
                key_mixing_mode: None,
                match_key: Some(false),
                include_half_double: None,
                must_have_cues: None,
                genres: vec![],
                limit: None,
            })
            .expect("mixable");

        assert_eq!(value["matches"].as_array().expect("matches").len(), 2);
    }

    #[test]
    fn smartlist_evaluate_returns_only_matching_tracks() {
        // Seed: tracks 1 and 3 are Techno, track 2 is House, track 4 is deleted.
        let library_path = make_fixture_db();
        let cache_file = NamedTempFile::new().expect("cache tempfile");
        let service = AgentToolService {
            cache_path: Some(cache_file.path().to_path_buf()),
        };
        let lib = library_path.display().to_string();

        let cache = cache::CacheDb::open(cache_file.path()).expect("cache");
        let created = cache
            .create_smartlist(
                &lib,
                "Techno",
                None,
                smartlists::Combinator::All,
                &[smartlists::Clause::single(smartlists::Rule::new(
                    smartlists::Field::Genre,
                    smartlists::Operator::Equals,
                    smartlists::Value::Text("Techno".into()),
                ))],
            )
            .expect("create smartlist");

        let value = service
            .execute(ToolRequest::SmartlistEvaluate {
                library_path: lib.clone(),
                id: created.id.clone(),
            })
            .expect("evaluate");

        let ids: Vec<String> = value
            .as_array()
            .expect("array")
            .iter()
            .map(|t| t["id"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(ids, vec!["1".to_string(), "3".to_string()]);

        // And the smartlist is discoverable through the list tool.
        let listed = service
            .execute(ToolRequest::SmartlistList { library_path: lib })
            .expect("list");
        assert_eq!(listed.as_array().expect("array").len(), 1);
    }

    #[test]
    fn smartlist_evaluate_errors_for_unknown_id() {
        let library_path = make_fixture_db();
        let cache_file = NamedTempFile::new().expect("cache tempfile");
        let service = AgentToolService {
            cache_path: Some(cache_file.path().to_path_buf()),
        };
        let err = service.execute(ToolRequest::SmartlistEvaluate {
            library_path: library_path.display().to_string(),
            id: "nope".into(),
        });
        assert!(err.is_err());
    }

    #[test]
    fn smartlist_evaluate_excludes_archived_tracks_by_default() {
        let library_path = make_fixture_db();
        let cache_file = NamedTempFile::new().expect("cache tempfile");
        let service = AgentToolService {
            cache_path: Some(cache_file.path().to_path_buf()),
        };
        let lib = library_path.display().to_string();

        let cache = cache::CacheDb::open(cache_file.path()).expect("cache");
        let created = cache
            .create_smartlist(
                &lib,
                "Techno",
                None,
                smartlists::Combinator::All,
                &[smartlists::Clause::single(smartlists::Rule::new(
                    smartlists::Field::Genre,
                    smartlists::Operator::Equals,
                    smartlists::Value::Text("Techno".into()),
                ))],
            )
            .expect("create smartlist");
        cache
            .archive_tracks(&lib, &["3".to_string()])
            .expect("archive");

        let value = service
            .execute(ToolRequest::SmartlistEvaluate {
                library_path: lib,
                id: created.id,
            })
            .expect("evaluate");
        let ids: Vec<String> = value
            .as_array()
            .expect("array")
            .iter()
            .map(|t| t["id"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(ids, vec!["1".to_string()]);
    }
}
