use anyhow::{Context, Result};
use rekordbox_db::RekordboxDb;
use rusqlite::Connection;
use std::env;
use std::path::{Path, PathBuf};

const RB_KEY: &str = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";
const SCHEMA: &str = include_str!("../src/sql/schema.sql");
const SEED: &str = include_str!("../src/sql/seed.sql");

fn main() -> Result<()> {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_output);
    let output = checked_output(&output, &allowed_dirs())?;
    eprintln!("WARNING: replaces {}", output.display());
    seed_library(&output)?;
    let db = RekordboxDb::open(&output)?;
    let track_count = db.tracks()?.len();
    anyhow::ensure!(track_count > 0, "fixture should contain tracks");
    println!("{}", output.display());
    Ok(())
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tiny-library")
}

fn default_output() -> PathBuf {
    fixture_dir().join("master.db")
}

/// Where a synthetic library may be written: its own fixture folder, or a
/// temporary directory. `fixtures/master.db` is where a real one may sit, so
/// the rest of `fixtures/` is not included.
fn allowed_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![fixture_dir(), env::temp_dir()];
    if cfg!(unix) {
        dirs.push(PathBuf::from("/tmp"));
    }
    dirs
}

/// Resolve `output` and refuse anything outside `allowed`.
///
/// The seeder replaces the file it is given, so this runs before anything is
/// created or removed.
fn checked_output(output: &Path, allowed: &[PathBuf]) -> Result<PathBuf> {
    let name = output
        .file_name()
        .context("output path must name a file")?
        .to_owned();
    let dir = resolve_dir(
        output
            .parent()
            .context("output path must include a parent directory")?,
    )?;
    let inside = allowed.iter().any(|a| {
        // The fixture folder does not exist in a fresh clone.
        resolve_dir(a).is_ok_and(|a| dir.starts_with(a))
    });
    anyhow::ensure!(
        inside,
        "refusing to replace {}: output must be inside fixtures/tiny-library or a temporary \
         directory",
        dir.join(&name).display()
    );
    Ok(dir.join(name))
}

/// The real location of `dir`, following links in whatever part of it exists.
fn resolve_dir(dir: &Path) -> Result<PathBuf> {
    let absolute = env::current_dir()?.join(dir);
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            return Ok(missing.iter().rev().fold(real, |p, part| p.join(part)));
        }
        let part = existing
            .file_name()
            .with_context(|| format!("cannot resolve {}", dir.display()))?;
        missing.push(part.to_owned());
        existing = existing
            .parent()
            .with_context(|| format!("cannot resolve {}", dir.display()))?;
    }
}

fn seed_library(output: &Path) -> Result<()> {
    let root = output
        .parent()
        .context("output path must include a parent directory")?;
    let music_dir = root.join("music");
    std::fs::create_dir_all(&music_dir)
        .with_context(|| format!("creating {}", music_dir.display()))?;
    for name in ["alpha.mp3", "beta.mp3", "gamma.mp3"] {
        let path = music_dir.join(name);
        if !path.exists() {
            std::fs::write(&path, b"synthetic fixture audio placeholder\n")
                .with_context(|| format!("writing {}", path.display()))?;
        }
    }

    if output.exists() {
        std::fs::remove_file(output).with_context(|| format!("removing {}", output.display()))?;
    }

    let conn = Connection::open(output).with_context(|| format!("opening {}", output.display()))?;
    conn.execute_batch(&format!(
        "PRAGMA key = '{RB_KEY}'; PRAGMA busy_timeout = 5000;"
    ))?;
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch(SEED)?;
    conn.execute(
        "UPDATE djmdContent SET FolderPath = ?1 WHERE ID = '1'",
        [music_dir.join("alpha.mp3").to_string_lossy().as_ref()],
    )?;
    conn.execute(
        "UPDATE djmdContent SET FolderPath = ?1 WHERE ID = '2'",
        [music_dir.join("beta.mp3").to_string_lossy().as_ref()],
    )?;
    conn.execute(
        "UPDATE djmdContent SET FolderPath = ?1 WHERE ID = '3'",
        [music_dir.join("gamma.mp3").to_string_lossy().as_ref()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_output_is_allowed() {
        checked_output(&default_output(), &allowed_dirs()).unwrap();
    }

    #[test]
    fn a_temporary_directory_is_allowed_and_seeds() {
        let dir = tempfile::tempdir().unwrap();
        let output = checked_output(&dir.path().join("new/master.db"), &allowed_dirs()).unwrap();
        seed_library(&output).unwrap();
        assert!(output.exists());
    }

    #[test]
    fn a_path_outside_the_allowed_directories_is_refused_untouched() {
        let allowed = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let library = elsewhere.path().join("master.db");
        std::fs::write(&library, b"existing library").unwrap();

        let err = checked_output(&library, &[allowed.path().to_path_buf()]).unwrap_err();
        assert!(err.to_string().contains("refusing to replace"));
        assert_eq!(std::fs::read(&library).unwrap(), b"existing library");
    }

    #[test]
    fn a_path_that_climbs_out_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let allowed = root.path().join("allowed");
        std::fs::create_dir(&allowed).unwrap();
        let climbing = allowed.join("../master.db");
        assert!(checked_output(&climbing, std::slice::from_ref(&allowed)).is_err());
        let missing = allowed.join("nope/../../master.db");
        assert!(checked_output(&missing, std::slice::from_ref(&allowed)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_directory_is_judged_by_where_it_leads() {
        let root = tempfile::tempdir().unwrap();
        let (allowed, outside) = (root.path().join("allowed"), root.path().join("outside"));
        std::fs::create_dir(&allowed).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, allowed.join("link")).unwrap();
        let through = allowed.join("link/master.db");
        assert!(checked_output(&through, std::slice::from_ref(&allowed)).is_err());
    }

    #[test]
    fn real_fixtures_beside_the_tiny_library_are_not_replaced() {
        let beside = fixture_dir().join("../master.db");
        assert!(checked_output(&beside, &[fixture_dir()]).is_err());
    }
}
