//! Delete from disk — the only operation in `decks` that destroys audio, and
//! therefore the only one built as two operations instead of one.
//!
//! Everywhere else the program is safe because it stages: a change is
//! `Proposed → Accepted → Applied`, and `Applied` is reversible from the undo
//! ledger or the pre-write backup. Neither of those can bring back a file that
//! `unlink` removed. So "delete from disk" here means **quarantine**:
//!
//! 1. [`plan`] decides what may be deleted at all, refusing rather than warning.
//! 2. [`execute`] *moves* the files into a timestamped batch folder and writes a
//!    [`Manifest`] recording where each one came from.
//! 3. [`restore`] puts a batch back.
//! 4. [`purge`] is the separate, explicit, irreversible step.
//!
//! Between 2 and 4 the user has real files they can inspect, and a manifest a
//! human can read without this program. Step 4 is never bundled into step 2 —
//! not as a checkbox, not as a "skip the trash" option. If someone wants a file
//! gone immediately they can empty the batch, which is one more deliberate act
//! than a mis-click.
//!
//! Planning is pure: it takes an oracle for the filesystem facts it needs, so
//! every refusal is unit-testable without a temp directory.
//!
//! Per `docs/lexicon/06-files.md §Delete from disk` and the divergence note in
//! `docs/lexicon/07-health.md`.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Directory name under the app data dir that holds every batch.
pub const QUARANTINE_DIR: &str = "deleted-audio";

/// Filename of the per-batch manifest.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Subdirectory of a batch that holds the quarantined audio.
const FILES_SUBDIR: &str = "files";

// ── Planning ─────────────────────────────────────────────────────────────────

/// One file the caller is asking to delete, plus the library context that
/// decides whether it may be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteCandidate {
    pub track_id: String,
    pub path: PathBuf,
    /// Playlists that still hold this track, by name.
    pub playlists: Vec<String>,
    /// Other track ids in the library whose file path resolves to this same
    /// file. Deleting it would break them too.
    pub shared_with: Vec<String>,
}

/// What the filesystem says about a candidate path.
///
/// Gathered by the caller so [`plan`] stays pure. [`facts_for`] is the real
/// implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PathFacts {
    pub exists: bool,
    /// A regular file — not a directory, not a device, not a socket.
    pub is_regular_file: bool,
    /// The path itself is a symlink. Following it would delete something the
    /// user did not name.
    pub is_symlink: bool,
    pub len: u64,
}

/// Why a candidate will not be touched.
///
/// These are refusals, not warnings: a refused candidate is absent from
/// [`DeletePlan::deletable`] and no override in this module can move it there.
/// The single exception is [`Refusal::StillInPlaylists`], which
/// [`GuardOptions::allow_playlist_members`] clears *before* planning decides —
/// so it is a choice made once, in the open, not a dialog dismissed per file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Refusal {
    /// The library holds no path for this track.
    NoPath,
    /// Nothing is there. Removing the library row is `stage_track_delete`'s
    /// job; there is no file to delete.
    Missing,
    /// A directory, or something that is not an ordinary file.
    NotARegularFile,
    /// A symlink. We would either delete the link (pointless) or follow it and
    /// destroy a file at a path the user never saw.
    Symlink,
    /// Outside every configured music root. The guard exists because a bad
    /// path mapping, or a library pointing at `/`, would otherwise let a bulk
    /// delete walk out of the music collection entirely.
    OutsideMusicRoots,
    /// Another track in the library points at this same file.
    SharedWithTracks { track_ids: Vec<String> },
    /// A playlist still references this track.
    StillInPlaylists { playlists: Vec<String> },
    /// Already inside the quarantine. Deleting a quarantined file is
    /// [`purge`]'s job and goes through its own confirmation.
    AlreadyQuarantined,
}

impl Refusal {
    /// A sentence for the UI. Kept next to the variants so a new refusal cannot
    /// ship without one.
    pub fn message(&self) -> String {
        match self {
            Refusal::NoPath => "The library holds no file path for this track.".into(),
            Refusal::Missing => "No file at that path — nothing to delete.".into(),
            Refusal::NotARegularFile => "Not an ordinary file.".into(),
            Refusal::Symlink => {
                "That path is a symbolic link. Deleting it would either leave the audio behind or \
                 destroy a file somewhere you did not name."
                    .into()
            }
            Refusal::OutsideMusicRoots => {
                "Outside every folder you have marked as music. Add its folder in Settings if this \
                 is really part of your library."
                    .into()
            }
            Refusal::SharedWithTracks { track_ids } => format!(
                "{} other track{} in the library point{} at this same file.",
                track_ids.len(),
                if track_ids.len() == 1 { "" } else { "s" },
                if track_ids.len() == 1 { "s" } else { "" },
            ),
            Refusal::StillInPlaylists { playlists } => format!(
                "Still in {}: {}.",
                if playlists.len() == 1 {
                    "a playlist".to_string()
                } else {
                    format!("{} playlists", playlists.len())
                },
                playlists.join(", "),
            ),
            Refusal::AlreadyQuarantined => {
                "Already in the deleted-audio folder. Empty that batch to remove it for good."
                    .into()
            }
        }
    }
}

