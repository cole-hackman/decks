//! Rules-driven dynamic playlists — Lexicon calls them **smartlists**.
//!
//! A smartlist stores rules, never a track list; membership is always derived.
//! See `docs/lexicon/03-smartlists.md` for the behavioural spec and ADR-0013
//! for why the rule model is a two-level structure rather than a boolean tree.
//!
//! ```
//! use smartlists::{evaluate, Clause, Combinator, EvalContext, Field, Operator, Rule, Smartlist, Value};
//!
//! let list = Smartlist {
//!     id: "s1".into(),
//!     name: "Peak time house".into(),
//!     parent_folder_id: None,
//!     combinator: Combinator::All,
//!     clauses: vec![
//!         Clause::single(Rule::new(Field::Genre, Operator::Equals, Value::Text("House".into()))),
//!         Clause::single(Rule::new(Field::Bpm, Operator::Between, Value::Range(124.0, 130.0))),
//!     ],
//!     created_at: 0,
//!     updated_at: 0,
//! };
//! let ids = evaluate(&list, &[], &EvalContext::default());
//! assert!(ids.is_empty());
//! ```

pub mod eval;
pub mod generator;
pub mod key;
pub mod model;
pub mod sync;
pub mod throttle;

pub use eval::{evaluate, matches, EvalContext};
pub use generator::{generate, only_missing, GeneratedSmartlist, GeneratorSpec, LEXICON_FOLDER};
pub use key::canonical_key;
pub use model::{Clause, Combinator, Field, FieldKind, Operator, Rule, Smartlist, Value};
pub use sync::{
    is_excluded_by_name, is_exclusion_tag, materialize_changes, rekordbox_compatibility,
    Compatibility, EXCLUDED_FROM_SYNC,
};
pub use throttle::{RecomputeCache, RECOMPUTE_INTERVAL_SECS};
