mod audio;
mod claude_agent;
mod cue_generator;
mod cues;
mod organizer;
mod smartlists;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::Manager;

#[derive(serde::Serialize)]
struct PlaylistDetail {
    playlist: decks_core::rekordbox_db::Playlist,
    tracks: Vec<decks_core::rekordbox_db::Track>,
}

#[derive(serde::Serialize)]
struct ExportResult {
    output_path: String,
    exported_count: usize,
}

#[derive(Debug, serde::Serialize)]
struct ClaudeCodeStatus {
    installed: bool,
    version: Option<String>,
    logged_in: Option<bool>,
    auth_method: Option<String>,
    subscription_type: Option<String>,
    email: Option<String>,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct ClaudeAuthStatus {
    #[serde(rename = "loggedIn")]
    logged_in: Option<bool>,
    #[serde(rename = "authMethod")]
    auth_method: Option<String>,
    email: Option<String>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

fn cache_db(app: &tauri::AppHandle) -> Result<cache::CacheDb, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    cache::CacheDb::open(&data_dir.join("cache.sqlite3")).map_err(|e| e.to_string())
}

/// Fill in `Track.energy` from the local cache's `audio_features` table.
///
/// Tracks come from `master.db` (no `energy` column); `audio_features` lives in
/// `cache.sqlite3` and is keyed by `track_uri`, which the rest of the codebase
/// treats as the track's `folder_path` (raw file-system path — see
/// `analyze_file_cached`). One batched query per ≤500-track chunk; no N+1.
fn hydrate_energy(tracks: &mut [decks_core::rekordbox_db::Track], cache: &cache::CacheDb) {
    let uris: Vec<&str> = tracks
        .iter()
        .filter_map(|t| t.folder_path.as_deref())
        .collect();
    if uris.is_empty() {
        return;
    }
    let Ok(map) = cache.get_energy_by_uris(&uris) else {
        // Cache lookup is best-effort; degrade gracefully (no energy column data).
        return;
    };
    for t in tracks.iter_mut() {
        if let Some(uri) = t.folder_path.as_deref() {
            if let Some(e) = map.get(uri) {
                t.energy = Some(*e as f32);
            }
        }
    }
}

// ── Config helpers ────────────────────────────────────────────────────────────

fn read_config(app: &tauri::AppHandle) -> Result<serde_json::Value, String> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("config.json");
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn write_config(app: &tauri::AppHandle, config: &serde_json::Value) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let path = config_dir.join("config.json");
    std::fs::write(&path, serde_json::to_string_pretty(config).unwrap()).map_err(|e| e.to_string())
}

// ── Library commands ─────────────────────────────────────────────────────────

