//! Turning patterns into concrete destination paths.
//!
//! Planning is deliberately pure: nothing here touches the filesystem. It takes
//! an existence oracle as an argument instead, so the whole of the interesting
//! behaviour — collisions, empty renders, no-op moves — is unit-testable, and
//! the caller can show the user the full plan before a single byte moves.
//!
//! Per `docs/lexicon/06-files.md`: **if no target folder is configured nothing
//! moves, but renaming still happens.**

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::pattern::{sanitize_component, Pattern};
use crate::subfolder::{RunDate, SubfolderSpec, TrackFacts};

/// How files should be laid out.
#[derive(Debug, Clone, Default)]
pub struct OrganizeSpec {
    /// Where files are moved to. `None` renames in place.
    pub target_folder: Option<PathBuf>,
    /// The filename pattern. `None` keeps the existing filename.
    pub filename: Option<Pattern>,
    /// Nested folder levels below the target folder.
    pub subfolders: SubfolderSpec,
}

/// What should happen to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanOutcome {
    /// Already where it belongs, under the name it should have.
    Unchanged,
    /// Move and/or rename to this path.
    Move(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovePlan {
    pub source: PathBuf,
    pub outcome: PlanOutcome,
}

impl MovePlan {
    pub fn destination(&self) -> Option<&Path> {
        match &self.outcome {
            PlanOutcome::Move(p) => Some(p.as_path()),
            PlanOutcome::Unchanged => None,
        }
    }
}

/// Append ` (2)`, ` (3)`, … until the path is free.
///
/// Suffixing rather than overwriting is not a preference: two tracks can
/// legitimately render to the same name (a remix and its original both titled
/// "Get Lucky"), and silently overwriting one with the other destroys audio.
fn deduplicate(
    candidate: PathBuf,
    taken: &HashSet<PathBuf>,
    exists: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    if !taken.contains(&candidate) && !exists(&candidate) {
        return candidate;
    }
    let parent = candidate
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let stem = candidate
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = candidate
        .extension()
        .map(|s| s.to_string_lossy().into_owned());

    for n in 2u32.. {
        let mut name = format!("{stem} ({n})");
        if let Some(ext) = &ext {
            name.push('.');
            name.push_str(ext);
        }
        let next = parent.join(name);
        if !taken.contains(&next) && !exists(&next) {
            return next;
        }
    }
    unreachable!("u32 range is exhausted before filenames are")
}

/// Plan where a single file should end up.
///
/// `taken` carries destinations already claimed earlier in the same batch, so
/// two tracks in one run cannot be planned onto the same path.
fn plan_one(
    spec: &OrganizeSpec,
    source: &Path,
    facts: &TrackFacts<'_>,
    now: RunDate,
    taken: &HashSet<PathBuf>,
    exists: &dyn Fn(&Path) -> bool,
) -> MovePlan {
    let extension = source.extension().map(|s| s.to_string_lossy().into_owned());
    let original_stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    // A render with nothing but punctuation left in it falls back to the
    // existing name. `%artist% - %title%` on an untagged track renders " - ",
    // and a file called "-.mp3" is worse than one that was simply not renamed.
    let stem = match &spec.filename {
        Some(pattern) => {
            let rendered = sanitize_component(&pattern.render(facts.fields));
            if rendered.chars().any(|c| c.is_alphanumeric()) {
                rendered
            } else {
                original_stem.clone()
            }
        }
        None => original_stem.clone(),
    };

    let mut name = stem;
    if let Some(ext) = &extension {
        name.push('.');
        name.push_str(ext);
    }

    // No target folder → rename in place. Subfolder levels are only meaningful
    // relative to a target folder, so they are ignored in that case.
    let base = match &spec.target_folder {
        Some(target) => {
            let mut base = target.clone();
            for level in spec.subfolders.resolve(facts, now) {
                base.push(level);
            }
            base
        }
        None => source.parent().map(Path::to_path_buf).unwrap_or_default(),
    };

    let candidate = base.join(&name);
    if candidate == source {
        return MovePlan {
            source: source.to_path_buf(),
            outcome: PlanOutcome::Unchanged,
        };
    }

    MovePlan {
        source: source.to_path_buf(),
        outcome: PlanOutcome::Move(deduplicate(candidate, taken, exists)),
    }
}

/// One file to plan for.
pub struct PlanRequest<'a> {
    pub source: &'a Path,
    pub facts: TrackFacts<'a>,
}

