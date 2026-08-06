//! File organisation — the `%field%` pattern language, subfolder patterns, and
//! move/rename planning.
//!
//! Three layers, each pure:
//!
//! - [`pattern`] — parse and render `%field%` templates with `{}` optional
//!   segments, plus filename sanitisation.
//! - [`subfolder`] — resolve up to three nested folder levels, including the
//!   computed patterns (bitrate bucket, first tag, date buckets).
//! - [`plan`] — combine the two into concrete destination paths, resolving
//!   collisions.
//!
//! Plus two neighbours that share the crate because they are the same concern
//! from other angles: [`unused`], the Find Unused Files sweep (which files on
//! disk the library does *not* account for), and [`mappings`], per-computer
//! path-prefix rewriting (where this machine keeps what the library refers to).
//!
//! Nothing here *writes* to the filesystem — [`unused::scan`] reads a folder
//! tree and that is the extent of it. Planning is separated from execution so a
//! bulk move over someone's music library can be shown in full and reviewed
//! before anything happens to it.
//!
//! See `docs/lexicon/06-files.md` for the behavioural spec.

pub mod mappings;
pub mod pattern;
pub mod plan;
pub mod subfolder;
pub mod unused;

pub use mappings::{PathMapping, PathMappings};
pub use pattern::{sanitize_component, Pattern, PatternError};
pub use plan::{plan_batch, MovePlan, OrganizeSpec, PlanOutcome, PlanRequest};
pub use subfolder::{
    RunDate, SubfolderError, SubfolderPattern, SubfolderSpec, TrackFacts, MAX_LEVELS,
};
pub use unused::{
    is_skipped_directory, is_unused, scan, ExtensionFilter, ExtensionMode, KnownPaths, UnusedFile,
    UnusedScan, SKIPPED_DIRECTORIES,
};
