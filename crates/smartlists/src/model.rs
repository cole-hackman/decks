//! The smartlist rule model.
//!
//! Deliberately a **two-level** structure rather than a general boolean tree,
//! per ADR-0013:
//!
//! ```text
//! All  →  Clause AND Clause AND …   where each Clause is  rule OR rule OR …
//! Any  →  rule OR rule OR …          (every clause holds exactly one rule)
//! ```
//!
//! This is what Lexicon actually exposes — OR grouping is only offered when the
//! smartlist is in "All Rules" mode — and it is the same shape as the Custom
//! Tags page's selection semantics (OR within a category, AND across
//! categories). A recursive tree would be more code and a harder editor for
//! expressiveness nobody asks for.

use serde::{Deserialize, Serialize};

/// How the top-level clauses combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Combinator {
    /// Every clause must match (clauses are AND-ed; rules inside a clause are OR-ed).
    All,
    /// Any rule may match. Each clause holds exactly one rule.
    Any,
}

/// A field a rule can test.
///
/// Limited to what `rekordbox_db::Track` actually exposes plus the derived
/// predicates the app already computes. Lexicon fields `decks` does not model
/// yet (label, remixer, mix, colour, date added, danceability, popularity,
/// happiness) are intentionally absent — adding them means widening `Track` and
/// the core `SELECT`, which belongs with the epics that introduce them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    // Text
    Title,
    Artist,
    Album,
    Genre,
    Comment,
    FilePath,
    Label,
    Remixer,
    Mix,
    /// Rekordbox's own colour label, matched by name.
    Color,
    // Date (ISO-8601 strings, compared lexicographically)
    DateAdded,
    // Key (notation-aware equality)
    MusicalKey,
    // Numeric
    Bpm,
    Rating,
    Year,
    DurationSecs,
    BitRate,
    SampleRate,
    PlayCount,
    Energy,
    // Derived booleans
    HasCues,
    InAnyPlaylist,
    IsFileMissing,
    IsArchived,
    // Custom tags
    Tags,
}

impl Field {
    /// Which operator family applies to this field. Used by the editor to offer
    /// only the operators that make sense, and by validation.
    pub fn kind(self) -> FieldKind {
        match self {
            Field::Title
            | Field::Artist
            | Field::Album
            | Field::Genre
            | Field::Comment
            | Field::FilePath
            | Field::Label
            | Field::Remixer
            | Field::Mix
            | Field::Color => FieldKind::Text,
            Field::DateAdded => FieldKind::Date,
            Field::MusicalKey => FieldKind::Key,
            Field::Bpm
            | Field::Rating
            | Field::Year
            | Field::DurationSecs
            | Field::BitRate
            | Field::SampleRate
            | Field::PlayCount
            | Field::Energy => FieldKind::Number,
            Field::HasCues | Field::InAnyPlaylist | Field::IsFileMissing | Field::IsArchived => {
                FieldKind::Bool
            }
            Field::Tags => FieldKind::Tags,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Key,
    Number,
    Bool,
    Tags,
    /// An ISO-8601 timestamp held as a string.
    ///
    /// Compared lexicographically rather than parsed. For ISO-8601 that gives
    /// the same ordering as real date comparison, and it avoids inventing a
    /// precision the source column does not have — `djmdContent.DateCreated`
    /// is sometimes a date, sometimes a full timestamp, depending on how the
    /// library was migrated.
    Date,
}

/// Comparison operators. The set mirrors the vocabulary Lexicon documents for
/// track-browser search, which is the same vocabulary its smartlist rules use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    // Text
    Contains,
    NotContains,
    Equals,
    NotEquals,
    /// The field has no value. Lexicon spells this as the literal `None`
    /// keyword in a search box; here it is a first-class operator.
    IsNone,
    IsNotNone,
    // Numeric
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
    /// Inclusive range, Lexicon's `120-150` syntax.
    Between,
    // Boolean
    IsTrue,
    IsFalse,
    // Tags
    HasAll,
    HasAny,
    HasNone,
}

/// The right-hand side of a rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Value {
    Text(String),
    Number(f64),
    Range(f64, f64),
    /// Inclusive range over two ISO-8601 strings, for `DateAdded`. Separate
    /// from `Range` because a date is not a number and coercing it to one
    /// would lose the prefix semantics date matching depends on.
    TextRange(String, String),
    /// Custom tag IDs. Matching is by exact tag identity — Lexicon matches full
    /// labels only, never partial, as a deliberate performance decision.
    Tags(Vec<String>),
    /// Operators like `IsNone` / `IsTrue` take no operand.
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub field: Field,
    pub op: Operator,
    #[serde(default = "default_value_none")]
    pub value: Value,
}

fn default_value_none() -> Value {
    Value::None
}

impl Rule {
    pub fn new(field: Field, op: Operator, value: Value) -> Self {
        Self { field, op, value }
    }

