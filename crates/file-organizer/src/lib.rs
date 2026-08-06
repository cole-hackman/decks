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
//! Nothing in this crate touches the filesystem. Planning is separated from
//! execution so a bulk move over someone's music library can be shown in full
//! and reviewed before anything happens to it.
//!
//! See `docs/lexicon/06-files.md` for the behavioural spec.

pub mod pattern;
pub mod plan;
pub mod subfolder;

pub use pattern::{sanitize_component, Pattern, PatternError};
pub use plan::{plan_batch, MovePlan, OrganizeSpec, PlanOutcome, PlanRequest};
pub use subfolder::{
    RunDate, SubfolderError, SubfolderPattern, SubfolderSpec, TrackFacts, MAX_LEVELS,
};