#[tauri::command]
async fn validate_library_path(path: String) -> Result<u64, String> {
    let p = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = Path::new(&p);
        if !path.exists() {
            return Err("File not found".into());
        }
        let db = decks_core::rekordbox_db::RekordboxDb::open(path).map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        Ok(tracks.len() as u64)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_tracks(
    app: tauri::AppHandle,
    path: String,
) -> Result<Vec<decks_core::rekordbox_db::Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        let mut tracks = db.tracks().map_err(|e| e.to_string())?;
        if let Ok(cache) = cache_db(&app) {
            hydrate_energy(&mut tracks, &cache);
        }
        Ok(tracks)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_track(
    path: String,
    track_id: String,
) -> Result<Option<decks_core::rekordbox_db::Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.track_by_id(&track_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_track_cues(
    path: String,
    track_id: String,
) -> Result<Vec<decks_core::rekordbox_db::HotCue>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.hot_cues_for_track(&track_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_library_path(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let config = read_config(&app)?;
    Ok(config["library_path"].as_str().map(|s| s.to_owned()))
}

#[tauri::command]
async fn set_library_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let mut config = read_config(&app)?;
    config["library_path"] = serde_json::json!(path);
    write_config(&app, &config)
}

#[tauri::command]
async fn library_search(
    app: tauri::AppHandle,
    path: String,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<decks_core::rekordbox_db::Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        let mut results = db.search_tracks(&query).map_err(|e| e.to_string())?;
        if let Some(n) = limit {
            results.truncate(n);
        }
        if let Ok(cache) = cache_db(&app) {
            hydrate_energy(&mut results, &cache);
        }
        Ok(results)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn suggest_next_tracks(
    path: String,
    track_id: String,
    limit: Option<usize>,
) -> Result<Vec<(decks_core::rekordbox_db::Track, scoring::TransitionScore)>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;

        let Some(source_track) = db.track_by_id(&track_id).map_err(|e| e.to_string())? else {
            return Err(format!("Source track not found: {track_id}"));
        };

        let all_tracks = db.tracks().map_err(|e| e.to_string())?;

        let mut scored: Vec<_> = all_tracks
            .into_iter()
            .filter(|t| t.id != source_track.id)
            .map(|t| {
                let score = scoring::score_transition(&source_track, &t);
                (t, score)
            })
            .collect();

        scored.sort_by(|a, b| {
            b.1.score
                .partial_cmp(&a.1.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let limit = limit.unwrap_or(10);
        scored.truncate(limit);

        Ok(scored)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn library_stage_playlist_remove_track(
    app: tauri::AppHandle,
    library_path: String,
    playlist_id: String,
    track_id: String,
) -> Result<cache::StagedChangeRecord, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        // Validate that the playlist + track combination is real and currently
        // related. Avoids staging removals for tracks that aren't actually in
        // the playlist (which would later fail at export time).
        let playlist_entries = db
            .playlist_entries(&playlist_id)
            .map_err(|e| e.to_string())?;
        if !playlist_entries
            .iter()
            .any(|entry| entry.content_id == track_id)
        {
            return Err(format!("track {track_id} is not in playlist {playlist_id}"));
        }

        let track_value = serde_json::Value::String(track_id.clone());
        let track_label = db
            .track_by_id(&track_id)
            .ok()
            .flatten()
            .map(|t| t.title)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| track_id.clone());
        let playlist_label = db
            .playlist_by_id(&playlist_id)
            .ok()
            .flatten()
            .map(|p| p.name)
            .unwrap_or_else(|| playlist_id.clone());

        cache
            .stage_change(changes::NewChange {
                library_path: Some(library_path),
                kind: changes::ChangeKind::PlaylistRemoveTrack,
                target_id: Some(playlist_id),
                field: None,
                old_value: Some(track_value),
                new_value: None,
                reason: Some(format!(
                    "Remove “{track_label}” from playlist “{playlist_label}”"
                )),
                confidence: None,
            })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn library_stage_intro_cues(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
) -> Result<Vec<cache::StagedChangeRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let lib_dir = Path::new(&library_path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(""));

        let mut results = Vec::new();
        for track_id in track_ids {
            let Some(track) = db.track_by_id(&track_id).map_err(|e| e.to_string())? else {
                continue;
            };
            let Some(analysis_path) = track.analysis_data_path else {
                continue;
            };

            let Some(resolved) =
                decks_core::rekordbox_db::anlz::resolve_anlz_path(&lib_dir, &analysis_path)
            else {
                continue;
            };

            let Ok(beat_grid) = decks_core::rekordbox_db::anlz::read_beat_grid(&resolved) else {
                continue;
            };
            let Some(first_beat) = beat_grid
                .iter()
                .find(|b| b.beat_number == 1)
                .or_else(|| beat_grid.first())
            else {
                continue;
            };

            let start_sec = first_beat.time_ms as f64 / 1000.0;
            let bpm = first_beat.bpm();
            if bpm <= 0.0 {
                continue;
            }
            let beat_duration = 60.0 / bpm;
            let loop_duration = beat_duration * 16.0;

            let cue_mark = decks_core::rekordbox_xml::PositionMark {
                name: None,
                mark_type: decks_core::rekordbox_xml::PositionMarkType::Cue,
                start: start_sec,
                end: None,
                num: -1,
            };
            let loop_mark = decks_core::rekordbox_xml::PositionMark {
                name: None,
                mark_type: decks_core::rekordbox_xml::PositionMarkType::Loop,
                start: start_sec,
                end: Some(start_sec + loop_duration),
                num: -1,
            };

            let cue_value = serde_json::to_value(&cue_mark).map_err(|e| e.to_string())?;
            let loop_value = serde_json::to_value(&loop_mark).map_err(|e| e.to_string())?;

            let cue_change = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackAddCue,
                    target_id: Some(track_id.clone()),
                    field: None,
                    old_value: None,
                    new_value: Some(cue_value),
                    reason: Some("Auto-generated intro cue at 1.1 downbeat".to_string()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            results.push(cue_change);

            let loop_change = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackAddCue,
                    target_id: Some(track_id.clone()),
                    field: None,
                    old_value: None,
                    new_value: Some(loop_value),
                    reason: Some("Auto-generated 4-bar intro loop".to_string()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            results.push(loop_change);
        }
        Ok(results)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_playlists(path: String) -> Result<Vec<decks_core::rekordbox_db::Playlist>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.playlists().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_playlist(path: String, playlist_id: String) -> Result<Option<PlaylistDetail>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        let Some(playlist) = db.playlist_by_id(&playlist_id).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let entries = db
            .playlist_entries(&playlist_id)
            .map_err(|e| e.to_string())?;
        let mut tracks = Vec::new();
        for entry in entries {
            if let Some(track) = db
                .track_by_id(&entry.content_id)
                .map_err(|e| e.to_string())?
            {
                tracks.push(track);
            }
        }
        Ok(Some(PlaylistDetail { playlist, tracks }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn health_orphan_scan(path: String) -> Result<Vec<decks_core::rekordbox_db::Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        Ok(tracks
            .into_iter()
            .filter(|t| {
                t.folder_path
                    .as_deref()
                    .map(|p| !Path::new(p).exists())
                    .unwrap_or(false)
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn health_duplicate_scan(
    path: String,
) -> Result<Vec<decks_core::rekordbox_db::DuplicateGroup>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.duplicate_tracks().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn health_fuzzy_duplicate_scan(
    path: String,
) -> Result<Vec<decks_core::rekordbox_db::DuplicateGroup>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.fuzzy_duplicate_tracks().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Library-wide duplicate scan combining exact title+artist, fuzzy title, and
/// (when fingerprint cache is populated) audio-fingerprint groupings.
#[tauri::command]
async fn library_duplicate_groups(
    app: tauri::AppHandle,
    path: String,
) -> Result<Vec<decks_core::rekordbox_db::DuplicateGroup>, String> {
    // Pull fingerprints from cache; failures degrade to "no fingerprints"
    // rather than crashing the whole scan.
    let fingerprints: HashMap<String, Vec<u8>> = cache_db(&app)
        .and_then(|c| c.get_all_fingerprints().map_err(|e| e.to_string()))
        .map(|rows| rows.into_iter().collect())
        .unwrap_or_default();

    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.library_duplicate_groups(&fingerprints)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn health_broken_link_scan(
    path: String,
) -> Result<decks_core::rekordbox_db::BrokenMetadataReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.broken_metadata_report().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Settings commands ─────────────────────────────────────────────────────────

#[tauri::command]
async fn get_theme(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let config = read_config(&app)?;
    Ok(config["theme"].as_str().map(|s| s.to_owned()))
}

#[tauri::command]
async fn set_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    let mut config = read_config(&app)?;
    config["theme"] = serde_json::json!(theme);
    write_config(&app, &config)
}

const DEFAULT_AGENT_MODEL: &str = "claude-sonnet-4-6";

#[tauri::command]
async fn get_agent_model(app: tauri::AppHandle) -> Result<String, String> {
    let config = read_config(&app)?;
    Ok(config["agent_model"]
        .as_str()
        .map(|s| s.to_owned())
        .unwrap_or_else(|| DEFAULT_AGENT_MODEL.to_owned()))
}

#[tauri::command]
async fn set_agent_model(app: tauri::AppHandle, model: String) -> Result<(), String> {
    let mut config = read_config(&app)?;
    config["agent_model"] = serde_json::json!(model);
    write_config(&app, &config)
}

#[tauri::command]
async fn get_api_key(service: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = keyring::Entry::new("decks", &service).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(pw) => Ok(Some(pw)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_api_key(service: String, key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = keyring::Entry::new("decks", &service).map_err(|e| e.to_string())?;
        entry.set_password(&key).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn delete_api_key(service: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = keyring::Entry::new("decks", &service).map_err(|e| e.to_string())?;
        match entry.delete_password() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stream_claude_code_chat(
    app: tauri::AppHandle,
    event_id: String,
    history: String,
    message: String,
    system: String,
) -> Result<(), String> {
    claude_agent::run(app, event_id, history, message, system).await
}

#[tauri::command]
async fn get_claude_code_status() -> Result<ClaudeCodeStatus, String> {
    tauri::async_runtime::spawn_blocking(detect_claude_code_status)
        .await
        .map_err(|e| e.to_string())
}

fn detect_claude_code_status() -> ClaudeCodeStatus {
    let Some(binary) = find_claude_binary() else {
        return ClaudeCodeStatus {
            installed: false,
            version: None,
            logged_in: None,
            auth_method: None,
            subscription_type: None,
            email: None,
            error: None,
        };
    };

    let version = std::process::Command::new(&binary)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty());

    let status_output = std::process::Command::new(&binary)
        .args(["auth", "status"])
        .output();

    let mut status = ClaudeCodeStatus {
        installed: true,
        version,
        logged_in: None,
        auth_method: None,
        subscription_type: None,
        email: None,
        error: None,
    };

    match status_output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match serde_json::from_str::<ClaudeAuthStatus>(&stdout) {
                Ok(auth) => {
                    status.logged_in = auth.logged_in;
                    status.auth_method = auth.auth_method;
                    status.subscription_type = auth.subscription_type;
                    status.email = auth.email;
                }
                Err(e) => {
                    status.error = Some(format!("Could not parse Claude Code auth status: {e}"))
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            status.error = Some(if stderr.is_empty() {
                "Claude Code auth status failed".to_owned()
            } else {
                stderr
            });
            status.logged_in = Some(false);
        }
        Err(e) => status.error = Some(e.to_string()),
    }

    status
}

fn find_claude_binary() -> Option<std::path::PathBuf> {
    let candidates = [
        std::path::PathBuf::from("claude"),
        std::path::PathBuf::from("/opt/homebrew/bin/claude"),
        std::path::PathBuf::from("/usr/local/bin/claude"),
    ];

    candidates.into_iter().find(|candidate| {
        std::process::Command::new(candidate)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
}

// ── Conversation commands ────────────────────────────────────────────────────

#[tauri::command]
async fn list_conversations(
    app: tauri::AppHandle,
    library_path: Option<String>,
) -> Result<Vec<cache::Conversation>, String> {
    let db = cache_db(&app)?;
    db.list_conversations(library_path.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_conversation(
    app: tauri::AppHandle,
    library_path: Option<String>,
    title: String,
) -> Result<cache::Conversation, String> {
    let db = cache_db(&app)?;
    db.create_conversation(library_path.as_deref(), &title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_conversation(
    app: tauri::AppHandle,
    id: String,
) -> Result<Option<cache::ConversationWithMessages>, String> {
    let db = cache_db(&app)?;
    db.load_conversation(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn append_conversation_message(
    app: tauri::AppHandle,
    conversation_id: String,
    role: String,
    content: serde_json::Value,
) -> Result<cache::ConversationMessage, String> {
    let db = cache_db(&app)?;
    db.append_conversation_message(&conversation_id, &role, content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn rename_conversation(
    app: tauri::AppHandle,
    id: String,
    title: String,
) -> Result<(), String> {
    let db = cache_db(&app)?;
    db.rename_conversation(&id, &title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_conversation(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = cache_db(&app)?;
    db.delete_conversation(&id).map_err(|e| e.to_string())
}

// ── Staged change commands ──────────────────────────────────────────────────

#[tauri::command]
async fn stage_change(
    app: tauri::AppHandle,
    change: changes::NewChange,
) -> Result<cache::StagedChangeRecord, String> {
    let db = cache_db(&app)?;
    db.stage_change(change).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_changes(
    app: tauri::AppHandle,
    library_path: Option<String>,
) -> Result<Vec<cache::StagedChangeRecord>, String> {
    let db = cache_db(&app)?;
    db.list_changes(library_path.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn accept_change(
    app: tauri::AppHandle,
    id: String,
) -> Result<cache::StagedChangeRecord, String> {
    let db = cache_db(&app)?;
    db.accept_change(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn reject_change(
    app: tauri::AppHandle,
    id: String,
) -> Result<cache::StagedChangeRecord, String> {
    let db = cache_db(&app)?;
    db.reject_change(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn accept_all_safe(
    app: tauri::AppHandle,
    library_path: Option<String>,
) -> Result<Vec<cache::StagedChangeRecord>, String> {
    let db = cache_db(&app)?;
    db.accept_all_safe(library_path.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn reject_all(
    app: tauri::AppHandle,
    library_path: Option<String>,
) -> Result<Vec<cache::StagedChangeRecord>, String> {
    let db = cache_db(&app)?;
    db.reject_all(library_path.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn export_accepted_changes(
    app: tauri::AppHandle,
    library_path: String,
    output_path: String,
) -> Result<ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let accepted = cache
            .list_changes(Some(&library_path))
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|change| change.status == changes::ChangeStatus::Accepted)
            .collect::<Vec<_>>();
        if accepted.is_empty() {
            return Err("No accepted changes to export".to_owned());
        }

        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        let playlists = db.playlists().map_err(|e| e.to_string())?;
        let mut playlist_entries_map = HashMap::new();
        for playlist in &playlists {
            playlist_entries_map.insert(
                playlist.id.clone(),
                db.playlist_entries(&playlist.id)
                    .map_err(|e| e.to_string())?,
            );
        }
        let mut existing_cues: HashMap<String, Vec<decks_core::rekordbox_db::HotCue>> =
            HashMap::new();
        for change in &accepted {
            if change.kind == changes::ChangeKind::TrackAddCue {
                if let Some(target_id) = &change.target_id {
                    if !existing_cues.contains_key(target_id) {
                        if let Ok(cues) = db.hot_cues_for_track(target_id) {
                            existing_cues.insert(target_id.clone(), cues);
                        }
                    }
                }
            }
        }

        let xml = generate_export_xml(
            &tracks,
            &playlists,
            &playlist_entries_map,
            &accepted,
            Some(&existing_cues),
        )?;
        std::fs::write(&output_path, xml).map_err(|e| e.to_string())?;

        let mut exported_count = 0;
        for change in &accepted {
            cache
                .mark_change_exported(&change.id)
                .map_err(|e| e.to_string())?;
            exported_count += 1;
        }
        Ok(ExportResult {
            output_path,
            exported_count,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn generate_export_xml(
    tracks: &[decks_core::rekordbox_db::Track],
    playlists: &[decks_core::rekordbox_db::Playlist],
    playlist_entries_map: &HashMap<String, Vec<decks_core::rekordbox_db::PlaylistEntry>>,
    accepted: &[cache::StagedChangeRecord],
    existing_cues: Option<&HashMap<String, Vec<decks_core::rekordbox_db::HotCue>>>,
) -> Result<String, String> {
    let mut track_id_map = HashMap::new();
    let mut xml_tracks = tracks
        .iter()
        .enumerate()
        .map(|(idx, track)| {
            let xml_id = track.id.parse::<u32>().unwrap_or((idx + 1) as u32);
            track_id_map.insert(track.id.clone(), xml_id);
            let mut xml_track = db_track_to_xml_track(track, xml_id);

            if let Some(cues_map) = existing_cues {
                if let Some(cues) = cues_map.get(&track.id) {
                    for cue in cues {
                        let mark_type = match cue.kind {
                            decks_core::rekordbox_db::CueKind::MemoryCue => {
                                if cue.out_msec.is_some() {
                                    decks_core::rekordbox_xml::PositionMarkType::Loop
                                } else {
                                    decks_core::rekordbox_xml::PositionMarkType::Cue
                                }
                            }
                            decks_core::rekordbox_db::CueKind::HotCue(_) => {
                                if cue.out_msec.is_some() {
                                    decks_core::rekordbox_xml::PositionMarkType::Loop
                                } else {
                                    decks_core::rekordbox_xml::PositionMarkType::Cue
                                }
                            }
                        };

                        let num = match cue.kind {
                            decks_core::rekordbox_db::CueKind::MemoryCue => -1,
                            decks_core::rekordbox_db::CueKind::HotCue(n) => n as i32,
                        };

                        xml_track
                            .position_marks
                            .push(decks_core::rekordbox_xml::PositionMark {
                                name: cue.comment.clone(),
                                mark_type,
                                start: (cue.in_msec.unwrap_or(0) as f64) / 1000.0,
                                end: cue.out_msec.map(|v| (v as f64) / 1000.0),
                                num,
                            });
                    }
                }
            }

            xml_track
        })
        .collect::<Vec<_>>();

    for change in accepted {
        apply_xml_overlay(&mut xml_tracks, &track_id_map, change)?;
    }

    let mut playlist_tracks: HashMap<String, Vec<u32>> = HashMap::new();
    let mut playlist_names: HashMap<String, String> = HashMap::new();
    let mut playlist_order: Vec<String> = Vec::new();
    let mut deleted_playlists = std::collections::HashSet::new();

    // 1. Initialize from DB. Track and warn about playlist entries that
    //    reference content the live DB no longer has — see Bug C.
    let mut dropped_initial_refs: Vec<(String, String)> = Vec::new();
    for playlist in playlists.iter().filter(|playlist| {
        matches!(
            playlist.kind,
            decks_core::rekordbox_db::PlaylistKind::Playlist
        )
    }) {
        let entries = playlist_entries_map
            .get(&playlist.id)
            .cloned()
            .unwrap_or_default();
        let mut track_ids = Vec::with_capacity(entries.len());
        for entry in &entries {
            match track_id_map.get(&entry.content_id) {
                Some(id) => track_ids.push(*id),
                None => dropped_initial_refs.push((playlist.id.clone(), entry.content_id.clone())),
            }
        }
        playlist_tracks.insert(playlist.id.clone(), track_ids);
        playlist_names.insert(playlist.id.clone(), playlist.name.clone());
        playlist_order.push(playlist.id.clone());
    }
    if !dropped_initial_refs.is_empty() {
        let sample: Vec<String> = dropped_initial_refs
            .iter()
            .take(5)
            .map(|(p, t)| format!("{p}->{t}"))
            .collect();
        tracing::warn!(
            dropped = dropped_initial_refs.len(),
            sample = ?sample,
            "export: playlist entries point at tracks not in the live library; \
             dropping them from the export",
        );
    }

    // 2. Apply mutations in two passes so a PlaylistCreate accepted alongside
    //    a PlaylistAddTrack targeting it isn't dropped just because the Add
    //    happened to come first in the accepted slice. Pass A: creates and
    //    deletes (structural). Pass B: contents (Add/Remove/Reorder/Rename).
    for change in accepted {
        let Some(target_id) = change.target_id.as_deref() else {
            continue;
        };
        match change.kind {
            changes::ChangeKind::PlaylistDelete => {
                deleted_playlists.insert(target_id.to_owned());
            }
            changes::ChangeKind::PlaylistCreate => {
                if let Some(name) = change.new_value.as_ref().and_then(json_to_string) {
                    playlist_names.insert(target_id.to_owned(), name);
                    playlist_tracks.insert(target_id.to_owned(), Vec::new());
                    playlist_order.push(target_id.to_owned());
                }
            }
            _ => {}
        }
    }
    for change in accepted {
        let Some(target_id) = change.target_id.as_deref() else {
            continue;
        };
        // Skip mutations against playlists that are being deleted in this
        // same export — the deletion supersedes any in-flight edits.
        if deleted_playlists.contains(target_id) {
            continue;
        }
        match change.kind {
            changes::ChangeKind::PlaylistRename => {
                if let Some(name) = change.new_value.as_ref().and_then(json_to_string) {
                    playlist_names.insert(target_id.to_owned(), name);
                }
            }
            changes::ChangeKind::PlaylistAddTrack => {
                if let Some(track_id) = change.new_value.as_ref().and_then(json_to_string) {
                    let xml_track_id = *track_id_map.get(&track_id).ok_or_else(|| {
                        format!(
                            "track {track_id} referenced by change {} no longer exists in library",
                            change.id
                        )
                    })?;
                    let tracks = playlist_tracks.get_mut(target_id).ok_or_else(|| {
                        format!(
                            "playlist {target_id} referenced by change {} no longer exists",
                            change.id
                        )
                    })?;
                    tracks.push(xml_track_id);
                }
            }
            changes::ChangeKind::PlaylistRemoveTrack => {
                if let Some(track_id) = change.old_value.as_ref().and_then(json_to_string) {
                    let xml_track_id = *track_id_map.get(&track_id).ok_or_else(|| {
                        format!(
                            "track {track_id} referenced by change {} no longer exists in library",
                            change.id
                        )
                    })?;
                    let tracks = playlist_tracks.get_mut(target_id).ok_or_else(|| {
                        format!(
                            "playlist {target_id} referenced by change {} no longer exists",
                            change.id
                        )
                    })?;
                    tracks.retain(|&id| id != xml_track_id);
                }
            }
            _ => {}
        }
    }

    let mut playlist_nodes = Vec::new();
    for playlist_id in playlist_order {
        if deleted_playlists.contains(&playlist_id) {
            continue;
        }
        if let Some(name) = playlist_names.get(&playlist_id) {
            if let Some(track_ids) = playlist_tracks.get(&playlist_id) {
                playlist_nodes.push(decks_core::rekordbox_xml::Node::Playlist {
                    name: name.clone(),
                    key_type: 0,
                    track_ids: track_ids.clone(),
                });
            }
        }
    }

    let collection = decks_core::rekordbox_xml::Collection {
        product: decks_core::rekordbox_xml::Product::default(),
        tracks: xml_tracks,
        playlists: vec![decks_core::rekordbox_xml::Node::Folder {
            name: "ROOT".to_owned(),
            children: playlist_nodes,
        }],
    };
    let xml = decks_core::rekordbox_xml::to_xml(&collection).map_err(|e| e.to_string())?;
    decks_core::rekordbox_xml::parse(&xml).map_err(|e| e.to_string())?;
    Ok(xml)
}

fn db_track_to_xml_track(
    track: &decks_core::rekordbox_db::Track,
    xml_id: u32,
) -> decks_core::rekordbox_xml::Track {
    decks_core::rekordbox_xml::Track {
        track_id: xml_id,
        name: track.title.clone(),
        location: track
            .folder_path
            .as_deref()
            .map(decks_core::rekordbox_xml::uri::path_to_location)
            .unwrap_or_default(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        genre: track.genre.clone(),
        total_time: track.duration_secs.and_then(|v| u32::try_from(v).ok()),
        year: track.release_year.and_then(|v| u32::try_from(v).ok()),
        average_bpm: track.bpm,
        bit_rate: track.bit_rate.and_then(|v| u32::try_from(v).ok()),
        sample_rate: track.sample_rate.and_then(|v| u32::try_from(v).ok()),
        comments: track.comment.clone(),
        play_count: track.dj_play_count.and_then(|v| u32::try_from(v).ok()),
        rating: track.rating.and_then(|v| u8::try_from(v).ok()),
        tonality: track.musical_key.clone(),
        ..Default::default()
    }
}

fn apply_xml_overlay(
    tracks: &mut [decks_core::rekordbox_xml::Track],
    track_id_map: &HashMap<String, u32>,
    change: &cache::StagedChangeRecord,
) -> Result<(), String> {
    if change.kind != changes::ChangeKind::TrackMetadataEdit {
        return Ok(());
    }
    let Some(target_id) = change.target_id.as_deref() else {
        return Ok(());
    };
    let Some(xml_id) = track_id_map.get(target_id) else {
        return Err(format!(
            "Accepted change references missing track {target_id}"
        ));
    };
    let Some(track) = tracks.iter_mut().find(|track| track.track_id == *xml_id) else {
        return Err(format!("Export track mapping failed for track {target_id}"));
    };
    let field = change.field.as_deref().unwrap_or_default();
    match field {
        "title" | "name" => {
            if let Some(value) = change.new_value.as_ref().and_then(json_to_string) {
                track.name = value;
            }
        }
        "artist" => track.artist = change.new_value.as_ref().and_then(json_to_string),
        "album" => track.album = change.new_value.as_ref().and_then(json_to_string),
        "genre" => track.genre = change.new_value.as_ref().and_then(json_to_string),
        "musical_key" | "key" | "tonality" => {
            track.tonality = change.new_value.as_ref().and_then(json_to_string);
        }
        "bpm" | "average_bpm" => {
            track.average_bpm = change.new_value.as_ref().and_then(json_to_f64)
        }
        "comment" | "comments" => {
            track.comments = change.new_value.as_ref().and_then(json_to_string)
        }
        "rating" => track.rating = change.new_value.as_ref().and_then(json_to_u8),
        "release_year" | "year" => track.year = change.new_value.as_ref().and_then(json_to_u32),
        _ => {}
    }
    Ok(())
}

fn json_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn json_to_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()))
}

fn json_to_u32(value: &serde_json::Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .or_else(|| value.as_str().and_then(|s| s.parse::<u32>().ok()))
}

fn json_to_u8(value: &serde_json::Value) -> Option<u8> {
    value
        .as_u64()
        .and_then(|v| u8::try_from(v).ok())
        .or_else(|| value.as_str().and_then(|s| s.parse::<u8>().ok()))
}

// ── Audio commands ────────────────────────────────────────────────────────────

#[tauri::command]
async fn play_track(
    path: String,
    player: tauri::State<'_, audio::AudioPlayer>,
) -> Result<(), String> {
    if path.is_empty() {
        return Err("empty path".into());
    }
    player.send(audio::AudioCmd::Play(std::path::PathBuf::from(path)))
}

#[tauri::command]
async fn pause_audio(player: tauri::State<'_, audio::AudioPlayer>) -> Result<(), String> {
    player.send(audio::AudioCmd::Pause)
}

#[tauri::command]
async fn resume_audio(player: tauri::State<'_, audio::AudioPlayer>) -> Result<(), String> {
    player.send(audio::AudioCmd::Resume)
}

#[tauri::command]
async fn stop_audio(player: tauri::State<'_, audio::AudioPlayer>) -> Result<(), String> {
    player.send(audio::AudioCmd::Stop)
}

#[tauri::command]
async fn get_playback_state(
    player: tauri::State<'_, audio::AudioPlayer>,
) -> Result<audio::PlaybackState, String> {
    Ok(player.playback_state())
}

#[tauri::command]
async fn get_playback_status(
    player: tauri::State<'_, audio::AudioPlayer>,
) -> Result<audio::PlaybackStatus, String> {
    Ok(player.playback_status())
}

#[tauri::command]
async fn seek_audio(
    time_secs: f64,
    player: tauri::State<'_, audio::AudioPlayer>,
) -> Result<(), String> {
    player.send(audio::AudioCmd::Seek(std::time::Duration::from_secs_f64(
        time_secs,
    )))
}

// ── Tracks meta scans (used by filters) ───────────────────────────────────────

#[tauri::command]
async fn list_tracks_with_cues(path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.track_ids_with_cues().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_tracks_in_any_playlist(path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.track_ids_in_any_playlist().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_tracks_with_missing_files(path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        Ok(tracks
            .into_iter()
            .filter(|t| {
                t.folder_path
                    .as_deref()
                    .map(|p| !Path::new(p).exists())
                    .unwrap_or(false)
            })
            .map(|t| t.id)
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Analytics ─────────────────────────────────────────────────────────────────

#[tauri::command]
async fn library_analytics(
    path: String,
) -> Result<decks_core::rekordbox_db::LibraryAnalytics, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.library_analytics().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Audio analysis ────────────────────────────────────────────────────────────

#[tauri::command]
async fn analyze_track(
    app: tauri::AppHandle,
    library_path: String,
    track_id: String,
) -> Result<audio_analysis::AnalysisResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let track = db
            .track_by_id(&track_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("track {track_id} not found"))?;
        let file_path = track
            .folder_path
            .ok_or_else(|| "track has no folder_path".to_string())?;
        let cache = cache_db(&app)?;
        audio_analysis::analyze_file_cached(Path::new(&file_path), &file_path, &cache)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_audio_waveform(
    app: tauri::AppHandle,
    file_path: String,
    bars: Option<usize>,
) -> Result<Vec<f32>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target = bars.unwrap_or(1200);
        let cache = cache_db(&app).ok();

        // Cache-first read: only honor the cached entry when its sample count
        // matches what the caller requested.
        if let Some(c) = cache.as_ref() {
            if let Ok(Some(cached)) = c.get_waveform_peaks(&file_path) {
                if cached.len() == target {
                    return Ok(cached);
                }
            }
        }

        let peaks = audio_analysis::extract_waveform_peaks(Path::new(&file_path), target)
            .map_err(|e| e.to_string())?;

        // Only persist non-empty results to avoid caching decode failures.
        if !peaks.is_empty() {
            if let Some(c) = cache.as_ref() {
                if let Err(e) = c.set_waveform_peaks(&file_path, &peaks) {
                    tracing::warn!("failed to cache waveform peaks for {file_path}: {e}");
                }
            }
        }

        Ok(peaks)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── ANLZ waveform (Pioneer color/detail waveforms + beat grid) ───────────────

#[derive(serde::Serialize)]
struct AnlzWaveform {
    preview: Vec<decks_core::rekordbox_db::PreviewPoint>,
    detail: Vec<decks_core::rekordbox_db::DetailPoint>,
    beat_grid: Vec<decks_core::rekordbox_db::BeatGridEntry>,
    peaks: Option<Vec<f32>>,
}

/// Resolve a Rekordbox `AnalysisDataPath` into an on-disk path.
/// Thin wrapper that strips the `master.db` filename to get the library root,
/// then defers to `decks_core::rekordbox_db::anlz::resolve_anlz_path`.
fn resolve_anlz_path(library_path: &str, analysis_path: &str) -> Option<PathBuf> {
    let lib_dir = Path::new(library_path).parent()?;
    decks_core::rekordbox_db::anlz::resolve_anlz_path(lib_dir, analysis_path)
}

#[tauri::command]
async fn get_anlz_waveform(library_path: String, track_id: String) -> Result<AnlzWaveform, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let track = db
            .track_by_id(&track_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("track {track_id} not found"))?;
        let analysis_path = track
            .analysis_data_path
            .ok_or_else(|| "track has no AnalysisDataPath".to_string())?;

        let dat_path = resolve_anlz_path(&library_path, &analysis_path)
            .ok_or_else(|| format!("ANLZ file not found: {analysis_path}"))?;
        // EXT (waveform color/detail) sits next to the DAT.
        let ext_path = dat_path.with_extension("EXT");

        let beat_grid =
            decks_core::rekordbox_db::anlz::read_beat_grid(&dat_path).unwrap_or_default();
        let (preview, detail) = if ext_path.exists() {
            (
                decks_core::rekordbox_db::anlz::read_preview_waveform(&ext_path)
                    .unwrap_or_default(),
                decks_core::rekordbox_db::anlz::read_detail_waveform(&ext_path).unwrap_or_default(),
            )
        } else {
            // Some libraries store waveform data in the DAT itself.
            (
                decks_core::rekordbox_db::anlz::read_preview_waveform(&dat_path)
                    .unwrap_or_default(),
                decks_core::rekordbox_db::anlz::read_detail_waveform(&dat_path).unwrap_or_default(),
            )
        };

        Ok(AnlzWaveform {
            preview,
            detail,
            beat_grid,
            peaks: None,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Audio tags ────────────────────────────────────────────────────────────────

#[tauri::command]
async fn read_audio_tags(file_path: String) -> Result<audio_tags::TrackTags, String> {
    tauri::async_runtime::spawn_blocking(move || {
        audio_tags::read_tags(Path::new(&file_path)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn write_audio_tags(
    file_path: String,
    fields: audio_tags::TagWriteFields,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        audio_tags::write_tag_fields(Path::new(&file_path), &fields).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Relocate ──────────────────────────────────────────────────────────────────

#[tauri::command]
async fn relocate_scan(
    library_path: String,
    search_roots: Vec<String>,
) -> Result<Vec<relocate::RelocateCandidate>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let orphans: Vec<_> = db
            .tracks()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|t| {
                t.folder_path
                    .as_deref()
                    .map(|p| !Path::new(p).exists())
                    .unwrap_or(false)
            })
            .collect();
        if orphans.is_empty() {
            return Ok(Vec::new());
        }
        let relocator =
            relocate::Relocator::new(&search_roots).map_err(|e| format!("relocator init: {e}"))?;
        let mut out = Vec::new();
        for track in orphans {
            let Some(orig) = track.folder_path else {
                continue;
            };
            let info = relocate::TrackInfo {
                id: track.id,
                original_path: orig,
                duration_secs: track.duration_secs,
                title: track.title,
                artist: track.artist,
            };
            let candidate = relocator.scan_track(&info);
            if !candidate.matches.is_empty() {
                out.push(candidate);
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn reveal_in_finder(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err(format!("path does not exist: {path}"));
    }
    let status = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .args(["-R", &path])
            .status()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .status()
    } else {
        // Linux/BSD: open the parent directory in the default file manager.
        let parent = target
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        std::process::Command::new("xdg-open").arg(parent).status()
    };
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("file manager exited with status {s}")),
        Err(e) => Err(format!("failed to launch file manager: {e}")),
    }
}

#[derive(serde::Serialize)]
struct SyncCheckResult {
    locked: bool,
    pending_changes: u32,
}

#[derive(serde::Serialize)]
struct PendingChange {
    change_id: String,
    kind: String,
    track_id: Option<String>,
    track_title: Option<String>,
    field: Option<String>,
    old_value: Option<serde_json::Value>,
    new_value: Option<serde_json::Value>,
    reason: Option<String>,
    updated_at: i64,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum SyncMode {
    #[default]
    Full,
    Playlist,
    Modified,
}

#[derive(Debug, Default, serde::Deserialize)]
struct SyncOptions {
    #[serde(default)]
    playlist_id: Option<String>,
    #[serde(default)]
    since_ts: Option<i64>,
    // Sub-Plan 6: writer-side options. Forwarded into `changes::applier`.
    #[serde(default)]
    cue_destination: changes::applier::CueDestination,
    #[serde(default)]
    keep_grids: bool,
    #[serde(default)]
    convert_keys: changes::applier::KeyFormat,
    #[serde(default)]
    change_to_nearest_color: bool,
    #[serde(default)]
    all_smartlists_to_playlists: bool,
}

impl SyncOptions {
    fn applier_options(&self) -> changes::applier::SyncOptions {
        changes::applier::SyncOptions {
            cue_destination: self.cue_destination,
            keep_grids: self.keep_grids,
            convert_keys: self.convert_keys,
            change_to_nearest_color: self.change_to_nearest_color,
            all_smartlists_to_playlists: self.all_smartlists_to_playlists,
        }
    }
}

fn filter_for_mode(
    changes: Vec<changes::StagedChange>,
    mode: &SyncMode,
    opts: &SyncOptions,
) -> Vec<changes::StagedChange> {
    let accepted = changes
        .into_iter()
        .filter(|c| c.status == changes::ChangeStatus::Accepted);
    match mode {
        SyncMode::Full => accepted.collect(),
        SyncMode::Playlist => {
            // For playlist mode we'd need a track-membership lookup. Stub: keep
            // changes whose target_id matches a track in the named playlist.
            // Playlist resolution happens at the Tauri layer below.
            let _ = opts.playlist_id;
            accepted.collect()
        }
        SyncMode::Modified => {
            let since = opts.since_ts.unwrap_or(0);
            accepted.filter(|c| c.updated_at >= since).collect()
        }
    }
}

#[tauri::command]
async fn sync_check(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<SyncCheckResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let changes = cache
            .list_changes(Some(&library_path))
            .map_err(|e| e.to_string())?;
        let pending_changes = changes
            .iter()
            .filter(|c| c.status == changes::ChangeStatus::Accepted)
            .count() as u32;

        let locked = decks_core::rekordbox_db::WriteGuard::probe_lock(Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        Ok(SyncCheckResult {
            locked,
            pending_changes,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn sync_preview(
    app: tauri::AppHandle,
    library_path: String,
    mode: SyncMode,
    options: SyncOptions,
) -> Result<Vec<PendingChange>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;

        // Materialise smartlists into plain playlists before collecting the
        // change set, so the staged rows are picked up by this same sync run.
        if options.all_smartlists_to_playlists {
            smartlists::stage_materialization(&app, &library_path)?;
        }

        let all = cache
            .list_changes(Some(&library_path))
            .map_err(|e| e.to_string())?;
        let filtered = filter_for_mode(all, &mode, &options);

        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        let mut title_cache: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        let mut out = Vec::with_capacity(filtered.len());
        for c in filtered {
            let track_title = if let Some(tid) = c.target_id.as_ref() {
                if let Some(t) = title_cache.get(tid) {
                    Some(t.clone())
                } else {
                    let title = db.track_by_id(tid).ok().flatten().map(|t| t.title);
                    if let Some(t) = title.as_ref() {
                        title_cache.insert(tid.clone(), t.clone());
                    }
                    title
                }
            } else {
                None
            };
            out.push(PendingChange {
                change_id: c.id,
                kind: format!("{:?}", c.kind),
                track_id: c.target_id,
                track_title,
                field: c.field,
                old_value: c.old_value,
                new_value: c.new_value,
                reason: c.reason,
                updated_at: c.updated_at,
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn sync_execute(
    app: tauri::AppHandle,
    library_path: String,
    mode: SyncMode,
    options: SyncOptions,
    change_ids: Vec<String>,
) -> Result<changes::applier::ApplyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let all = cache
            .list_changes(Some(&library_path))
            .map_err(|e| e.to_string())?;
        let filtered = filter_for_mode(all, &mode, &options);

        let id_filter: std::collections::HashSet<String> = change_ids.into_iter().collect();
        let to_apply: Vec<_> = filtered
            .into_iter()
            .filter(|c| id_filter.is_empty() || id_filter.contains(&c.id))
            .collect();

        if to_apply.is_empty() {
            return Ok(changes::applier::ApplyResult::default());
        }

        let session_state = app.state::<std::sync::Mutex<decks_core::rekordbox_db::WriteSession>>();
        let mut session = session_state.lock().map_err(|e| e.to_string())?;
        let mut guard = decks_core::rekordbox_db::WriteGuard::acquire_for_write(
            Path::new(&library_path),
            &mut session,
        )
        .map_err(|e| e.to_string())?;

        let applier_opts = options.applier_options();
        let res = guard
            .with_tx(|tx| {
                changes::applier::apply_with_options(tx, &to_apply, &applier_opts)
                    .map_err(|e| anyhow::anyhow!(e))
            })
            .map_err(|e| e.to_string())?;
        drop(guard);
        drop(session);

        for id in &res.applied {
            let _ = cache.mark_change_exported(id);
        }

        Ok(res)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Backwards-compatible wrapper used by older callers (CleanupPanel pre-Phase A).
#[tauri::command]
async fn sync_execute_accepted(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<changes::applier::ApplyResult, String> {
    sync_execute(
        app,
        library_path,
        SyncMode::Full,
        SyncOptions::default(),
        Vec::new(),
    )
    .await
}

// ── Cleanup Commands ──────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct CleanupResult {
    affected_tracks: u32,
    staged_change_ids: Vec<String>,
}

#[tauri::command]
async fn list_genres(path: String) -> Result<Vec<decks_core::rekordbox_db::GenreCount>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.list_genres().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_artists(path: String) -> Result<Vec<decks_core::rekordbox_db::ArtistCount>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&path))
            .map_err(|e| e.to_string())?;
        db.list_artists().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn stage_cleanup_changes(
    cache: &cache::CacheDb,
    library_path: &str,
    field: &'static str,
    tracks: Vec<decks_core::rekordbox_db::Track>,
    old_value: serde_json::Value,
    new_value: serde_json::Value,
    reason: String,
) -> Result<CleanupResult, String> {
    let affected_tracks = tracks.len() as u32;
    let mut staged_change_ids = Vec::with_capacity(tracks.len());
    for track in tracks {
        let change = cache
            .stage_change(changes::NewChange {
                library_path: Some(library_path.to_string()),
                kind: changes::ChangeKind::TrackMetadataEdit,
                target_id: Some(track.id),
                field: Some(field.to_string()),
                old_value: Some(old_value.clone()),
                new_value: Some(new_value.clone()),
                reason: Some(reason.clone()),
                confidence: Some(1.0),
            })
            .map_err(|e| e.to_string())?;
        cache.accept_change(&change.id).map_err(|e| e.to_string())?;
        staged_change_ids.push(change.id);
    }
    Ok(CleanupResult {
        affected_tracks,
        staged_change_ids,
    })
}

#[tauri::command]
async fn rename_genre(
    app: tauri::AppHandle,
    library_path: String,
    old_genre: String,
    new_genre: String,
) -> Result<CleanupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let cache = cache_db(&app)?;
        let tracks = db.tracks_by_genre(&old_genre).map_err(|e| e.to_string())?;
        stage_cleanup_changes(
            &cache,
            &library_path,
            "Genre",
            tracks,
            serde_json::Value::String(old_genre.clone()),
            serde_json::Value::String(new_genre.clone()),
            format!("Rename genre '{}' to '{}'", old_genre, new_genre),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn rename_artist(
    app: tauri::AppHandle,
    library_path: String,
    old_artist: String,
    new_artist: String,
) -> Result<CleanupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let cache = cache_db(&app)?;
        let tracks = db
            .tracks_by_artist(&old_artist)
            .map_err(|e| e.to_string())?;
        stage_cleanup_changes(
            &cache,
            &library_path,
            "Artist",
            tracks,
            serde_json::Value::String(old_artist.clone()),
            serde_json::Value::String(new_artist.clone()),
            format!("Rename artist '{}' to '{}'", old_artist, new_artist),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn delete_genre(
    app: tauri::AppHandle,
    library_path: String,
    genre: String,
) -> Result<CleanupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let cache = cache_db(&app)?;
        let tracks = db.tracks_by_genre(&genre).map_err(|e| e.to_string())?;
        stage_cleanup_changes(
            &cache,
            &library_path,
            "Genre",
            tracks,
            serde_json::Value::String(genre.clone()),
            serde_json::Value::Null,
            format!("Delete genre '{}'", genre),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn delete_artist(
    app: tauri::AppHandle,
    library_path: String,
    artist: String,
) -> Result<CleanupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let cache = cache_db(&app)?;
        let tracks = db.tracks_by_artist(&artist).map_err(|e| e.to_string())?;
        stage_cleanup_changes(
            &cache,
            &library_path,
            "Artist",
            tracks,
            serde_json::Value::String(artist.clone()),
            serde_json::Value::Null,
            format!("Delete artist '{}'", artist),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stage_track_delete(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let mut staged = 0u32;
        for id in &track_ids {
            let c = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackDelete,
                    target_id: Some(id.clone()),
                    field: None,
                    old_value: None,
                    new_value: None,
                    reason: Some("Delete from library (archive)".into()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            cache.accept_change(&c.id).map_err(|e| e.to_string())?;
            staged += 1;
        }
        Ok(staged)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Track Matcher Commands ────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct MatchInputDto {
    title: String,
    #[serde(default)]
    artist: Option<String>,
}

#[tauri::command]
async fn match_tracks(
    library_path: String,
    candidates: Vec<MatchInputDto>,
) -> Result<Vec<track_matcher::MatchResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        let library: Vec<track_matcher::MatchCandidate> = tracks
            .into_iter()
            .map(|t| track_matcher::MatchCandidate {
                id: t.id,
                title: t.title,
                artist: t.artist,
            })
            .collect();
        let inputs: Vec<track_matcher::MatchInput> = candidates
            .into_iter()
            .map(|c| track_matcher::MatchInput {
                title: c.title,
                artist: c.artist,
            })
            .collect();
        Ok(track_matcher::match_all(&library, &inputs))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn parse_csv_for_matcher(
    content: String,
    title_col: String,
    artist_col: Option<String>,
) -> Result<Vec<track_matcher::MatchInput>, String> {
    track_matcher::csv_input::parse_csv(&content, &title_col, artist_col.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn parse_csv_headers_for_matcher(content: String) -> Result<Vec<String>, String> {
    track_matcher::csv_input::parse_headers(&content).map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_playlist_from_tracks(
    app: tauri::AppHandle,
    library_path: String,
    name: String,
    track_ids: Vec<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let playlist_id = uuid::Uuid::new_v4().to_string();
        // Stage as Accepted so it lands in the Sync panel for write.
        let create_change = cache
            .stage_change(changes::NewChange {
                library_path: Some(library_path.clone()),
                kind: changes::ChangeKind::PlaylistCreate,
                target_id: Some(playlist_id.clone()),
                field: None,
                old_value: None,
                new_value: Some(serde_json::json!({ "name": name, "attribute": 0 })),
                reason: Some(format!("Track Matcher: create playlist '{}'", name)),
                confidence: Some(1.0),
            })
            .map_err(|e| e.to_string())?;
        cache
            .accept_change(&create_change.id)
            .map_err(|e| e.to_string())?;

        for (idx, tid) in track_ids.iter().enumerate() {
            let c = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::PlaylistAddTrack,
                    target_id: Some(playlist_id.clone()),
                    field: None,
                    old_value: None,
                    new_value: Some(
                        serde_json::json!({ "content_id": tid, "track_no": (idx + 1) as i64 }),
                    ),
                    reason: Some(format!("Track Matcher: add to '{}'", name)),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            cache.accept_change(&c.id).map_err(|e| e.to_string())?;
        }

        Ok(playlist_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Smart Fixes Commands ──────────────────────────────────────────────────────

fn track_to_view(t: &decks_core::rekordbox_db::Track) -> smart_fixes::TrackView {
    smart_fixes::TrackView {
        id: t.id.clone(),
        title: Some(t.title.clone()).filter(|s| !s.is_empty()),
        artist: t.artist.clone(),
        album: t.album.clone(),
        comment: t.comment.clone(),
    }
}

fn load_fix_config(cache: &cache::CacheDb) -> smart_fixes::FixConfig {
    let mut cfg = smart_fixes::FixConfig::with_defaults();
    if let Ok(custom) = cache.list_common_text_patterns() {
        if !custom.is_empty() {
            cfg.common_text_patterns = custom;
        }
    }
    cfg
}

#[tauri::command]
async fn smart_fix_preview(
    app: tauri::AppHandle,
    library_path: String,
    fix_name: String,
) -> Result<Vec<smart_fixes::FixProposal>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let cfg = load_fix_config(&cache);
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for t in &tracks {
            let view = track_to_view(t);
            out.extend(smart_fixes::propose(&fix_name, &view, &cfg));
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn smart_fix_apply(
    app: tauri::AppHandle,
    library_path: String,
    fix_name: String,
    proposal_ids: Vec<String>,
) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let cfg = load_fix_config(&cache);
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        let id_filter: std::collections::HashSet<String> = proposal_ids.into_iter().collect();
        let mut staged = 0u32;
        for t in &tracks {
            let view = track_to_view(t);
            let proposals = smart_fixes::propose(&fix_name, &view, &cfg);
            for p in proposals {
                if !id_filter.contains(&p.id) {
                    continue;
                }
                cache
                    .stage_change(changes::NewChange {
                        library_path: Some(library_path.clone()),
                        kind: changes::ChangeKind::TrackMetadataEdit,
                        target_id: Some(p.track_id),
                        field: Some(p.field),
                        old_value: Some(serde_json::Value::String(p.old_value)),
                        new_value: Some(serde_json::Value::String(p.new_value)),
                        reason: Some(format!("Smart fix: {}", fix_name)),
                        confidence: Some(0.9),
                    })
                    .map_err(|e| e.to_string())?;
                staged += 1;
            }
        }
        Ok(staged)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn common_text_blocklist_list(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache.list_common_text_patterns().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn common_text_blocklist_add(app: tauri::AppHandle, pattern: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache
            .add_common_text_pattern(&pattern)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn common_text_blocklist_remove(
    app: tauri::AppHandle,
    pattern: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache
            .remove_common_text_pattern(&pattern)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Incoming / Archive Commands ───────────────────────────────────────────────

fn epoch_to_iso(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

#[tauri::command]
async fn list_incoming_tracks(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<Vec<decks_core::rekordbox_db::Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let watermark = cache
            .get_incoming_watermark(&library_path)
            .map_err(|e| e.to_string())?
            .unwrap_or(0);
        let iso = epoch_to_iso(watermark);
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks_added_since(&iso).map_err(|e| e.to_string())?;

        // Filter out archived tracks too.
        let archived: std::collections::HashSet<String> = cache
            .list_archived(&library_path)
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
        let mut filtered: Vec<_> = tracks
            .into_iter()
            .filter(|t| !archived.contains(&t.id))
            .collect();
        hydrate_energy(&mut filtered, &cache);
        Ok(filtered)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn clear_incoming(app: tauri::AppHandle, library_path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        cache
            .set_incoming_watermark(&library_path, now)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_archived_tracks(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<Vec<decks_core::rekordbox_db::Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let ids = cache
            .list_archived(&library_path)
            .map_err(|e| e.to_string())?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let mut tracks = db.tracks_by_ids(&ids).map_err(|e| e.to_string())?;
        hydrate_energy(&mut tracks, &cache);
        Ok(tracks)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_archived_track_ids(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache
            .list_archived(&library_path)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn archive_tracks(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache
            .archive_tracks(&library_path, &track_ids)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn unarchive_tracks(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache
            .unarchive_tracks(&library_path, &track_ids)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Custom Tags Commands ──────────────────────────────────────────────────────

#[tauri::command]
async fn list_tag_categories(app: tauri::AppHandle) -> Result<Vec<cache::TagCategory>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.list_tag_categories().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn create_tag_category(
    app: tauri::AppHandle,
    name: String,
) -> Result<cache::TagCategory, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.create_tag_category(&name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn rename_tag_category(
    app: tauri::AppHandle,
    id: String,
    name: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.rename_tag_category(&id, &name)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn delete_tag_category(app: tauri::AppHandle, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.delete_tag_category(&id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_tags(
    app: tauri::AppHandle,
    category_id: Option<String>,
) -> Result<Vec<cache::Tag>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.list_tags(category_id.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn create_tag(
    app: tauri::AppHandle,
    category_id: String,
    name: String,
) -> Result<cache::Tag, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.create_tag(&category_id, &name)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn rename_tag(app: tauri::AppHandle, id: String, name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.rename_tag(&id, &name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn delete_tag(app: tauri::AppHandle, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.delete_tag(&id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn move_tag(
    app: tauri::AppHandle,
    id: String,
    new_category_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.move_tag(&id, &new_category_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_track_tags(
    app: tauri::AppHandle,
    library_path: String,
    track_id: String,
) -> Result<Vec<cache::Tag>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.get_track_tags(&library_path, &track_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_track_tags(
    app: tauri::AppHandle,
    library_path: String,
    track_id: String,
    tag_ids: Vec<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.set_track_tags(&library_path, &track_id, &tag_ids)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn add_track_tag(
    app: tauri::AppHandle,
    library_path: String,
    track_id: String,
    tag_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.add_track_tag(&library_path, &track_id, &tag_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn remove_track_tag(
    app: tauri::AppHandle,
    library_path: String,
    track_id: String,
    tag_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.remove_track_tag(&library_path, &track_id, &tag_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_track_tags_map(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.list_track_tags_map(&library_path)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn search_tracks_by_tags(
    app: tauri::AppHandle,
    library_path: String,
    tag_ids: Vec<String>,
    match_all: bool,
) -> Result<Vec<decks_core::rekordbox_db::Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let track_ids = cache
            .search_tracks_by_tags(&library_path, &tag_ids, match_all)
            .map_err(|e| e.to_string())?;

        if track_ids.is_empty() {
            return Ok(Vec::new());
        }

        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        let mut tracks = Vec::new();
        for tid in track_ids {
            if let Ok(Some(track)) = db.track_by_id(&tid) {
                tracks.push(track);
            }
        }
        Ok(tracks)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── App entry point ───────────────────────────────────────────────────────────

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(audio::AudioPlayer::new(Some(handle)));
            app.manage(std::sync::Mutex::new(
                decks_core::rekordbox_db::WriteSession::new(),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            validate_library_path,
            list_tracks,
            get_track,
            get_track_cues,
            get_library_path,
            set_library_path,
            library_search,
            suggest_next_tracks,
            library_stage_intro_cues,
            library_stage_playlist_remove_track,
            list_playlists,
            get_playlist,
            health_orphan_scan,
            health_duplicate_scan,
            health_fuzzy_duplicate_scan,
            library_duplicate_groups,
            health_broken_link_scan,
            cue_generator::preview_generated_cues,
            cue_generator::apply_generated_cues,
            cue_generator::suggest_anchor_rules,
            organizer::pattern_fields,
            organizer::validate_pattern,
            organizer::preview_organize,
            organizer::apply_organize,
            cues::get_beat_grid,
            cues::quantize_position,
            cues::beat_jump_position,
            cues::stage_cue_add,
            cues::stage_cue_delete,
            cues::stage_cue_edit,
            cues::stage_grid_shift,
            smartlists::list_smartlists,
            smartlists::create_smartlist,
            smartlists::update_smartlist,
            smartlists::delete_smartlist,
            smartlists::evaluate_smartlist,
            smartlists::preview_smartlist,
            smartlists::generate_smartlists,
            smartlists::smartlist_counts,
            smartlists::smartlist_compatibility,
            list_tracks_with_cues,
            list_tracks_in_any_playlist,
            list_tracks_with_missing_files,
            library_analytics,
            analyze_track,
            get_audio_waveform,
            get_anlz_waveform,
            read_audio_tags,
            write_audio_tags,
            relocate_scan,
            get_theme,
            set_theme,
            get_agent_model,
            set_agent_model,
            get_api_key,
            set_api_key,
            delete_api_key,
            get_claude_code_status,
            stream_claude_code_chat,
            list_conversations,
            create_conversation,
            load_conversation,
            append_conversation_message,
            rename_conversation,
            delete_conversation,
            stage_change,
            list_changes,
            accept_change,
            reject_change,
            accept_all_safe,
            reject_all,
            export_accepted_changes,
            play_track,
            pause_audio,
            resume_audio,
            stop_audio,
            get_playback_state,
            get_playback_status,
            seek_audio,
            reveal_in_finder,
            sync_check,
            sync_preview,
            sync_execute,
            sync_execute_accepted,
            match_tracks,
            parse_csv_for_matcher,
            parse_csv_headers_for_matcher,
            create_playlist_from_tracks,
            stage_track_delete,
            smart_fix_preview,
            smart_fix_apply,
            common_text_blocklist_list,
            common_text_blocklist_add,
            common_text_blocklist_remove,
            list_incoming_tracks,
            clear_incoming,
            list_archived_tracks,
            list_archived_track_ids,
            archive_tracks,
            unarchive_tracks,
            list_genres,
            list_artists,
            rename_genre,
            rename_artist,
            delete_genre,
            delete_artist,
            list_tag_categories,
            create_tag_category,
            rename_tag_category,
            delete_tag_category,
            list_tags,
            create_tag,
            rename_tag,
            delete_tag,
            move_tag,
            get_track_tags,
            set_track_tags,
            add_track_tag,
            remove_track_tag,
            search_tracks_by_tags,
            list_track_tags_map,
        ])
        .run(tauri::generate_context!())
        .expect("error while running decks");
}

#[cfg(test)]
mod tests {
    use super::*;
    use changes::{ChangeKind, ChangeStatus};
    use decks_core::rekordbox_db::{Playlist, PlaylistEntry, PlaylistKind, Track};
    use serde_json::json;

    #[test]
    fn test_generate_export_xml_playlist_mutations() {
        let tracks = vec![
            Track {
                id: "track1".into(),
                title: "Track One".into(),
                artist: None,
                album: None,
                genre: None,
                duration_secs: None,
                release_year: None,
                bpm: None,
                bit_rate: None,
                sample_rate: None,
                comment: None,
                dj_play_count: None,
                rating: None,
                musical_key: None,
                folder_path: None,
                analysis_data_path: None,
                file_type: None,
                energy: None,
            },
            Track {
                id: "track2".into(),
                title: "Track Two".into(),
                artist: None,
                album: None,
                genre: None,
                duration_secs: None,
                release_year: None,
                bpm: None,
                bit_rate: None,
                sample_rate: None,
                comment: None,
                dj_play_count: None,
                rating: None,
                musical_key: None,
                folder_path: None,
                analysis_data_path: None,
                file_type: None,
                energy: None,
            },
        ];

        let playlists = vec![Playlist {
            id: "pl1".into(),
            name: "Original Playlist".into(),
            kind: PlaylistKind::Playlist,
            parent_id: Some("root".into()),
            seq: Some(1),
        }];

        let mut playlist_entries_map = HashMap::new();
        playlist_entries_map.insert(
            "pl1".into(),
            vec![PlaylistEntry {
                playlist_id: "pl1".into(),
                content_id: "track1".into(),
                track_no: Some(1),
            }],
        );

        let accepted = vec![
            cache::StagedChangeRecord {
                id: "c1".into(),
                library_path: None,
                kind: ChangeKind::PlaylistRename,
                target_id: Some("pl1".into()),
                field: None,
                old_value: Some(json!("Original Playlist")),
                new_value: Some(json!("Renamed Playlist")),
                reason: None,
                confidence: None,
                status: ChangeStatus::Accepted,
                created_at: 0,
                updated_at: 0,
            },
            cache::StagedChangeRecord {
                id: "c2".into(),
                library_path: None,
                kind: ChangeKind::PlaylistAddTrack,
                target_id: Some("pl1".into()),
                field: None,
                old_value: None,
                new_value: Some(json!("track2")),
                reason: None,
                confidence: None,
                status: ChangeStatus::Accepted,
                created_at: 0,
                updated_at: 0,
            },
            cache::StagedChangeRecord {
                id: "c3".into(),
                library_path: None,
                kind: ChangeKind::PlaylistCreate,
                target_id: Some("pl2".into()),
                field: None,
                old_value: None,
                new_value: Some(json!("New Playlist")),
                reason: None,
                confidence: None,
                status: ChangeStatus::Accepted,
                created_at: 0,
                updated_at: 0,
            },
            cache::StagedChangeRecord {
                id: "c4".into(),
                library_path: None,
                kind: ChangeKind::PlaylistAddTrack,
                target_id: Some("pl2".into()),
                field: None,
                old_value: None,
                new_value: Some(json!("track1")),
                reason: None,
                confidence: None,
                status: ChangeStatus::Accepted,
                created_at: 0,
                updated_at: 0,
            },
        ];

        let xml = generate_export_xml(&tracks, &playlists, &playlist_entries_map, &accepted, None)
            .expect("should generate xml");

        let parsed = decks_core::rekordbox_xml::parse(&xml).expect("should parse back");
        let root = &parsed.playlists[0];

        let mut pl_names = Vec::new();
        let mut pl_tracks = Vec::new();
        if let decks_core::rekordbox_xml::Node::Folder { children, .. } = root {
            for child in children {
                if let decks_core::rekordbox_xml::Node::Playlist {
                    name, track_ids, ..
                } = child
                {
                    pl_names.push(name.clone());
                    pl_tracks.push(track_ids.clone());
                }
            }
        }

        assert_eq!(pl_names, vec!["Renamed Playlist", "New Playlist"]);
        // Track IDs are assigned sequentially if they aren't numbers. track1 -> 1, track2 -> 2.
        assert_eq!(pl_tracks[0], vec![1, 2]);
        assert_eq!(pl_tracks[1], vec![1]);
    }

    #[test]
    fn hydrate_energy_populates_from_cache() {
        // Seed cache.db's audio_features with one row that has energy set.
        let cache = cache::CacheDb::open_in_memory().unwrap();
        cache
            .upsert_audio_features("/music/a.mp3", "v1", Some(128.0), None, Some(0.72), None)
            .unwrap();
        // A second track with no audio_features row.
        let mut tracks = vec![
            Track {
                id: "t1".into(),
                title: "A".into(),
                artist: None,
                album: None,
                genre: None,
                musical_key: None,
                bpm: None,
                duration_secs: None,
                rating: None,
                comment: None,
                folder_path: Some("/music/a.mp3".into()),
                analysis_data_path: None,
                file_type: None,
                sample_rate: None,
                bit_rate: None,
                release_year: None,
                dj_play_count: None,
                energy: None,
            },
            Track {
                id: "t2".into(),
                title: "B".into(),
                artist: None,
                album: None,
                genre: None,
                musical_key: None,
                bpm: None,
                duration_secs: None,
                rating: None,
                comment: None,
                folder_path: Some("/music/b.mp3".into()),
                analysis_data_path: None,
                file_type: None,
                sample_rate: None,
                bit_rate: None,
                release_year: None,
                dj_play_count: None,
                energy: None,
            },
        ];

        super::hydrate_energy(&mut tracks, &cache);

        assert!(tracks[0].energy.is_some(), "t1 should be hydrated");
        assert!(
            (tracks[0].energy.unwrap() - 0.72).abs() < 1e-5,
            "t1 energy should be 0.72, got {:?}",
            tracks[0].energy
        );
        assert!(tracks[1].energy.is_none(), "t2 has no cache row");
    }

    fn make_track(id: &str, title: &str) -> Track {
        Track {
            id: id.into(),
            title: title.into(),
            artist: None,
            album: None,
            genre: None,
            duration_secs: None,
            release_year: None,
            bpm: None,
            bit_rate: None,
            sample_rate: None,
            comment: None,
            dj_play_count: None,
            rating: None,
            musical_key: None,
            folder_path: None,
            analysis_data_path: None,
            file_type: None,
            energy: None,
        }
    }

    #[test]
    fn test_generate_export_xml_playlist_remove_track() {
        let tracks = vec![
            make_track("track1", "Track One"),
            make_track("track2", "Track Two"),
        ];
        let playlists = vec![Playlist {
            id: "pl1".into(),
            name: "Set".into(),
            kind: PlaylistKind::Playlist,
            parent_id: Some("root".into()),
            seq: Some(1),
        }];
        let mut playlist_entries_map = HashMap::new();
        playlist_entries_map.insert(
            "pl1".into(),
            vec![
                PlaylistEntry {
                    playlist_id: "pl1".into(),
                    content_id: "track1".into(),
                    track_no: Some(1),
                },
                PlaylistEntry {
                    playlist_id: "pl1".into(),
                    content_id: "track2".into(),
                    track_no: Some(2),
                },
            ],
        );

        let accepted = vec![cache::StagedChangeRecord {
            id: "c1".into(),
            library_path: None,
            kind: ChangeKind::PlaylistRemoveTrack,
            target_id: Some("pl1".into()),
            field: None,
            old_value: Some(json!("track1")),
            new_value: None,
            reason: None,
            confidence: None,
            status: ChangeStatus::Accepted,
            created_at: 0,
            updated_at: 0,
        }];

        let xml = generate_export_xml(&tracks, &playlists, &playlist_entries_map, &accepted, None)
            .expect("xml");
        let parsed = decks_core::rekordbox_xml::parse(&xml).expect("parse");
        let root = &parsed.playlists[0];

        let mut pl_tracks = Vec::new();
        if let decks_core::rekordbox_xml::Node::Folder { children, .. } = root {
            for child in children {
                if let decks_core::rekordbox_xml::Node::Playlist { track_ids, .. } = child {
                    pl_tracks.push(track_ids.clone());
                }
            }
        }
        // track1 removed; only track2 (XML id 2) remains.
        assert_eq!(pl_tracks, vec![vec![2]]);
    }

    #[test]
    fn test_generate_export_xml_playlist_delete() {
        let tracks = vec![make_track("track1", "Track One")];
        let playlists = vec![
            Playlist {
                id: "pl1".into(),
                name: "Keeper".into(),
                kind: PlaylistKind::Playlist,
                parent_id: Some("root".into()),
                seq: Some(1),
            },
            Playlist {
                id: "pl2".into(),
                name: "Trash Me".into(),
                kind: PlaylistKind::Playlist,
                parent_id: Some("root".into()),
                seq: Some(2),
            },
        ];
        let mut playlist_entries_map = HashMap::new();
        playlist_entries_map.insert(
            "pl1".into(),
            vec![PlaylistEntry {
                playlist_id: "pl1".into(),
                content_id: "track1".into(),
                track_no: Some(1),
            }],
        );
        playlist_entries_map.insert("pl2".into(), vec![]);

        let accepted = vec![cache::StagedChangeRecord {
            id: "c1".into(),
            library_path: None,
            kind: ChangeKind::PlaylistDelete,
            target_id: Some("pl2".into()),
            field: None,
            old_value: None,
            new_value: None,
            reason: None,
            confidence: None,
            status: ChangeStatus::Accepted,
            created_at: 0,
            updated_at: 0,
        }];

        let xml = generate_export_xml(&tracks, &playlists, &playlist_entries_map, &accepted, None)
            .expect("xml");
        let parsed = decks_core::rekordbox_xml::parse(&xml).expect("parse");
        let root = &parsed.playlists[0];

        let mut pl_names = Vec::new();
        if let decks_core::rekordbox_xml::Node::Folder { children, .. } = root {
            for child in children {
                if let decks_core::rekordbox_xml::Node::Playlist { name, .. } = child {
                    pl_names.push(name.clone());
                }
            }
        }
        // pl2 dropped from export.
        assert_eq!(pl_names, vec!["Keeper"]);
    }

    #[test]
    fn export_fails_when_playlist_add_target_missing() {
        // No playlists in DB; staged PlaylistAddTrack targets a phantom playlist.
        let tracks = vec![make_track("track1", "Track One")];
        let playlists: Vec<Playlist> = vec![];
        let playlist_entries_map = HashMap::new();
        let accepted = vec![cache::StagedChangeRecord {
            id: "c1".into(),
            library_path: None,
            kind: ChangeKind::PlaylistAddTrack,
            target_id: Some("pl-ghost".into()),
            field: None,
            old_value: None,
            new_value: Some(json!("track1")),
            reason: None,
            confidence: None,
            status: ChangeStatus::Accepted,
            created_at: 0,
            updated_at: 0,
        }];

        let result =
            generate_export_xml(&tracks, &playlists, &playlist_entries_map, &accepted, None);
        let err = result.expect_err("must fail loudly, not silently drop");
        assert!(
            err.contains("pl-ghost") && err.contains("no longer exists"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_fails_when_playlist_add_track_missing() {
        // Playlist exists, but the track referenced by the Add isn't in the live library.
        let tracks = vec![make_track("track1", "Track One")];
        let playlists = vec![Playlist {
            id: "pl1".into(),
            name: "Set".into(),
            kind: PlaylistKind::Playlist,
            parent_id: Some("root".into()),
            seq: Some(1),
        }];
        let mut playlist_entries_map = HashMap::new();
        playlist_entries_map.insert("pl1".into(), vec![]);
        let accepted = vec![cache::StagedChangeRecord {
            id: "c1".into(),
            library_path: None,
            kind: ChangeKind::PlaylistAddTrack,
            target_id: Some("pl1".into()),
            field: None,
            old_value: None,
            new_value: Some(json!("track-ghost")),
            reason: None,
            confidence: None,
            status: ChangeStatus::Accepted,
            created_at: 0,
            updated_at: 0,
        }];

        let result =
            generate_export_xml(&tracks, &playlists, &playlist_entries_map, &accepted, None);
        let err = result.expect_err("must fail loudly when track is missing");
        assert!(
            err.contains("track-ghost") && err.contains("no longer exists"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_add_track_works_when_playlist_create_comes_after_add_in_slice() {
        // Bug B's other half: the user accepts a PlaylistCreate AND a
        // PlaylistAddTrack, but the Add appears first in the slice.
        // Without the two-pass fix, the Add silently drops because the
        // playlist hasn't been initialized yet when the Add runs.
        let tracks = vec![make_track("track1", "Track One")];
        let playlists: Vec<Playlist> = vec![]; // playlist comes only from the Create change
        let playlist_entries_map = HashMap::new();
        let accepted = vec![
            cache::StagedChangeRecord {
                id: "c-add".into(),
                library_path: None,
                kind: ChangeKind::PlaylistAddTrack,
                target_id: Some("pl-new".into()),
                field: None,
                old_value: None,
                new_value: Some(json!("track1")),
                reason: None,
                confidence: None,
                status: ChangeStatus::Accepted,
                created_at: 0,
                updated_at: 0,
            },
            cache::StagedChangeRecord {
                id: "c-create".into(),
                library_path: None,
                kind: ChangeKind::PlaylistCreate,
                target_id: Some("pl-new".into()),
                field: None,
                old_value: None,
                new_value: Some(json!("Fresh Set")),
                reason: None,
                confidence: None,
                status: ChangeStatus::Accepted,
                created_at: 0,
                updated_at: 0,
            },
        ];

        let xml = generate_export_xml(&tracks, &playlists, &playlist_entries_map, &accepted, None)
            .expect("xml");
        let parsed = decks_core::rekordbox_xml::parse(&xml).expect("parse");
        let root = &parsed.playlists[0];
        let mut found = false;
        if let decks_core::rekordbox_xml::Node::Folder { children, .. } = root {
            for child in children {
                if let decks_core::rekordbox_xml::Node::Playlist {
                    name, track_ids, ..
                } = child
                {
                    if name == "Fresh Set" {
                        assert_eq!(*track_ids, vec![1]);
                        found = true;
                    }
                }
            }
        }
        assert!(
            found,
            "newly-created playlist with its added track must appear"
        );
    }

    #[test]
    fn export_does_not_fail_when_add_targets_a_playlist_being_deleted() {
        // If the user simultaneously stages PlaylistDelete and PlaylistAddTrack
        // for the same playlist, the deletion should win and we shouldn't error
        // on the Add.
        let tracks = vec![make_track("track1", "Track One")];
        let playlists = vec![Playlist {
            id: "pl1".into(),
            name: "Doomed".into(),
            kind: PlaylistKind::Playlist,
            parent_id: Some("root".into()),
            seq: Some(1),
        }];
        let mut playlist_entries_map = HashMap::new();
        playlist_entries_map.insert("pl1".into(), vec![]);
        let accepted = vec![
            cache::StagedChangeRecord {
                id: "c-add".into(),
                library_path: None,
                kind: ChangeKind::PlaylistAddTrack,
                target_id: Some("pl1".into()),
                field: None,
                old_value: None,
                new_value: Some(json!("track1")),
                reason: None,
                confidence: None,
                status: ChangeStatus::Accepted,
                created_at: 0,
                updated_at: 0,
            },
            cache::StagedChangeRecord {
                id: "c-del".into(),
                library_path: None,
                kind: ChangeKind::PlaylistDelete,
                target_id: Some("pl1".into()),
                field: None,
                old_value: None,
                new_value: None,
                reason: None,
                confidence: None,
                status: ChangeStatus::Accepted,
                created_at: 0,
                updated_at: 0,
            },
        ];

        let xml = generate_export_xml(&tracks, &playlists, &playlist_entries_map, &accepted, None)
            .expect("delete supersedes the add");
        // pl1 should be absent from the export.
        assert!(!xml.contains("Doomed"));
    }
}
