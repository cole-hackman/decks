//! The Cue Point Generator's template engine.
//!
//! Splits into two halves deliberately:
//!
//! - **Anchors** (`anchor`) — the structural landmarks a template hangs off.
//!   Today they come from the user's own cues via *custom cue anchors*;
//!   detection lands later and produces the same `ResolvedAnchor` values.
//! - **Templates** (`template`) — declarative placement relative to anchors,
//!   plus slot assignment, overflow trimming and the Rekordbox duplicate
//!   memory-cue guard.
//!
//! Building anchors-from-existing-cues first is what the roadmap calls for: it
//! is pure matching with no analysis behind it, it delivers the whole template
//! system standalone, and human-placed cues become the ground truth we evaluate
//! detection against.
//!
//! See `docs/lexicon/05-cues-player.md` for the behavioural spec.

pub mod anchor;
pub mod template;

pub use anchor::{
    resolve_custom_anchors, Anchor, Confidence, CueRef, CustomAnchorRule, ResolvedAnchor,
};
pub use template::{
    apply_template, CueTemplate, GeneratedCue, GenerationResult, SkippedCue, StartCueBehavior,
    TemplateEntry,
};
