//! Delete from disk — the IPC surface for `file_organizer::trash`.
//!
//! The engine decides what may be deleted; this module supplies the three facts
//! it cannot know on its own — where the user's music lives, which playlists
//! still hold a track, and which other tracks point at the same file — and owns
//! the quarantine directory.
//!
//! **The feature is off until the user says where their music is.** With no
//! configured roots, `trash::plan` refuses every candidate, which is the
//! intended fail-closed behaviour rather than a bug to work around. The
//! Settings panel is where the list gets filled, and
//! [`suggest_music_roots`] makes filling it one click by reading the
//! directories the library already draws from.
//!
//! Per `docs/lexicon/06-files.md §Delete from disk`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use file_organizer::trash::{
    self, Batch, DeleteCandidate, DeletePlan, DeleteReceipt, GuardOptions, RestoreReport,
    QUARANTINE_DIR,
};
use serde::{Deserialize, Serialize};

use crate::organizer::path_mappings;
use crate::{read_config, write_config};

/// Where the confirmed music roots live in `config.json`.
const MUSIC_ROOTS: &str = "music_roots";

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

pub(crate) fn quarantine_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(QUARANTINE_DIR))
}

/// The directories the user has confirmed hold their music.
pub fn stored_music_roots(app: &tauri::AppHandle) -> Vec<PathBuf> {
    read_config(app)
        .ok()
        .and_then(|c| c[MUSIC_ROOTS].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        // Also applied on read, so a root stored before this check existed
        // does not stay in force.
        .filter(|p| check_music_root(p, home_dir(app).as_deref()).is_ok())
        .collect()
}

fn home_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    app.path().home_dir().ok()
}

/// Is this folder specific enough to be a music root?
///
/// A root decides what a bulk delete may reach, so a filesystem root, a home
/// folder, anything containing one, and a whole volume are all refused. Checked
/// on the path as typed and on where it really leads, since a link to `/` is
/// still `/`.
fn check_music_root(path: &Path, home: Option<&Path>) -> Result<(), String> {
    use std::path::Component;
    let shown = path.display();
    if !path.is_absolute() {
        return Err(format!("Music folders must be absolute paths: {shown}"));
    }
    if path.components().any(|c| c == Component::ParentDir) {
        return Err(format!("Music folders cannot contain `..`: {shown}"));
    }
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let home = home.map(|h| std::fs::canonicalize(h).unwrap_or_else(|_| h.to_path_buf()));
    for p in [path, resolved.as_path()] {
        if is_too_broad(p) || home.as_deref().is_some_and(|h| contains(p, h)) {
            return Err(format!(
                "{shown} covers more than music. Choose the folder that holds your music, such \
                 as a Music folder inside it."
            ));
        }
    }
    Ok(())
}