    /// Reject combinations the evaluator cannot answer meaningfully, e.g. a
    /// `Between` on a text field or `HasAll` on anything but `Tags`. Returns the
    /// offending description so the editor can surface it.
    pub fn validate(&self) -> Result<(), String> {
        let kind = self.field.kind();
        let ok = match self.op {
            Operator::Contains | Operator::NotContains => kind == FieldKind::Text,
            Operator::Equals | Operator::NotEquals => {
                matches!(
                    kind,
                    FieldKind::Text | FieldKind::Number | FieldKind::Key | FieldKind::Date
                )
            }
            Operator::IsNone | Operator::IsNotNone => {
                matches!(
                    kind,
                    FieldKind::Text | FieldKind::Number | FieldKind::Key | FieldKind::Date
                )
            }
            Operator::GreaterThan
            | Operator::LessThan
            | Operator::GreaterOrEqual
            | Operator::LessOrEqual
            | Operator::Between => matches!(kind, FieldKind::Number | FieldKind::Date),
            Operator::IsTrue | Operator::IsFalse => kind == FieldKind::Bool,
            Operator::HasAll | Operator::HasAny | Operator::HasNone => kind == FieldKind::Tags,
        };
        if !ok {
            return Err(format!(
                "operator {:?} is not valid for field {:?}",
                self.op, self.field
            ));
        }
        Ok(())
    }
}

/// A group of rules OR-ed together. Only meaningful as a group when the
/// smartlist combinator is `All`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clause {
    pub rules: Vec<Rule>,
}

impl Clause {
    pub fn single(rule: Rule) -> Self {
        Self { rules: vec![rule] }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Smartlist {
    pub id: String,
    pub name: String,
    /// Where the smartlist sits in the playlist tree. `None` = root.
    #[serde(default)]
    pub parent_folder_id: Option<String>,
    pub combinator: Combinator,
    pub clauses: Vec<Clause>,
    /// Archived tracks are hidden from smartlists unless a rule explicitly asks
    /// for them — Lexicon's documented default.
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl Smartlist {
    /// True when any rule targets `IsArchived`, which is how a smartlist opts
    /// in to seeing archived tracks.
    pub fn mentions_archived(&self) -> bool {
        self.clauses
            .iter()
            .flat_map(|c| c.rules.iter())
            .any(|r| r.field == Field::IsArchived)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("smartlist name must not be empty".into());
        }
        for clause in &self.clauses {
            if clause.rules.is_empty() {
                return Err("a clause must contain at least one rule".into());
            }
            for rule in &clause.rules {
                rule.validate()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_kinds_are_assigned() {
        assert_eq!(Field::Title.kind(), FieldKind::Text);
        assert_eq!(Field::Bpm.kind(), FieldKind::Number);
        assert_eq!(Field::MusicalKey.kind(), FieldKind::Key);
        assert_eq!(Field::HasCues.kind(), FieldKind::Bool);
        assert_eq!(Field::Tags.kind(), FieldKind::Tags);
    }

    #[test]
    fn validate_rejects_operator_field_mismatch() {
        let r = Rule::new(Field::Title, Operator::Between, Value::Range(1.0, 2.0));
        assert!(r.validate().is_err());

        let r = Rule::new(Field::Bpm, Operator::Contains, Value::Text("x".into()));
        assert!(r.validate().is_err());

        let r = Rule::new(Field::Title, Operator::HasAll, Value::Tags(vec![]));
        assert!(r.validate().is_err());
    }

    #[test]
    fn validate_accepts_sensible_combinations() {
        assert!(
            Rule::new(Field::Title, Operator::Contains, Value::Text("a".into()))
                .validate()
                .is_ok()
        );
        assert!(
            Rule::new(Field::Bpm, Operator::Between, Value::Range(120.0, 130.0))
                .validate()
                .is_ok()
        );
        assert!(Rule::new(Field::HasCues, Operator::IsTrue, Value::None)
            .validate()
            .is_ok());
        assert!(Rule::new(
            Field::Tags,
            Operator::HasNone,
            Value::Tags(vec!["t1".into()])
        )
        .validate()
        .is_ok());
        assert!(Rule::new(
            Field::MusicalKey,
            Operator::Equals,
            Value::Text("8A".into())
        )
        .validate()
        .is_ok());
    }

    #[test]
    fn empty_name_is_invalid() {
        let s = Smartlist {
            id: "s1".into(),
            name: "  ".into(),
            parent_folder_id: None,
            combinator: Combinator::All,
            clauses: vec![],
            created_at: 0,
            updated_at: 0,
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn empty_clause_is_invalid() {
        let s = Smartlist {
            id: "s1".into(),
            name: "Test".into(),
            parent_folder_id: None,
            combinator: Combinator::All,
            clauses: vec![Clause { rules: vec![] }],
            created_at: 0,
            updated_at: 0,
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn mentions_archived_detects_opt_in() {
        let mut s = Smartlist {
            id: "s1".into(),
            name: "Test".into(),
            parent_folder_id: None,
            combinator: Combinator::All,
            clauses: vec![Clause::single(Rule::new(
                Field::Title,
                Operator::Contains,
                Value::Text("a".into()),
            ))],
            created_at: 0,
            updated_at: 0,
        };
        assert!(!s.mentions_archived());
        s.clauses.push(Clause::single(Rule::new(
            Field::IsArchived,
            Operator::IsTrue,
            Value::None,
        )));
        assert!(s.mentions_archived());
    }

    #[test]
    fn round_trips_through_json() {
        let s = Smartlist {
            id: "s1".into(),
            name: "Peak time".into(),
            parent_folder_id: Some("f1".into()),
            combinator: Combinator::All,
            clauses: vec![
                Clause {
                    rules: vec![
                        Rule::new(Field::Genre, Operator::Equals, Value::Text("House".into())),
                        Rule::new(Field::Genre, Operator::Equals, Value::Text("Techno".into())),
                    ],
                },
                Clause::single(Rule::new(
                    Field::Rating,
                    Operator::Equals,
                    Value::Number(3.0),
                )),
            ],
            created_at: 1,
            updated_at: 2,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Smartlist = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
