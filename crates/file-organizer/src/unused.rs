//! Find Unused Files — every file under a folder tree that the library does
//! not know about.
//!
//! The inverse of a missing-file scan, and aimed at reclaiming disk space. That
//! makes it the most dangerous read-only feature in the app: its output is a
//! list of deletion candidates, and one false positive is a lost track. So the
//! comparison is deliberately conservative, the DJ-app folders that hold
//! irreplaceable state are skipped by name, and the scan is separate from any
//! deletion.
//!
//! See `docs/lexicon/06-files.md §Find Unused Files`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Directory names never descended into.
///
/// The DJ-app folders hold cue points, grids and playlists that live nowhere
/// else — offering them as "unused" would be actively harmful. The rest are OS
/// and version-control directories that have no business in a music sweep.
pub const SKIPPED_DIRECTORIES: &[&str] = &[
    // DJ applications.
    "_Serato_",
    "Traktor",
    "PioneerDJ",
    "rekordbox",
    "iTunes",
    "Engine Library",
    "Lexicon",
    "decks",
    // OS and tooling.
    ".Trash",
    "$RECYCLE.BIN",
    "System Volume Information",
    ".git",
    ".svn",
];

/// Whether a directory should be skipped, by name.
///
/// Case-insensitive: macOS and Windows filesystems are, and a folder called
/// `pioneerdj` is the same folder.
pub fn is_skipped_directory(name: &str) -> bool {
    SKIPPED_DIRECTORIES
        .iter()
        .any(|s| s.eq_ignore_ascii_case(name))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionMode {
    /// Report only files whose extension is listed. `PNG,JPG` sweeps stray
    /// images out of a music folder.
    Include,
    /// Report everything except the listed extensions.
    Exclude,
}

/// An extension filter, as typed into the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionFilter {
    pub mode: ExtensionMode,
    /// Lower-case, without leading dots.
    pub extensions: Vec<String>,
}

impl ExtensionFilter {
    /// Parse a comma-separated list. `PNG, .jpg,JPEG` and `png,jpg,jpeg` are
    /// the same filter — users type both.
    pub fn parse(mode: ExtensionMode, input: &str) -> Self {
        let extensions = input
            .split(',')
            .map(|s| s.trim().trim_start_matches('.').to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        ExtensionFilter { mode, extensions }
    }

    /// An empty include list would report nothing and an empty exclude list
    /// would report everything. The first is a mistake; the second is the
    /// natural "no filter" state, so an empty list always means "no filter".
    pub fn allows(&self, path: &Path) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let listed = self.extensions.contains(&ext);
        match self.mode {
            ExtensionMode::Include => listed,
            ExtensionMode::Exclude => !listed,
        }
    }
}

impl Default for ExtensionFilter {
    fn default() -> Self {
        ExtensionFilter {
            mode: ExtensionMode::Exclude,
            extensions: Vec::new(),
        }
    }
}

/// The set of paths the library already knows about.
///
/// Keys are lower-cased with separators normalised, because Rekordbox and the
/// filesystem do not reliably agree on case or separator, and a case-only
/// mismatch here would offer a track in the library for deletion.
#[derive(Debug, Default)]
pub struct KnownPaths(HashSet<String>);

fn key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