/// A candidate that passed every guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Planned {
    pub track_id: String,
    pub source: PathBuf,
    pub bytes: u64,
}

/// A candidate that did not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refused {
    pub track_id: String,
    pub path: PathBuf,
    pub reason: Refusal,
    /// `reason.message()`, resolved here so the renderer does not reimplement
    /// the match.
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DeletePlan {
    pub deletable: Vec<Planned>,
    pub refused: Vec<Refused>,
    /// Bytes the quarantine will hold — *not* bytes freed. Nothing is freed
    /// until [`purge`].
    pub total_bytes: u64,
}

impl DeletePlan {
    pub fn is_empty(&self) -> bool {
        self.deletable.is_empty()
    }
}

/// The guards, as configuration rather than as prompts.
#[derive(Debug, Clone, Default)]
pub struct GuardOptions {
    /// Absolute directories the library legitimately draws audio from. A
    /// candidate outside all of them is refused.
    ///
    /// Empty means **refuse everything** — a deliberate fail-closed default, so
    /// a caller that forgets to populate it deletes nothing rather than
    /// everything.
    pub music_roots: Vec<PathBuf>,
    /// Where quarantined files live, so candidates already inside it are
    /// refused rather than nested.
    pub quarantine_root: PathBuf,
    /// Permit deleting files that playlists still reference.
    ///
    /// Off by default. On, because duplicate resolution legitimately deletes a
    /// copy that a playlist points at — but the caller has to say so, and the
    /// playlist names travel through to the confirmation either way.
    pub allow_playlist_members: bool,
}

/// Is `path` inside one of `roots`?
///
/// Compares normalised components, so `/music` does not match `/musicals` the
/// way a string prefix would. Both sides are expected to be absolute and
/// already canonicalised by the caller where that is possible; `..` components
/// are rejected outright rather than resolved, since resolving them without
/// touching the disk cannot be done correctly in the presence of symlinks.
pub fn is_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    if path.components().any(|c| c == Component::ParentDir) {
        return false;
    }
    roots
        .iter()
        .any(|root| !root.as_os_str().is_empty() && path.starts_with(root))
}

/// Decide what may be deleted.
///
/// `facts` answers filesystem questions; see [`facts_for`] for the real one.
/// Candidates are evaluated independently and the order of the input is
/// preserved in both output lists, so the UI can show them next to the rows the
/// user selected.
pub fn plan(
    candidates: &[DeleteCandidate],
    opts: &GuardOptions,
    facts: &dyn Fn(&Path) -> PathFacts,
) -> DeletePlan {
    let mut out = DeletePlan::default();

    // Two candidates naming the same file: the second is a duplicate request,
    // not a second file. Keep the first and drop the rest silently — reporting
    // it as a refusal would read as a problem when it is just deduplication.
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for c in candidates {
        let refuse = |reason: Refusal| Refused {
            track_id: c.track_id.clone(),
            path: c.path.clone(),
            message: reason.message(),
            reason,
        };

        if c.path.as_os_str().is_empty() {
            out.refused.push(refuse(Refusal::NoPath));
            continue;
        }
        if is_within_roots(&c.path, std::slice::from_ref(&opts.quarantine_root)) {
            out.refused.push(refuse(Refusal::AlreadyQuarantined));
            continue;
        }
        if !is_within_roots(&c.path, &opts.music_roots) {
            out.refused.push(refuse(Refusal::OutsideMusicRoots));
            continue;
        }
        if !c.shared_with.is_empty() {
            out.refused.push(refuse(Refusal::SharedWithTracks {
                track_ids: c.shared_with.clone(),
            }));
            continue;
        }
        if !opts.allow_playlist_members && !c.playlists.is_empty() {
            out.refused.push(refuse(Refusal::StillInPlaylists {
                playlists: c.playlists.clone(),
            }));
            continue;
        }

        let f = facts(&c.path);
        if !f.exists {
            out.refused.push(refuse(Refusal::Missing));
            continue;
        }
        if f.is_symlink {
            out.refused.push(refuse(Refusal::Symlink));
            continue;
        }
        if !f.is_regular_file {
            out.refused.push(refuse(Refusal::NotARegularFile));
            continue;
        }

        if !seen.insert(c.path.clone()) {
            continue;
        }
        out.total_bytes += f.len;
        out.deletable.push(Planned {
            track_id: c.track_id.clone(),
            source: c.path.clone(),
            bytes: f.len,
        });
    }

    out
}

/// The real filesystem oracle.
///
/// Uses `symlink_metadata` so a symlink is reported as a symlink rather than as
/// whatever it points at — the whole reason [`Refusal::Symlink`] exists.
pub fn facts_for(path: &Path) -> PathFacts {
    match std::fs::symlink_metadata(path) {
        Ok(md) => PathFacts {
            exists: true,
            is_regular_file: md.is_file(),
            is_symlink: md.file_type().is_symlink(),
            len: md.len(),
        },
        Err(_) => PathFacts::default(),
    }
}

