pub mod rewrite;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use strsim::levenshtein;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocateCandidate {
    pub track_id: String,
    pub original_path: String,
    pub matches: Vec<RelocateMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocateMatch {
    pub path: String,
    pub score: f32,
    pub reasons: Vec<String>,
}

pub struct TrackInfo {
    pub id: String,
    pub original_path: String,
    pub duration_secs: Option<i64>,
    pub title: String,
    pub artist: Option<String>,
}

pub struct Relocator {
    /// Maps lowercase filename to a list of full paths found in the search roots.
    filename_index: HashMap<String, Vec<PathBuf>>,
    /// All indexed files to support fuzzy searching if needed.
    all_files: Vec<PathBuf>,
}

impl Relocator {
    /// Build an index from a list of root directories.
    pub fn new(roots: &[impl AsRef<Path>]) -> Result<Self> {
        let mut filename_index: HashMap<String, Vec<PathBuf>> = HashMap::new();
        let mut all_files = Vec::new();

        for root in roots {
            let root = root.as_ref();
            if !root.exists() || !root.is_dir() {
                continue;
            }

            for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path().to_path_buf();

                // Simple filter for common audio extensions
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if matches!(
                        ext_lower.as_str(),
                        "mp3" | "wav" | "aif" | "aiff" | "flac" | "m4a" | "aac"
                    ) {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let name_lower = name.to_lowercase();
                            filename_index
                                .entry(name_lower)
                                .or_default()
                                .push(path.clone());
                            all_files.push(path);
                        }
                    }
                }
            }
        }

        Ok(Self {
            filename_index,
            all_files,
        })
    }

    /// Scan a single missing track to find candidates.
    pub fn scan_track(&self, track: &TrackInfo) -> RelocateCandidate {
        let mut matches = Vec::new();
        let original_path = Path::new(&track.original_path);
        let orig_file_name = original_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        let orig_parent_name = original_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        let orig_size = fs::metadata(original_path).map(|m| m.len()).ok();

        // 1. Exact filename match
        if let Some(paths) = self.filename_index.get(&orig_file_name) {
            for path in paths {
                let mut score = 0.0;
                let mut reasons = Vec::new();

                reasons.push("filename".to_string());
                score += 0.5;

                // If size matches
                let new_size = fs::metadata(path).map(|m| m.len()).ok();
                if let (Some(os), Some(ns)) = (orig_size, new_size) {
                    if os == ns {
                        reasons.push("size".to_string());
                        score += 0.4;
                    }
                }

                // If duration matches (could check audio file duration here using symphonia, but expensive.
                // We'll rely on size for now, which is a strong proxy for identical files).

                // If it's the only exact filename match, boost the score
                if paths.len() == 1 {
                    reasons.push("unique".to_string());
                    score += 0.1;
                }

                matches.push(RelocateMatch {
                    path: path.to_string_lossy().to_string(),
                    score,
                    reasons,
                });
            }
        }

        // 2. Fuzzy filename match within the same parent dir name
        if matches.is_empty() && !orig_parent_name.is_empty() {
            for path in &self.all_files {
                let new_file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let new_parent_name = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                if orig_parent_name == new_parent_name {
                    let dist = levenshtein(&orig_file_name, &new_file_name);
                    if dist <= 3 && dist > 0 {
                        let mut reasons =
                            vec!["fuzzy_filename".to_string(), "parent_dir".to_string()];
                        let mut score = 0.5;

                        let new_size = fs::metadata(path).map(|m| m.len()).ok();
                        if let (Some(os), Some(ns)) = (orig_size, new_size) {
                            if os == ns {
                                reasons.push("size".to_string());
                                score += 0.3;
                            }
                        }

                        matches.push(RelocateMatch {
                            path: path.to_string_lossy().to_string(),
                            score,
                            reasons,
                        });
                    }
                }
            }
        }

        // Sort by score descending
        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Keep top 3
        matches.truncate(3);

        RelocateCandidate {
            track_id: track.id.clone(),
            original_path: track.original_path.clone(),
            matches,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_file(root: &Path, rel: &str, contents: &[u8]) -> PathBuf {
        // Join one component at a time so the result uses the platform's native
        // separator. `Path::join("a/b")` keeps the embedded '/' verbatim, which on
        // Windows yields "root\a/b" — that never compares equal to the "root\a\b"
        // that walkdir produces, so path assertions here would fail on Windows only.
        let full = rel
            .split('/')
            .fold(root.to_path_buf(), |acc, part| acc.join(part));
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = File::create(&full).unwrap();
        f.write_all(contents).unwrap();
        full
    }

    fn track(id: &str, path: &Path) -> TrackInfo {
        TrackInfo {
            id: id.to_string(),
            original_path: path.to_string_lossy().to_string(),
            duration_secs: None,
            title: "t".to_string(),
            artist: None,
        }
    }

    #[test]
    fn indexes_only_audio_extensions() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "music/a.mp3", b"x");
        make_file(dir.path(), "music/b.flac", b"x");
        make_file(dir.path(), "music/c.txt", b"x");
        make_file(dir.path(), "music/d.jpg", b"x");
        let r = Relocator::new(&[dir.path()]).unwrap();
        assert_eq!(r.all_files.len(), 2);
        assert!(r.filename_index.contains_key("a.mp3"));
        assert!(r.filename_index.contains_key("b.flac"));
        assert!(!r.filename_index.contains_key("c.txt"));
    }

    #[test]
    fn exact_filename_match_finds_relocation() {
        let dir = TempDir::new().unwrap();
        // Original missing path (under /Old/...) vs. relocated file in dir
        let relocated = make_file(dir.path(), "NewMusic/song.mp3", b"hello");
        let r = Relocator::new(&[dir.path()]).unwrap();

        let missing_path = PathBuf::from("/Some/Old/Path/song.mp3");
        let t = track("t1", &missing_path);
        let cand = r.scan_track(&t);

        assert_eq!(cand.track_id, "t1");
        assert_eq!(cand.matches.len(), 1);
        let m = &cand.matches[0];
        assert_eq!(m.path, relocated.to_string_lossy().to_string());
        assert!(m.reasons.contains(&"filename".to_string()));
        assert!(m.reasons.contains(&"unique".to_string()));
        // Original file does not exist, so size cannot match.
        assert!(!m.reasons.contains(&"size".to_string()));
    }

    #[test]
    fn size_match_boosts_score_when_original_exists() {
        let dir = TempDir::new().unwrap();
        let original_dir = TempDir::new().unwrap();
        let original = make_file(original_dir.path(), "song.mp3", b"hello-world");
        // Two candidates: one matches by name only, one matches by name + size.
        make_file(dir.path(), "wrong/song.mp3", b"different bytes here");
        let right = make_file(dir.path(), "right/song.mp3", b"hello-world");
        let r = Relocator::new(&[dir.path()]).unwrap();

        let t = track("t1", &original);
        let cand = r.scan_track(&t);

        assert_eq!(cand.matches.len(), 2);
        // First (highest score) should be the size-matching candidate.
        assert_eq!(cand.matches[0].path, right.to_string_lossy().to_string());
        assert!(cand.matches[0].reasons.contains(&"size".to_string()));
        assert!(cand.matches[0].score > cand.matches[1].score);
        // The "unique" bonus should not apply when there are multiple matches.
        assert!(!cand.matches[0].reasons.contains(&"unique".to_string()));
    }

    #[test]
    fn fuzzy_match_only_within_same_parent_dir() {
        let dir = TempDir::new().unwrap();
        // Same parent dir name "Album1", slightly different filename (1 char diff).
        make_file(dir.path(), "Album1/track_01.mp3", b"x");
        // Different parent name, similarly-fuzzy filename — should NOT match (parent name differs).
        make_file(dir.path(), "Other/track_99.mp3", b"x");
        let r = Relocator::new(&[dir.path()]).unwrap();

        // Missing original: parent "Album1", filename "track_07.mp3" — 1 char different
        // from "track_01.mp3" under the same parent name, and 1 char from "track_99.mp3"
        // under a different parent. Only the same-parent fuzzy match should be reported.
        let missing = PathBuf::from("/Old/Album1/track_07.mp3");
        let t = track("t1", &missing);
        let cand = r.scan_track(&t);

        assert_eq!(cand.matches.len(), 1);
        assert!(cand.matches[0].path.contains("Album1"));
        assert!(cand.matches[0]
            .reasons
            .contains(&"fuzzy_filename".to_string()));
        assert!(cand.matches[0].reasons.contains(&"parent_dir".to_string()));
    }

    #[test]
    fn fuzzy_match_skipped_when_exact_match_exists() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "Album1/track.mp3", b"x");
        make_file(dir.path(), "Album1/trakc.mp3", b"x"); // 2-char fuzzy candidate
        let r = Relocator::new(&[dir.path()]).unwrap();

        let missing = PathBuf::from("/Old/Album1/track.mp3");
        let t = track("t1", &missing);
        let cand = r.scan_track(&t);

        // Exact match populated `matches`, so fuzzy pass is skipped — only the exact
        // candidate is returned.
        assert_eq!(cand.matches.len(), 1);
        assert!(cand.matches[0].reasons.contains(&"filename".to_string()));
        assert!(!cand.matches[0]
            .reasons
            .contains(&"fuzzy_filename".to_string()));
    }

    #[test]
    fn fuzzy_threshold_rejects_distant_filenames() {
        let dir = TempDir::new().unwrap();
        // Same parent, but 5 characters different (above threshold of 3).
        make_file(dir.path(), "Album1/completely_other.mp3", b"x");
        let r = Relocator::new(&[dir.path()]).unwrap();

        let missing = PathBuf::from("/Old/Album1/song.mp3");
        let t = track("t1", &missing);
        let cand = r.scan_track(&t);

        assert!(cand.matches.is_empty());
    }

    #[test]
    fn missing_roots_are_skipped_silently() {
        // A non-existent path mixed with a real one — should index the real one only.
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.mp3", b"x");
        let nonexistent = PathBuf::from("/definitely/does/not/exist/anywhere");
        let r = Relocator::new(&[dir.path().to_path_buf(), nonexistent]).unwrap();
        assert_eq!(r.all_files.len(), 1);
    }

    #[test]
    fn matches_capped_at_three() {
        let dir = TempDir::new().unwrap();
        for i in 0..5 {
            make_file(dir.path(), &format!("dir{i}/song.mp3"), b"x");
        }
        let r = Relocator::new(&[dir.path()]).unwrap();

        let missing = PathBuf::from("/Old/song.mp3");
        let t = track("t1", &missing);
        let cand = r.scan_track(&t);

        assert_eq!(cand.matches.len(), 3);
    }
}