/// The named parts of a path, lower-cased: neither macOS nor Windows tells
/// `/Users` from `/users`.
fn names(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(n) => Some(n.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

/// Is `inner` the same folder as `outer`, or inside it?
fn contains(outer: &Path, inner: &Path) -> bool {
    names(inner).starts_with(&names(outer))
}

/// How many named components a folder needs before it is about music rather
/// than about a whole user or volume.
fn required_depth(first: &str) -> usize {
    match first.to_lowercase().as_str() {
        "users" | "volumes" | "home" => 3,
        _ => 1,
    }
}

fn is_too_broad(path: &Path) -> bool {
    let names = names(path);
    match names.first() {
        None => true,
        // macOS mirrors the whole data volume, home folders included, here.
        Some(first) if first == "system" => true,
        Some(first) => names.len() < required_depth(first),
    }
}

#[tauri::command]
pub fn music_roots(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    Ok(stored_music_roots(&app)
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// Replace the confirmed roots.
///
/// Relative paths are rejected rather than resolved: a root is a guard, and a
/// guard that depends on the process's working directory is not one. Folders
/// too broad to guard anything are rejected too; see [`check_music_root`].
#[tauri::command]
pub fn set_music_roots(app: tauri::AppHandle, roots: Vec<String>) -> Result<(), String> {
    let home = home_dir(&app);
    let mut cleaned = Vec::new();
    for root in roots {
        let trimmed = root.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        check_music_root(Path::new(&trimmed), home.as_deref())?;
        if !cleaned.contains(&trimmed) {
            cleaned.push(trimmed);
        }
    }
    let mut config = read_config(&app)?;
    config[MUSIC_ROOTS] = serde_json::json!(cleaned);
    write_config(&app, &config)
}

/// A directory the library actually draws audio from.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RootSuggestion {
    pub path: String,
    pub track_count: usize,
}

/// Suggest roots from the library's own track paths.
///
/// Groups every track's directory by a component near the top — deep enough to
/// be a real collection folder rather than `/`, a home folder or a volume,
/// shallow enough that one entry covers a whole library. Suggestions are *suggestions*:
/// nothing is stored until the user confirms them.
#[tauri::command]
pub async fn suggest_music_roots(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<Vec<RootSuggestion>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let mappings = path_mappings(&app);

        let home = home_dir(&app);
        let mut counts: HashMap<PathBuf, usize> = HashMap::new();
        for track in db.tracks().map_err(|e| e.to_string())? {
            let Some(raw) = track.folder_path.as_deref() else {
                continue;
            };
            let resolved = mappings.resolve(raw);
            let Some(dir) = resolved.parent() else {
                continue;
            };
            if let Some(root) = collection_root(dir) {
                if check_music_root(&root, home.as_deref()).is_ok() {
                    *counts.entry(root).or_default() += 1;
                }
            }
        }

        let mut out: Vec<_> = counts
            .into_iter()
            .map(|(path, track_count)| RootSuggestion {
                path: path.to_string_lossy().to_string(),
                track_count,
            })
            .collect();
        out.sort_by(|a, b| b.track_count.cmp(&a.track_count).then(a.path.cmp(&b.path)));
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The first few meaningful components of a directory, as an absolute path.
///
/// Two, or three under `/Users` and `/Volumes` where two name a whole home
/// folder or disk: `/Users/cole/Music/House` → `/Users/cole/Music`,
/// `C:\Music\House\x` → `C:\Music\House`. `None` for anything too shallow to be a useful root — suggesting `/` would
/// defeat the guard the roots exist to provide.
fn collection_root(dir: &Path) -> Option<PathBuf> {
    use std::path::Component;
    if !dir.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    let mut normal = 0usize;
    let mut wanted = 2usize;
    for component in dir.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => out.push(component.as_os_str()),
            Component::Normal(part) => {
                out.push(part);
                normal += 1;
                if normal == 1 {
                    wanted = required_depth(&part.to_string_lossy()).max(2);
                }
                if normal == wanted {
                    return Some(out);
                }
            }
            Component::CurDir | Component::ParentDir => return None,
        }
    }
    // One level under the root is still a plausible collection folder
    // (`/Music`, `D:\Tracks`); zero is not.
    (!is_too_broad(&out)).then_some(out)
}

// ── Planning and deleting ────────────────────────────────────────────────────

/// A plan, plus the titles the UI needs to name the rows.
#[derive(Debug, Serialize)]
pub struct DeletePlanView {
    #[serde(flatten)]
    pub plan: DeletePlan,
    /// Track id → "Artist — Title", so refusals read as tracks rather than
    /// as paths.
    pub labels: HashMap<String, String>,
    /// True when no music roots are configured, which is why everything was
    /// refused. Distinguished from an ordinary refusal because the fix is a
    /// different one: set up Settings, not deselect rows.
    pub no_roots_configured: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteRequest {
    pub library_path: String,
    pub track_ids: Vec<String>,
    /// Permit deleting files that playlists still reference.
    #[serde(default)]
    pub allow_playlist_members: bool,
    /// What the user was doing, recorded in the manifest.
    pub reason: String,
}

/// Gather everything the engine needs, then plan.
fn build_plan(
    app: &tauri::AppHandle,
    request: &DeleteRequest,
) -> Result<(DeletePlanView, GuardOptions), String> {
    let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&request.library_path))
        .map_err(|e| e.to_string())?;
    let mappings = path_mappings(app);

    let mut playlists_by_track: HashMap<String, Vec<String>> = HashMap::new();
    for playlist in db.playlists().map_err(|e| e.to_string())? {
        if !matches!(
            playlist.kind,
            decks_core::rekordbox_db::PlaylistKind::Playlist
        ) {
            continue;
        }
        for entry in db
            .playlist_entries(&playlist.id)
            .map_err(|e| e.to_string())?
        {
            let names = playlists_by_track.entry(entry.content_id).or_default();
            if !names.contains(&playlist.name) {
                names.push(playlist.name.clone());
            }
        }
    }

    // Which resolved paths more than one track points at. Built over the whole
    // library, not just the selection: the track that would be broken by the
    // delete is precisely the one the user did *not* select.
    let tracks = db.tracks().map_err(|e| e.to_string())?;
    let mut tracks_by_path: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for track in &tracks {
        if let Some(raw) = track.folder_path.as_deref() {
            tracks_by_path
                .entry(mappings.resolve(raw))
                .or_default()
                .push(track.id.clone());
        }
    }

    let wanted: HashSet<&String> = request.track_ids.iter().collect();
    let mut candidates = Vec::new();
    let mut labels = HashMap::new();
    for track in &tracks {
        if !wanted.contains(&track.id) {
            continue;
        }
        let resolved = track
            .folder_path
            .as_deref()
            .map(|p| mappings.resolve(p))
            .unwrap_or_default();
        let shared_with = tracks_by_path
            .get(&resolved)
            .map(|ids| {
                ids.iter()
                    .filter(|id| **id != track.id)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        labels.insert(
            track.id.clone(),
            match track.artist.as_deref() {
                Some(artist) if !artist.is_empty() => format!("{artist} — {}", track.title),
                _ => track.title.clone(),
            },
        );
        candidates.push(DeleteCandidate {
            track_id: track.id.clone(),
            path: resolved,
            playlists: playlists_by_track
                .get(&track.id)
                .cloned()
                .unwrap_or_default(),
            shared_with,
        });
    }

    let music_roots = stored_music_roots(app);
    let no_roots_configured = music_roots.is_empty();
    let opts = GuardOptions {
        music_roots,
        quarantine_root: quarantine_root(app)?,
        allow_playlist_members: request.allow_playlist_members,
    };

    let plan = trash::plan(&candidates, &opts, &trash::facts_for);
    Ok((
        DeletePlanView {
            plan,
            labels,
            no_roots_configured,
        },
        opts,
    ))
}

/// What would happen, without doing it.
///
/// The confirmation dialog is built from this — every refusal carries its own
/// sentence, so the user sees exactly which of their selected tracks will not
/// be touched and why, *before* agreeing to anything.
#[tauri::command]
pub async fn plan_delete_from_disk(
    app: tauri::AppHandle,
    request: DeleteRequest,
) -> Result<DeletePlanView, String> {
    tauri::async_runtime::spawn_blocking(move || build_plan(&app, &request).map(|(view, _)| view))
        .await
        .map_err(|e| e.to_string())?
}

/// Move the audio into the quarantine.
///
/// Re-plans from scratch rather than trusting a plan the renderer sent back:
/// between the preview and the confirmation the disk can change, and the guards
/// are only guards if they run against the state that actually exists at the
/// moment of the move.
#[tauri::command]
pub async fn delete_from_disk(
    app: tauri::AppHandle,
    request: DeleteRequest,
) -> Result<DeleteReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (view, opts) = build_plan(&app, &request)?;
        if view.plan.is_empty() {
            return Err(if view.no_roots_configured {
                "No music folders are configured — add them in Settings first.".to_string()
            } else {
                "Nothing here can be deleted from disk.".to_string()
            });
        }
        let receipt = trash::execute(
            &view.plan,
            &opts.quarantine_root,
            &request.library_path,
            &request.reason,
            now_secs(),
        )
        .map_err(|e| e.to_string())?;
        Ok(receipt)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── The quarantine ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_deleted_batches(app: tauri::AppHandle) -> Result<Vec<Batch>, String> {
    trash::list_batches(&quarantine_root(&app)?).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restore_deleted_batch(
    app: tauri::AppHandle,
    batch_id: String,
) -> Result<RestoreReport, String> {
    trash::restore(&quarantine_root(&app)?, &batch_id).map_err(|e| e.to_string())
}

/// The irreversible one.
///
/// Deliberately takes a single batch id and has no "empty everything"
/// counterpart — emptying the quarantine is always something the user asked for
/// by name.
#[tauri::command]
pub fn purge_deleted_batch(app: tauri::AppHandle, batch_id: String) -> Result<u64, String> {
    trash::purge(&quarantine_root(&app)?, &batch_id).map_err(|e| e.to_string())
}

/// Track ids whose audio is sitting in the quarantine for this library.
///
/// Lets the browser distinguish "you deleted this" from "this link is broken",
/// which are the same symptom and very different situations.
#[tauri::command]
pub fn quarantined_track_ids(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<HashMap<String, String>, String> {
    trash::quarantined_tracks(&quarantine_root(&app)?, &library_path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The filesystem root, spelled the way the platform means it.
    ///
    /// `Path::new("/Music").is_absolute()` is **false** on Windows: a rooted
    /// path with no drive prefix is relative to the *current* drive, which is
    /// exactly the ambiguity `collection_root` refuses. Hard-coding `/` here
    /// made these tests assert Unix semantics everywhere, and they failed the
    /// first time CI ran them on Windows.
    #[cfg(windows)]
    const ROOT: &str = "C:\\";
    #[cfg(not(windows))]
    const ROOT: &str = "/";

    fn abs(parts: &[&str]) -> PathBuf {
        let mut p = PathBuf::from(ROOT);
        for part in parts {
            p.push(part);
        }
        p
    }

    #[test]
    fn a_collection_root_stops_two_levels_down() {
        assert_eq!(
            collection_root(&abs(&["Music", "House", "Deep"])),
            Some(abs(&["Music", "House"]))
        );
    }

    #[test]
    fn a_home_folder_or_a_volume_is_never_suggested_whole() {
        for top in ["Users", "Volumes", "home"] {
            assert_eq!(
                collection_root(&abs(&[top, "cole", "Music", "House"])),
                Some(abs(&[top, "cole", "Music"]))
            );
            assert_eq!(collection_root(&abs(&[top, "cole"])), None);
            assert_eq!(collection_root(&abs(&[top])), None);
        }
    }

    #[test]
    fn folders_that_cover_more_than_music_are_not_accepted_as_roots() {
        let home = abs(&["srv", "people", "cole"]);
        for broad in [
            abs(&[]),
            abs(&["Users"]),
            abs(&["Users", "someone"]),
            abs(&["users", "someone"]),
            abs(&["Volumes"]),
            abs(&["Volumes", "SSD"]),
            abs(&["System", "Volumes", "Data", "Users", "cole"]),
            abs(&["srv"]),
            home.clone(),
            abs(&["SRV", "People", "Cole"]),
            home.join("Music").join(".."),
            PathBuf::from("Music"),
        ] {
            assert!(
                check_music_root(&broad, Some(&home)).is_err(),
                "{} was accepted",
                broad.display()
            );
        }
        for fine in [
            home.join("Music"),
            abs(&["Users", "someone", "Music"]),
            abs(&["Volumes", "SSD", "Music"]),
            abs(&["Music"]),
        ] {
            check_music_root(&fine, Some(&home)).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_broad_folder_is_not_accepted_as_a_root() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("music");
        std::os::unix::fs::symlink("/", &link).unwrap();
        assert!(check_music_root(&link, None).is_err());
    }

    #[test]
    fn a_shallow_directory_is_still_a_root() {
        assert_eq!(collection_root(&abs(&["Music"])), Some(abs(&["Music"])));
    }

    #[test]
    fn the_filesystem_root_is_never_suggested() {
        // Suggesting `/` — or `C:\` — would make the guard meaningless.
        assert_eq!(collection_root(Path::new(ROOT)), None);
    }

    #[test]
    fn a_relative_or_traversing_path_is_never_suggested() {
        assert_eq!(collection_root(Path::new("../Music")), None);
        assert_eq!(collection_root(Path::new("Music/House")), None);
    }

    #[cfg(windows)]
    #[test]
    fn a_drive_less_rooted_path_is_refused_on_windows() {
        // `\Music\House` is relative to whichever drive happens to be current,
        // so it cannot name a stable music root. This is the behaviour the
        // Windows CI failure taught us, and it is worth pinning rather than
        // rediscovering.
        assert_eq!(collection_root(Path::new("\\Music\\House")), None);
    }
}