/// Plan a whole batch, resolving collisions both against the filesystem and
/// against destinations claimed earlier in the same batch.
pub fn plan_batch(
    spec: &OrganizeSpec,
    requests: &[PlanRequest<'_>],
    now: RunDate,
    exists: &dyn Fn(&Path) -> bool,
) -> Vec<MovePlan> {
    let mut taken: HashSet<PathBuf> = HashSet::new();
    let mut out = Vec::with_capacity(requests.len());
    for req in requests {
        let plan = plan_one(spec, req.source, &req.facts, now, &taken, exists);
        if let PlanOutcome::Move(dest) = &plan.outcome {
            taken.insert(dest.clone());
        }
        out.push(plan);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subfolder::SubfolderPattern;
    use std::collections::HashMap;

    const NOW: RunDate = RunDate {
        year: 2026,
        month: 8,
    };

    fn nothing_exists(_: &Path) -> bool {
        false
    }

    fn fields(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn facts<'a>(f: &'a HashMap<String, String>) -> TrackFacts<'a> {
        TrackFacts {
            fields: f,
            bitrate_kbps: None,
            tags: &[],
            year: None,
        }
    }

    fn spec(
        target: Option<&str>,
        filename: Option<&str>,
        levels: Vec<SubfolderPattern>,
    ) -> OrganizeSpec {
        OrganizeSpec {
            target_folder: target.map(PathBuf::from),
            filename: filename.map(|p| Pattern::parse(p).unwrap()),
            subfolders: SubfolderSpec::new(levels).unwrap(),
        }
    }

    fn field(name: &str) -> SubfolderPattern {
        SubfolderPattern::Field {
            name: name.to_string(),
        }
    }

    fn plan(spec: &OrganizeSpec, source: &str, f: &HashMap<String, String>) -> MovePlan {
        plan_batch(
            spec,
            &[PlanRequest {
                source: Path::new(source),
                facts: facts(f),
            }],
            NOW,
            &nothing_exists,
        )
        .remove(0)
    }

    #[test]
    fn moves_into_the_target_folder_with_subfolder_levels() {
        let f = fields(&[
            ("artist", "Daft Punk"),
            ("title", "Get Lucky"),
            ("genre", "House"),
        ]);
        let s = spec(
            Some("/Music"),
            Some("%artist% - %title%"),
            vec![field("genre")],
        );
        assert_eq!(
            plan(&s, "/Incoming/track.mp3", &f).destination().unwrap(),
            Path::new("/Music/House/Daft Punk - Get Lucky.mp3")
        );
    }

    #[test]
    fn renames_in_place_when_no_target_folder_is_configured() {
        let f = fields(&[("artist", "Daft Punk"), ("title", "Get Lucky")]);
        let s = spec(None, Some("%artist% - %title%"), vec![field("genre")]);
        assert_eq!(
            plan(&s, "/Incoming/track.mp3", &f).destination().unwrap(),
            Path::new("/Incoming/Daft Punk - Get Lucky.mp3")
        );
    }

    #[test]
    fn moves_without_renaming_when_no_filename_pattern_is_configured() {
        let f = fields(&[("genre", "House")]);
        let s = spec(Some("/Music"), None, vec![field("genre")]);
        assert_eq!(
            plan(&s, "/Incoming/original name.mp3", &f)
                .destination()
                .unwrap(),
            Path::new("/Music/House/original name.mp3")
        );
    }

    #[test]
    fn a_track_missing_the_subfolder_field_still_moves_to_the_target_folder() {
        let f = fields(&[("artist", "A"), ("title", "B")]);
        let s = spec(
            Some("/Music"),
            Some("%artist% - %title%"),
            vec![field("genre")],
        );
        assert_eq!(
            plan(&s, "/Incoming/track.mp3", &f).destination().unwrap(),
            Path::new("/Music/A - B.mp3")
        );
    }

    #[test]
    fn a_file_already_in_place_is_unchanged() {
        let f = fields(&[("artist", "A"), ("title", "B")]);
        let s = spec(Some("/Music"), Some("%artist% - %title%"), vec![]);
        assert_eq!(
            plan(&s, "/Music/A - B.mp3", &f).outcome,
            PlanOutcome::Unchanged
        );
    }

    #[test]
    fn the_extension_is_preserved_verbatim() {
        let f = fields(&[("artist", "A"), ("title", "B")]);
        let s = spec(Some("/Music"), Some("%artist% - %title%"), vec![]);
        assert_eq!(
            plan(&s, "/Incoming/track.FLAC", &f).destination().unwrap(),
            Path::new("/Music/A - B.FLAC")
        );
    }

    #[test]
    fn an_empty_render_falls_back_to_the_original_filename() {
        // Otherwise a track with no tags becomes ".mp3" — an invisible file.
        let f = fields(&[]);
        let s = spec(Some("/Music"), Some("%artist% - %title%"), vec![]);
        assert_eq!(
            plan(&s, "/Incoming/original.mp3", &f)
                .destination()
                .unwrap(),
            Path::new("/Music/original.mp3")
        );
    }

    #[test]
    fn illegal_characters_in_a_rendered_name_are_sanitised() {
        let f = fields(&[("artist", "AC/DC"), ("title", "Back: In Black")]);
        let s = spec(Some("/Music"), Some("%artist% - %title%"), vec![]);
        assert_eq!(
            plan(&s, "/Incoming/track.mp3", &f).destination().unwrap(),
            Path::new("/Music/AC-DC - Back- In Black.mp3")
        );
    }

    #[test]
    fn a_collision_with_an_existing_file_suffixes_rather_than_overwrites() {
        let f = fields(&[("artist", "A"), ("title", "B")]);
        let s = spec(Some("/Music"), Some("%artist% - %title%"), vec![]);
        let exists = |p: &Path| p == Path::new("/Music/A - B.mp3");
        let got = plan_batch(
            &s,
            &[PlanRequest {
                source: Path::new("/Incoming/track.mp3"),
                facts: facts(&f),
            }],
            NOW,
            &exists,
        );
        assert_eq!(
            got[0].destination().unwrap(),
            Path::new("/Music/A - B (2).mp3")
        );
    }

    #[test]
    fn two_tracks_rendering_to_the_same_name_do_not_overwrite_each_other() {
        let f = fields(&[("artist", "A"), ("title", "B")]);
        let s = spec(Some("/Music"), Some("%artist% - %title%"), vec![]);
        let got = plan_batch(
            &s,
            &[
                PlanRequest {
                    source: Path::new("/Incoming/one.mp3"),
                    facts: facts(&f),
                },
                PlanRequest {
                    source: Path::new("/Incoming/two.mp3"),
                    facts: facts(&f),
                },
                PlanRequest {
                    source: Path::new("/Incoming/three.mp3"),
                    facts: facts(&f),
                },
            ],
            NOW,
            &nothing_exists,
        );
        assert_eq!(got[0].destination().unwrap(), Path::new("/Music/A - B.mp3"));
        assert_eq!(
            got[1].destination().unwrap(),
            Path::new("/Music/A - B (2).mp3")
        );
        assert_eq!(
            got[2].destination().unwrap(),
            Path::new("/Music/A - B (3).mp3")
        );
    }

    #[test]
    fn the_files_own_path_does_not_count_as_a_collision() {
        // The file being planned already exists on disk; that must not push it
        // to "(2)" when the plan is a no-op.
        let f = fields(&[("artist", "A"), ("title", "B")]);
        let s = spec(Some("/Music"), Some("%artist% - %title%"), vec![]);
        let exists = |p: &Path| p == Path::new("/Music/A - B.mp3");
        let got = plan_batch(
            &s,
            &[PlanRequest {
                source: Path::new("/Music/A - B.mp3"),
                facts: facts(&f),
            }],
            NOW,
            &exists,
        );
        assert_eq!(got[0].outcome, PlanOutcome::Unchanged);
    }

    #[test]
    fn subfolder_levels_are_ignored_when_renaming_in_place() {
        let f = fields(&[("genre", "House"), ("artist", "A"), ("title", "B")]);
        let s = spec(None, Some("%artist% - %title%"), vec![field("genre")]);
        assert_eq!(
            plan(&s, "/Incoming/track.mp3", &f).destination().unwrap(),
            Path::new("/Incoming/A - B.mp3")
        );
    }
}