// ── Manifest ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub track_id: String,
    /// Where the file was before it moved. Restoring puts it back here.
    pub original_path: PathBuf,
    /// Filename inside the batch's `files/` directory.
    pub stored_as: String,
    pub bytes: u64,
}

/// The record of one delete batch, written next to the files it describes.
///
/// Deliberately plain JSON with absolute paths: if this program is uninstalled
/// tomorrow, someone with a text editor can still put their music back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub batch_id: String,
    /// Unix seconds.
    pub created_at: i64,
    /// The library the tracks came from.
    pub library_path: String,
    /// What the user was doing — "Duplicate resolution", "Broken tracks".
    pub reason: String,
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn bytes(&self) -> u64 {
        self.entries.iter().map(|e| e.bytes).sum()
    }
}

/// A batch as the UI lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Batch {
    pub manifest: Manifest,
    pub total_bytes: u64,
    pub file_count: usize,
}

// ── Execution ────────────────────────────────────────────────────────────────

/// One file that could not be moved, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MoveFailure {
    pub track_id: String,
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeleteReceipt {
    pub manifest: Manifest,
    /// Files that stayed put. A partial batch is still a valid batch — the
    /// manifest describes exactly what moved, so restore remains correct.
    pub failed: Vec<MoveFailure>,
}

/// Batch id from a timestamp: `2026-08-06T14-22-01`.
///
/// Sorts lexicographically in time order, and is a legal directory name on
/// every platform we target (`:` is not).
pub fn batch_id_from(created_at: i64) -> String {
    // Civil-from-days, so we do not pull `chrono` into this crate for one
    // format call. Howard Hinnant's algorithm.
    let days = created_at.div_euclid(86_400);
    let secs = created_at.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}-{:02}-{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Move the planned files into a new batch under `quarantine_root`.
///
/// The manifest is written **after** the moves, so a crash mid-batch leaves
/// files in the quarantine with no manifest rather than a manifest promising
/// files that are not there — the recoverable direction of the two.
pub fn execute(
    plan: &DeletePlan,
    quarantine_root: &Path,
    library_path: &str,
    reason: &str,
    created_at: i64,
) -> io::Result<DeleteReceipt> {
    let batch_id = unique_batch_id(quarantine_root, created_at);
    let batch_dir = quarantine_root.join(&batch_id);
    let files_dir = batch_dir.join(FILES_SUBDIR);
    std::fs::create_dir_all(&files_dir)?;

    let mut entries = Vec::new();
    let mut failed = Vec::new();
    let mut taken: HashSet<String> = HashSet::new();

    for item in &plan.deletable {
        let stored_as = free_name(&item.source, &taken);
        let dest = files_dir.join(&stored_as);

        match relocate_file(&item.source, &dest) {
            Ok(()) => {
                taken.insert(stored_as.clone());
                entries.push(ManifestEntry {
                    track_id: item.track_id.clone(),
                    original_path: item.source.clone(),
                    stored_as,
                    bytes: item.bytes,
                });
            }
            Err(e) => failed.push(MoveFailure {
                track_id: item.track_id.clone(),
                path: item.source.clone(),
                error: e.to_string(),
            }),
        }
    }

    let manifest = Manifest {
        batch_id,
        created_at,
        library_path: library_path.to_string(),
        reason: reason.to_string(),
        entries,
    };

    if manifest.entries.is_empty() {
        // Nothing moved — do not leave an empty batch folder behind for the
        // user to wonder about.
        let _ = std::fs::remove_dir_all(&batch_dir);
    } else {
        write_manifest(&batch_dir, &manifest)?;
    }

    Ok(DeleteReceipt { manifest, failed })
}

/// Move a file, falling back to copy-then-remove across filesystems.
///
/// The copy is verified by length before the source is removed. A failed
/// verification leaves *both* copies rather than removing the original — the
/// caller can clean up a duplicate; nobody can clean up a lost file.
fn relocate_file(source: &Path, dest: &Path) -> io::Result<()> {
    match std::fs::rename(source, dest) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(e),
        Err(_) => {}
    }

    let copied = std::fs::copy(source, dest)?;
    let original = std::fs::metadata(source)?.len();
    if copied != original {
        let _ = std::fs::remove_file(dest);
        return Err(io::Error::other(format!(
            "copy verification failed: {copied} of {original} bytes; original left in place"
        )));
    }
    std::fs::remove_file(source)
}

/// A filename free within this batch, suffixing ` (2)`, ` (3)`, … on collision.
///
/// Two tracks can share a basename and both be real; overwriting one with the
/// other inside the quarantine would destroy the very file we are preserving.
fn free_name(source: &Path, taken: &HashSet<String>) -> String {
    let base = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "file".to_string());

    if !taken.contains(&base) {
        return base;
    }

    let (stem, ext) = match base.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (base.clone(), String::new()),
    };
    for n in 2.. {
        let candidate = format!("{stem} ({n}){ext}");
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!("the loop returns")
}