impl KnownPaths {
    pub fn new<I, S>(paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<Path>,
    {
        KnownPaths(paths.into_iter().map(|p| key(p.as_ref())).collect())
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.0.contains(&key(path))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedFile {
    pub path: String,
    pub size_bytes: u64,
}

/// Everything one scan produced.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UnusedScan {
    pub files: Vec<UnusedFile>,
    pub total_bytes: u64,
    /// Directories skipped by name, so the UI can say what was not looked at
    /// rather than implying the sweep was exhaustive.
    pub skipped_directories: Vec<String>,
    /// Paths that could not be read (permissions, broken links).
    pub errors: Vec<String>,
}

/// Decide whether one visited file belongs in the report.
///
/// Split out from the walk so the interesting logic is testable without a
/// filesystem.
pub fn is_unused(path: &Path, known: &KnownPaths, filter: &ExtensionFilter) -> bool {
    !known.contains(path) && filter.allows(path)
}

/// Walk `roots` and report every file the library does not reference.
///
/// Refuses to scan when `known` is empty: an empty library would report the
/// user's entire music folder as deletable, which is the single worst thing
/// this function could do.
pub fn scan(
    roots: &[PathBuf],
    known: &KnownPaths,
    filter: &ExtensionFilter,
) -> Result<UnusedScan, String> {
    if known.is_empty() {
        return Err("refusing to scan with an empty library — everything would look unused".into());
    }

    let mut out = UnusedScan::default();
    for root in roots {
        let walker = walkdir::WalkDir::new(root).into_iter().filter_entry(|e| {
            if !e.file_type().is_dir() || e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !is_skipped_directory(&name)
        });

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    out.errors.push(e.to_string());
                    continue;
                }
            };
            if entry.file_type().is_dir() {
                continue;
            }
            let path = entry.path();
            if !is_unused(path, known, filter) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.total_bytes += size;
            out.files.push(UnusedFile {
                path: path.to_string_lossy().into_owned(),
                size_bytes: size,
            });
        }
    }

    out.skipped_directories = SKIPPED_DIRECTORIES.iter().map(|s| s.to_string()).collect();
    out.files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(mode: ExtensionMode, list: &str) -> ExtensionFilter {
        ExtensionFilter::parse(mode, list)
    }

    #[test]
    fn extension_lists_are_forgiving_about_how_they_are_typed() {
        let f = filter(ExtensionMode::Include, "PNG, .jpg ,JPEG,");
        assert_eq!(f.extensions, vec!["png", "jpg", "jpeg"]);
    }

    #[test]
    fn include_mode_reports_only_the_listed_extensions() {
        let f = filter(ExtensionMode::Include, "png,jpg");
        assert!(f.allows(Path::new("/m/cover.PNG")));
        assert!(!f.allows(Path::new("/m/track.mp3")));
    }

    #[test]
    fn exclude_mode_reports_everything_else() {
        let f = filter(ExtensionMode::Exclude, "mp3,flac");
        assert!(!f.allows(Path::new("/m/track.mp3")));
        assert!(f.allows(Path::new("/m/cover.png")));
    }

    #[test]
    fn an_empty_list_means_no_filter_in_either_mode() {
        // An empty include list would otherwise report nothing at all, which
        // looks like a broken scan rather than a mis-set filter.
        assert!(filter(ExtensionMode::Include, "").allows(Path::new("/m/a.mp3")));
        assert!(filter(ExtensionMode::Exclude, "  ").allows(Path::new("/m/a.mp3")));
    }

    #[test]
    fn extensionless_files_are_not_in_any_include_list() {
        let f = filter(ExtensionMode::Include, "png");
        assert!(!f.allows(Path::new("/m/README")));
        assert!(filter(ExtensionMode::Exclude, "png").allows(Path::new("/m/README")));
    }

    #[test]
    fn dj_application_folders_are_skipped_whatever_their_case() {
        assert!(is_skipped_directory("_Serato_"));
        assert!(is_skipped_directory("pioneerdj"));
        assert!(is_skipped_directory("Engine Library"));
        assert!(is_skipped_directory("Lexicon"));
        assert!(!is_skipped_directory("House"));
    }

    #[test]
    fn known_paths_match_regardless_of_case_or_separator() {
        // Rekordbox and the filesystem do not reliably agree on either, and a
        // mismatch would offer a library track for deletion.
        let known = KnownPaths::new(["/Music/House/Track.mp3"]);
        assert!(known.contains(Path::new("/music/house/track.mp3")));
        assert!(known.contains(Path::new("\\Music\\House\\Track.mp3")));
        assert!(!known.contains(Path::new("/Music/House/Other.mp3")));
    }

    #[test]
    fn a_file_in_the_library_is_never_unused() {
        let known = KnownPaths::new(["/m/a.mp3"]);
        let f = ExtensionFilter::default();
        assert!(!is_unused(Path::new("/m/a.mp3"), &known, &f));
        assert!(is_unused(Path::new("/m/b.mp3"), &known, &f));
    }

    #[test]
    fn the_filter_can_still_exclude_a_file_the_library_does_not_know() {
        let known = KnownPaths::new(["/m/a.mp3"]);
        let f = filter(ExtensionMode::Include, "png");
        assert!(!is_unused(Path::new("/m/b.mp3"), &known, &f));
        assert!(is_unused(Path::new("/m/cover.png"), &known, &f));
    }

    fn temp_tree(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("decks-unused-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("House")).unwrap();
        std::fs::create_dir_all(dir.join("PioneerDJ")).unwrap();
        std::fs::write(dir.join("House/known.mp3"), b"aaaa").unwrap();
        std::fs::write(dir.join("House/stray.png"), b"bb").unwrap();
        std::fs::write(dir.join("PioneerDJ/analysis.dat"), b"ccc").unwrap();
        dir
    }

    #[test]
    fn scanning_reports_the_stray_file_and_never_descends_into_a_dj_folder() {
        let dir = temp_tree("scan");
        let known = KnownPaths::new([dir.join("House/known.mp3")]);
        let got = scan(
            std::slice::from_ref(&dir),
            &known,
            &ExtensionFilter::default(),
        )
        .unwrap();

        let paths: Vec<_> = got.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths.len(), 1, "unexpected report: {paths:?}");
        assert!(paths[0].ends_with("stray.png"));
        assert_eq!(got.total_bytes, 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scanning_an_empty_library_refuses_rather_than_offering_everything() {
        let dir = temp_tree("empty");
        let err = scan(
            std::slice::from_ref(&dir),
            &KnownPaths::default(),
            &ExtensionFilter::default(),
        )
        .unwrap_err();
        assert!(err.contains("empty library"), "unexpected error: {err}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_report_says_which_directories_were_never_looked_at() {
        let dir = temp_tree("skipped");
        let known = KnownPaths::new([dir.join("House/known.mp3")]);
        let got = scan(
            std::slice::from_ref(&dir),
            &known,
            &ExtensionFilter::default(),
        )
        .unwrap();
        assert!(got.skipped_directories.iter().any(|d| d == "PioneerDJ"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_biggest_reclaimable_files_come_first() {
        let dir = std::env::temp_dir().join(format!("decks-unused-sort-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("small.png"), b"a").unwrap();
        std::fs::write(dir.join("big.png"), b"aaaaaaaa").unwrap();
        std::fs::write(dir.join("keep.mp3"), b"x").unwrap();

        let known = KnownPaths::new([dir.join("keep.mp3")]);
        let got = scan(
            std::slice::from_ref(&dir),
            &known,
            &ExtensionFilter::default(),
        )
        .unwrap();
        assert!(got.files[0].path.ends_with("big.png"));
        assert!(got.files[1].path.ends_with("small.png"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