/// A batch id not already on disk, so two deletes in the same second do not
/// merge into one batch.
fn unique_batch_id(quarantine_root: &Path, created_at: i64) -> String {
    let base = batch_id_from(created_at);
    if !quarantine_root.join(&base).exists() {
        return base;
    }
    for n in 2.. {
        let candidate = format!("{base}-{n}");
        if !quarantine_root.join(&candidate).exists() {
            return candidate;
        }
    }
    unreachable!("the loop returns")
}

fn write_manifest(batch_dir: &Path, manifest: &Manifest) -> io::Result<()> {
    let json = serde_json::to_string_pretty(manifest).map_err(io::Error::other)?;
    let tmp = batch_dir.join("manifest.json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, batch_dir.join(MANIFEST_FILE))
}

// ── Listing, restoring, purging ──────────────────────────────────────────────

/// Every batch in the quarantine, newest first.
///
/// A directory without a readable manifest is skipped rather than failing the
/// listing — one corrupt batch must not hide the others.
pub fn list_batches(quarantine_root: &Path) -> io::Result<Vec<Batch>> {
    let mut out = Vec::new();
    let dir = match std::fs::read_dir(quarantine_root) {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };

    for entry in dir.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        if let Some(manifest) = read_manifest(&entry.path()) {
            out.push(Batch {
                total_bytes: manifest.bytes(),
                file_count: manifest.entries.len(),
                manifest,
            });
        }
    }
    out.sort_by(|a, b| {
        b.manifest
            .created_at
            .cmp(&a.manifest.created_at)
            .then_with(|| b.manifest.batch_id.cmp(&a.manifest.batch_id))
    });
    Ok(out)
}

fn read_manifest(batch_dir: &Path) -> Option<Manifest> {
    let raw = std::fs::read_to_string(batch_dir.join(MANIFEST_FILE)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// What happened to one entry during a restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RestoreOutcome {
    Restored {
        path: PathBuf,
    },
    /// Something is at the original path now. We never overwrite it.
    Occupied {
        path: PathBuf,
    },
    /// The quarantined file is gone — someone emptied the folder by hand.
    MissingFromQuarantine,
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RestoreResult {
    pub track_id: String,
    pub original_path: PathBuf,
    pub outcome: RestoreOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RestoreReport {
    pub batch_id: String,
    pub results: Vec<RestoreResult>,
    pub restored: usize,
    /// True when every entry came back and the batch folder was removed.
    pub batch_emptied: bool,
}

/// Put a batch back where it came from.
///
/// Entries that succeed are removed from the manifest, so a partial restore can
/// be retried after the user clears whatever was in the way. Files are only
/// ever written to a path that is currently free.
pub fn restore(quarantine_root: &Path, batch_id: &str) -> io::Result<RestoreReport> {
    let batch_dir = quarantine_root.join(batch_id);
    let mut manifest = read_manifest(&batch_dir)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no manifest for that batch"))?;
    let files_dir = batch_dir.join(FILES_SUBDIR);

    let mut results = Vec::new();
    let mut remaining = Vec::new();
    let mut restored = 0usize;

    for entry in manifest.entries.drain(..) {
        let stored = files_dir.join(&entry.stored_as);
        let outcome = if !stored.exists() {
            RestoreOutcome::MissingFromQuarantine
        } else if entry.original_path.exists() {
            RestoreOutcome::Occupied {
                path: entry.original_path.clone(),
            }
        } else {
            let parent_ok = entry
                .original_path
                .parent()
                .map(std::fs::create_dir_all)
                .unwrap_or(Ok(()));
            match parent_ok.and_then(|()| relocate_file(&stored, &entry.original_path)) {
                Ok(()) => RestoreOutcome::Restored {
                    path: entry.original_path.clone(),
                },
                Err(e) => RestoreOutcome::Failed {
                    error: e.to_string(),
                },
            }
        };

        match &outcome {
            RestoreOutcome::Restored { .. } | RestoreOutcome::MissingFromQuarantine => {
                restored += usize::from(matches!(outcome, RestoreOutcome::Restored { .. }))
            }
            _ => remaining.push(entry.clone()),
        }
        results.push(RestoreResult {
            track_id: entry.track_id,
            original_path: entry.original_path,
            outcome,
        });
    }

    let batch_emptied = remaining.is_empty();
    if batch_emptied {
        std::fs::remove_dir_all(&batch_dir)?;
    } else {
        manifest.entries = remaining;
        write_manifest(&batch_dir, &manifest)?;
    }

    Ok(RestoreReport {
        batch_id: batch_id.to_string(),
        results,
        restored,
        batch_emptied,
    })
}

/// The irreversible step: remove a batch and everything in it.
///
/// Separate from [`execute`] on purpose, and takes the batch id rather than a
/// "delete everything" switch, so emptying the quarantine is always something
/// the user asked for by name.
pub fn purge(quarantine_root: &Path, batch_id: &str) -> io::Result<u64> {
    // Refuse a batch id that escapes the quarantine root. The id arrives from
    // the UI, so it is not trusted input by construction — and `join` does not
    // normalise, so `starts_with` alone would happily accept `..`.
    let mut components = Path::new(batch_id).components();
    let single_normal =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if !single_normal {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a batch id",
        ));
    }
    let batch_dir = quarantine_root.join(batch_id);
    let bytes = read_manifest(&batch_dir).map(|m| m.bytes()).unwrap_or(0);
    std::fs::remove_dir_all(&batch_dir)?;
    Ok(bytes)
}

/// Track ids the quarantine currently holds, by library.
///
/// The library still has rows for deleted tracks — deleting the file and
/// removing the row are separate acts, and the row goes through the normal
/// staged pipeline. This is what lets the browser mark a track as "audio in the
/// deleted folder" rather than as a plain broken link.
pub fn quarantined_tracks(
    quarantine_root: &Path,
    library_path: &str,
) -> io::Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for batch in list_batches(quarantine_root)? {
        if batch.manifest.library_path != library_path {
            continue;
        }
        for entry in &batch.manifest.entries {
            out.entry(entry.track_id.clone())
                .or_insert_with(|| batch.manifest.batch_id.clone());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, path: &str) -> DeleteCandidate {
        DeleteCandidate {
            track_id: id.into(),
            path: PathBuf::from(path),
            playlists: vec![],
            shared_with: vec![],
        }
    }

    fn opts() -> GuardOptions {
        GuardOptions {
            music_roots: vec![PathBuf::from("/music")],
            quarantine_root: PathBuf::from("/data/deleted-audio"),
            allow_playlist_members: false,
        }
    }

    fn ordinary_file(len: u64) -> PathFacts {
        PathFacts {
            exists: true,
            is_regular_file: true,
            is_symlink: false,
            len,
        }
    }

    // ── Guards ───────────────────────────────────────────────────────────────

    #[test]
    fn an_ordinary_file_under_a_music_root_is_deletable() {
        let p = plan(&[candidate("t1", "/music/a.mp3")], &opts(), &|_| {
            ordinary_file(1024)
        });
        assert_eq!(p.deletable.len(), 1);
        assert!(p.refused.is_empty());
        assert_eq!(p.total_bytes, 1024);
    }

    #[test]
    fn no_music_roots_means_nothing_is_deletable() {
        // Fail closed. A caller that forgets to configure roots must delete
        // nothing, not everything.
        let guards = GuardOptions {
            music_roots: vec![],
            ..opts()
        };
        let p = plan(&[candidate("t1", "/music/a.mp3")], &guards, &|_| {
            ordinary_file(1)
        });
        assert!(p.deletable.is_empty());
        assert_eq!(p.refused[0].reason, Refusal::OutsideMusicRoots);
    }

    #[test]
    fn a_sibling_directory_with_the_same_prefix_is_not_inside_the_root() {
        // `/musicals` starts with `/music` as a string. Component comparison is
        // the whole reason `is_within_roots` exists.
        assert!(!is_within_roots(
            Path::new("/musicals/a.mp3"),
            &[PathBuf::from("/music")]
        ));
        assert!(is_within_roots(
            Path::new("/music/a.mp3"),
            &[PathBuf::from("/music")]
        ));
    }

    #[test]
    fn a_path_with_dot_dot_is_never_inside_a_root() {
        assert!(!is_within_roots(
            Path::new("/music/../etc/passwd"),
            &[PathBuf::from("/music")]
        ));
    }

    #[test]
    fn a_symlink_is_refused_even_when_it_points_somewhere_valid() {
        let p = plan(&[candidate("t1", "/music/a.mp3")], &opts(), &|_| {
            PathFacts {
                exists: true,
                is_regular_file: true,
                is_symlink: true,
                len: 10,
            }
        });
        assert_eq!(p.refused[0].reason, Refusal::Symlink);
        assert!(p.deletable.is_empty());
    }

    #[test]
    fn a_directory_is_refused() {
        let p = plan(&[candidate("t1", "/music/album")], &opts(), &|_| {
            PathFacts {
                exists: true,
                is_regular_file: false,
                is_symlink: false,
                len: 0,
            }
        });
        assert_eq!(p.refused[0].reason, Refusal::NotARegularFile);
    }

    #[test]
    fn a_missing_file_is_refused_rather_than_counted() {
        let p = plan(&[candidate("t1", "/music/gone.mp3")], &opts(), &|_| {
            PathFacts::default()
        });
        assert_eq!(p.refused[0].reason, Refusal::Missing);
        assert_eq!(p.total_bytes, 0);
    }

    #[test]
    fn a_file_two_tracks_point_at_is_refused_and_cannot_be_overridden() {
        let mut c = candidate("t1", "/music/a.mp3");
        c.shared_with = vec!["t9".into()];
        // Even with every override on, sharing still refuses — deleting the
        // file would break a track the user did not select.
        let guards = GuardOptions {
            allow_playlist_members: true,
            ..opts()
        };
        let p = plan(&[c], &guards, &|_| ordinary_file(1));
        assert!(p.deletable.is_empty());
        assert!(matches!(
            p.refused[0].reason,
            Refusal::SharedWithTracks { .. }
        ));
    }

    #[test]
    fn playlist_membership_refuses_by_default_and_clears_with_the_option() {
        let mut c = candidate("t1", "/music/a.mp3");
        c.playlists = vec!["Warmup".into(), "Peak".into()];

        let refused = plan(std::slice::from_ref(&c), &opts(), &|_| ordinary_file(1));
        assert!(refused.deletable.is_empty());
        assert!(refused.refused[0].message.contains("Warmup"));

        let guards = GuardOptions {
            allow_playlist_members: true,
            ..opts()
        };
        let allowed = plan(&[c], &guards, &|_| ordinary_file(1));
        assert_eq!(allowed.deletable.len(), 1);
    }

    #[test]
    fn a_file_already_in_the_quarantine_is_refused() {
        let p = plan(
            &[candidate("t1", "/data/deleted-audio/b/files/a.mp3")],
            &opts(),
            &|_| ordinary_file(1),
        );
        assert_eq!(p.refused[0].reason, Refusal::AlreadyQuarantined);
    }

    #[test]
    fn a_track_with_no_path_is_refused() {
        let p = plan(&[candidate("t1", "")], &opts(), &|_| ordinary_file(1));
        assert_eq!(p.refused[0].reason, Refusal::NoPath);
    }

    #[test]
    fn the_same_file_named_twice_is_deleted_once_and_counted_once() {
        let p = plan(
            &[
                candidate("t1", "/music/a.mp3"),
                candidate("t2", "/music/a.mp3"),
            ],
            &opts(),
            &|_| ordinary_file(500),
        );
        assert_eq!(p.deletable.len(), 1);
        assert_eq!(p.total_bytes, 500);
        assert!(p.refused.is_empty());
    }

    #[test]
    fn every_refusal_has_a_message() {
        for r in [
            Refusal::NoPath,
            Refusal::Missing,
            Refusal::NotARegularFile,
            Refusal::Symlink,
            Refusal::OutsideMusicRoots,
            Refusal::SharedWithTracks {
                track_ids: vec!["t2".into()],
            },
            Refusal::StillInPlaylists {
                playlists: vec!["Warmup".into()],
            },
            Refusal::AlreadyQuarantined,
        ] {
            assert!(!r.message().is_empty(), "{r:?} has no message");
        }
    }

    #[test]
    fn the_shared_message_agrees_with_itself_about_plurals() {
        let one = Refusal::SharedWithTracks {
            track_ids: vec!["t2".into()],
        };
        assert_eq!(
            one.message(),
            "1 other track in the library points at this same file."
        );
        let two = Refusal::SharedWithTracks {
            track_ids: vec!["t2".into(), "t3".into()],
        };
        assert!(two
            .message()
            .starts_with("2 other tracks in the library point at"));
    }

    // ── Batch ids ────────────────────────────────────────────────────────────

    #[test]
    fn batch_ids_format_as_a_sortable_timestamp() {
        assert_eq!(batch_id_from(0), "1970-01-01T00-00-00");
        assert_eq!(batch_id_from(1_754_490_121), "2025-08-06T14-22-01");
        // A leap day, since the civil-from-days arithmetic is hand-rolled.
        assert_eq!(batch_id_from(1_709_164_800), "2024-02-29T00-00-00");
    }

    #[test]
    fn batch_ids_sort_lexicographically_in_time_order() {
        let a = batch_id_from(1_754_490_121);
        let b = batch_id_from(1_754_490_122);
        assert!(a < b);
    }

    #[test]
    fn batch_ids_contain_no_character_windows_rejects() {
        let id = batch_id_from(1_754_490_121);
        assert!(
            !id.contains(':'),
            "{id} would be an illegal path on Windows"
        );
    }

    #[test]
    fn colliding_basenames_get_suffixed_inside_the_batch() {
        let mut taken = HashSet::new();
        assert_eq!(free_name(Path::new("/a/x.mp3"), &taken), "x.mp3");
        taken.insert("x.mp3".to_string());
        assert_eq!(free_name(Path::new("/b/x.mp3"), &taken), "x (2).mp3");
        taken.insert("x (2).mp3".to_string());
        assert_eq!(free_name(Path::new("/c/x.mp3"), &taken), "x (3).mp3");
    }

    #[test]
    fn an_extensionless_name_still_suffixes() {
        let taken: HashSet<String> = ["track".to_string()].into_iter().collect();
        assert_eq!(free_name(Path::new("/a/track"), &taken), "track (2)");
    }

    #[test]
    fn a_dotfile_is_not_treated_as_all_extension() {
        let taken: HashSet<String> = [".hidden".to_string()].into_iter().collect();
        assert_eq!(free_name(Path::new("/a/.hidden"), &taken), ".hidden (2)");
    }

    // ── Round trip on a real temp directory ──────────────────────────────────

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "decks-trash-{tag}-{}",
                std::process::id() as u64 + tag.len() as u64
            ));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(&base).unwrap();
            TempDir(base)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn round_trip_setup(tag: &str) -> (TempDir, PathBuf, PathBuf) {
        let tmp = TempDir::new(tag);
        let music = tmp.path().join("music");
        let quarantine = tmp.path().join(QUARANTINE_DIR);
        write(&music.join("a.mp3"), "aaaa");
        write(&music.join("sub/b.mp3"), "bbbbbb");
        (tmp, music, quarantine)
    }

    fn plan_for(music: &Path, quarantine: &Path, names: &[(&str, &str)]) -> DeletePlan {
        let guards = GuardOptions {
            music_roots: vec![music.to_path_buf()],
            quarantine_root: quarantine.to_path_buf(),
            allow_playlist_members: false,
        };
        let candidates: Vec<_> = names
            .iter()
            .map(|(id, rel)| DeleteCandidate {
                track_id: (*id).into(),
                path: music.join(rel),
                playlists: vec![],
                shared_with: vec![],
            })
            .collect();
        plan(&candidates, &guards, &facts_for)
    }

    #[test]
    fn deleting_moves_the_files_and_writes_a_manifest_beside_them() {
        let (tmp, music, quarantine) = round_trip_setup("exec");
        let p = plan_for(&music, &quarantine, &[("t1", "a.mp3"), ("t2", "sub/b.mp3")]);
        assert_eq!(p.deletable.len(), 2);

        let receipt = execute(
            &p,
            &quarantine,
            "/lib/master.db",
            "Duplicates",
            1_754_490_121,
        )
        .unwrap();
        assert!(receipt.failed.is_empty());
        assert_eq!(receipt.manifest.entries.len(), 2);
        assert_eq!(receipt.manifest.bytes(), 10);

        // Sources gone, quarantine holds them, manifest is on disk.
        assert!(!music.join("a.mp3").exists());
        assert!(!music.join("sub/b.mp3").exists());
        let batch = quarantine.join(&receipt.manifest.batch_id);
        assert!(batch.join(MANIFEST_FILE).exists());
        assert_eq!(
            std::fs::read_to_string(batch.join(FILES_SUBDIR).join("a.mp3")).unwrap(),
            "aaaa"
        );
        drop(tmp);
    }

    #[test]
    fn a_deleted_batch_restores_to_the_exact_paths_it_came_from() {
        let (tmp, music, quarantine) = round_trip_setup("restore");
        let p = plan_for(&music, &quarantine, &[("t1", "a.mp3"), ("t2", "sub/b.mp3")]);
        let receipt = execute(&p, &quarantine, "/lib/master.db", "Broken", 1_754_490_121).unwrap();

        let report = restore(&quarantine, &receipt.manifest.batch_id).unwrap();
        assert_eq!(report.restored, 2);
        assert!(report.batch_emptied);
        assert_eq!(
            std::fs::read_to_string(music.join("a.mp3")).unwrap(),
            "aaaa"
        );
        assert_eq!(
            std::fs::read_to_string(music.join("sub/b.mp3")).unwrap(),
            "bbbbbb"
        );
        // The emptied batch is gone, so it cannot be restored twice.
        assert!(!quarantine.join(&receipt.manifest.batch_id).exists());
        drop(tmp);
    }

    #[test]
    fn restore_never_overwrites_a_file_that_reappeared() {
        let (tmp, music, quarantine) = round_trip_setup("occupied");
        let p = plan_for(&music, &quarantine, &[("t1", "a.mp3")]);
        let receipt = execute(&p, &quarantine, "/lib/master.db", "Broken", 1_754_490_121).unwrap();

        // The user re-downloaded the track before restoring.
        write(&music.join("a.mp3"), "REPLACEMENT");

        let report = restore(&quarantine, &receipt.manifest.batch_id).unwrap();
        assert_eq!(report.restored, 0);
        assert!(!report.batch_emptied);
        assert!(matches!(
            report.results[0].outcome,
            RestoreOutcome::Occupied { .. }
        ));
        // Both survive: the replacement in place, the original still held.
        assert_eq!(
            std::fs::read_to_string(music.join("a.mp3")).unwrap(),
            "REPLACEMENT"
        );
        assert!(quarantine
            .join(&receipt.manifest.batch_id)
            .join(FILES_SUBDIR)
            .join("a.mp3")
            .exists());
        drop(tmp);
    }

    #[test]
    fn two_files_with_the_same_name_both_survive_quarantine_and_restore() {
        let tmp = TempDir::new("collide");
        let music = tmp.path().join("music");
        let quarantine = tmp.path().join(QUARANTINE_DIR);
        write(&music.join("one/x.mp3"), "first");
        write(&music.join("two/x.mp3"), "second");

        let p = plan_for(
            &music,
            &quarantine,
            &[("t1", "one/x.mp3"), ("t2", "two/x.mp3")],
        );
        let receipt = execute(
            &p,
            &quarantine,
            "/lib/master.db",
            "Duplicates",
            1_754_490_121,
        )
        .unwrap();
        assert_eq!(receipt.manifest.entries.len(), 2);
        assert_ne!(
            receipt.manifest.entries[0].stored_as,
            receipt.manifest.entries[1].stored_as
        );

        restore(&quarantine, &receipt.manifest.batch_id).unwrap();
        assert_eq!(
            std::fs::read_to_string(music.join("one/x.mp3")).unwrap(),
            "first"
        );
        assert_eq!(
            std::fs::read_to_string(music.join("two/x.mp3")).unwrap(),
            "second"
        );
        drop(tmp);
    }

    #[test]
    fn listing_reports_batches_newest_first_and_purging_removes_one() {
        let (tmp, music, quarantine) = round_trip_setup("list");
        let older = execute(
            &plan_for(&music, &quarantine, &[("t1", "a.mp3")]),
            &quarantine,
            "/lib/master.db",
            "Broken",
            1_754_490_121,
        )
        .unwrap();
        let newer = execute(
            &plan_for(&music, &quarantine, &[("t2", "sub/b.mp3")]),
            &quarantine,
            "/lib/master.db",
            "Duplicates",
            1_754_490_999,
        )
        .unwrap();

        let batches = list_batches(&quarantine).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].manifest.batch_id, newer.manifest.batch_id);
        assert_eq!(batches[0].total_bytes, 6);
        assert_eq!(batches[1].manifest.reason, "Broken");

        let freed = purge(&quarantine, &older.manifest.batch_id).unwrap();
        assert_eq!(freed, 4);
        assert_eq!(list_batches(&quarantine).unwrap().len(), 1);
        drop(tmp);
    }

    #[test]
    fn purge_refuses_a_batch_id_that_escapes_the_quarantine() {
        let tmp = TempDir::new("escape");
        let quarantine = tmp.path().join(QUARANTINE_DIR);
        std::fs::create_dir_all(&quarantine).unwrap();
        let victim = tmp.path().join("music");
        write(&victim.join("a.mp3"), "precious");

        for id in ["../music", "..", "a/b", ""] {
            let err = purge(&quarantine, id).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "accepted {id:?}");
        }
        assert!(victim.join("a.mp3").exists());
        drop(tmp);
    }

    #[test]
    fn listing_an_absent_quarantine_is_empty_not_an_error() {
        let tmp = TempDir::new("absent");
        assert!(list_batches(&tmp.path().join("nope")).unwrap().is_empty());
        drop(tmp);
    }

    #[test]
    fn two_deletes_in_the_same_second_are_two_batches() {
        let (tmp, music, quarantine) = round_trip_setup("same-second");
        let a = execute(
            &plan_for(&music, &quarantine, &[("t1", "a.mp3")]),
            &quarantine,
            "/lib/master.db",
            "Broken",
            1_754_490_121,
        )
        .unwrap();
        let b = execute(
            &plan_for(&music, &quarantine, &[("t2", "sub/b.mp3")]),
            &quarantine,
            "/lib/master.db",
            "Broken",
            1_754_490_121,
        )
        .unwrap();
        assert_ne!(a.manifest.batch_id, b.manifest.batch_id);
        assert_eq!(list_batches(&quarantine).unwrap().len(), 2);
        drop(tmp);
    }

    #[test]
    fn an_empty_plan_leaves_no_batch_folder_behind() {
        let tmp = TempDir::new("empty");
        let quarantine = tmp.path().join(QUARANTINE_DIR);
        let receipt = execute(
            &DeletePlan::default(),
            &quarantine,
            "/lib/master.db",
            "Broken",
            1_754_490_121,
        )
        .unwrap();
        assert!(receipt.manifest.entries.is_empty());
        assert!(list_batches(&quarantine).unwrap().is_empty());
        drop(tmp);
    }

    #[test]
    fn quarantined_tracks_are_reported_per_library() {
        let (tmp, music, quarantine) = round_trip_setup("per-lib");
        execute(
            &plan_for(&music, &quarantine, &[("t1", "a.mp3")]),
            &quarantine,
            "/lib/one.db",
            "Broken",
            1_754_490_121,
        )
        .unwrap();
        execute(
            &plan_for(&music, &quarantine, &[("t2", "sub/b.mp3")]),
            &quarantine,
            "/lib/two.db",
            "Broken",
            1_754_490_999,
        )
        .unwrap();

        let one = quarantined_tracks(&quarantine, "/lib/one.db").unwrap();
        assert_eq!(one.len(), 1);
        assert!(one.contains_key("t1"));
        assert!(quarantined_tracks(&quarantine, "/lib/two.db")
            .unwrap()
            .contains_key("t2"));
        drop(tmp);
    }
}
