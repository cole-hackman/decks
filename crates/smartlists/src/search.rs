//! The track-browser search box, parsed into the rule engine's vocabulary.
//!
//! Per `docs/lexicon/02-library.md §Search`. Lexicon's browser search accepts
//! the same operators its smartlists do — `None`, `>`, `<`, `>=`, `<=`, ranges
//! and `!` — plus a tag query language (`~` requires all, `!` negates).
//!
//! **Parsing to `Clause`s rather than reimplementing matching** is the whole
//! point of this module. The evaluator already knows what `bpm > 128` means,
//! what makes `4A` equal `Abm`, and how tags compare; a second implementation
//! in the search box is how the two drift. Nothing here evaluates.
//!
//! ## Syntax
//!
//! | Input | Meaning |
//! |---|---|
//! | `deadmau5` | any text field contains it |
//! | `artist:deadmau5` | that field contains it |
//! | `bpm>128` | numeric comparison |
//! | `bpm:120-130` | inclusive range |
//! | `key:8A` | notation-aware key equality |
//! | `genre:None` | the field is empty |
//! | `!genre:techno` | negated |
//! | `~tag1,tag2` | has *all* of these tags |
//! | `tag:tag1,tag2` | has *any* of these tags |
//!
//! Terms are separated by whitespace and **ANDed**, which is what a search box
//! is for: each word you add narrows the result.

use crate::model::{Clause, Combinator, Field, Operator, Rule, Smartlist, Value};

/// Field names the search box accepts, and what they map to.
///
/// Aliases are the names a DJ would actually type. `year` rather than
/// `release_year`, `time` as well as `duration`.
fn field_for(name: &str) -> Option<Field> {
    match name.to_ascii_lowercase().as_str() {
        "title" | "name" => Some(Field::Title),
        "artist" => Some(Field::Artist),
        "album" => Some(Field::Album),
        "genre" => Some(Field::Genre),
        "comment" | "comments" => Some(Field::Comment),
        "path" | "file" | "location" => Some(Field::FilePath),
        "key" => Some(Field::MusicalKey),
        "bpm" | "tempo" => Some(Field::Bpm),
        "rating" | "stars" => Some(Field::Rating),
        "year" => Some(Field::Year),
        "time" | "duration" | "length" => Some(Field::DurationSecs),
        "bitrate" => Some(Field::BitRate),
        "samplerate" => Some(Field::SampleRate),
        "plays" | "playcount" => Some(Field::PlayCount),
        "energy" => Some(Field::Energy),
        "tag" | "tags" => Some(Field::Tags),
        _ => None,
    }
}

/// The text fields a bare word searches.
const FREE_TEXT_FIELDS: &[Field] = &[
    Field::Title,
    Field::Artist,
    Field::Album,
    Field::Genre,
    Field::Comment,
    Field::FilePath,
];

/// Split on whitespace, but keep `"quoted phrases"` together.
fn tokenise(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in query.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Whether a query uses any syntax at all.
///
/// The browser can keep its instant client-side substring match for a plain
/// query and only reach for the engine when there is something to parse — so
/// typing a band's name never waits on a round-trip.
pub fn has_operators(query: &str) -> bool {
    tokenise(query).iter().any(|t| {
        t.starts_with('!')
            || t.starts_with('~')
            || t.contains(':')
            || t.contains('>')
            || t.contains('<')
    })
}

fn parse_number(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

/// A range, `120-130`. Not applied to text — a title can contain a dash.
fn parse_range(s: &str) -> Option<(f64, f64)> {
    let (lo, hi) = s.split_once('-')?;
    let (lo, hi) = (parse_number(lo)?, parse_number(hi)?);
    // Accept them either way round rather than returning nothing: someone who
    // typed `130-120` meant the same range.
    Some(if lo <= hi { (lo, hi) } else { (hi, lo) })
}

/// Turn one token into a clause. `None` when the token means nothing.
fn parse_term(token: &str) -> Option<Clause> {
    let (negated, token) = match token.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    if token.is_empty() {
        return None;
    }

    // `~a,b` — has all of these tags. The tilde has no field prefix because
    // it is the tag language's own syntax, per the spec.
    if let Some(rest) = token.strip_prefix('~') {
        let tags = split_tags(rest);
        if tags.is_empty() {
            return None;
        }
        return Some(clause(
            Field::Tags,
            if negated {
                Operator::HasNone
            } else {
                Operator::HasAll
            },
            Value::Tags(tags),
        ));
    }

    // `field>value`, `field<value`, `field>=value`, `field<=value`
    for (marker, op) in [
        (">=", Operator::GreaterOrEqual),
        ("<=", Operator::LessOrEqual),
        (">", Operator::GreaterThan),
        ("<", Operator::LessThan),
    ] {
        if let Some((name, raw)) = token.split_once(marker) {
            let field = field_for(name)?;
            let value = parse_number(raw)?;
            // A negated comparison is the opposite comparison, which keeps the
            // rule model free of a "not" wrapper it does not have.
            let op = if negated { flip(op) } else { op };
            return Some(clause(field, op, Value::Number(value)));
        }
    }

    // `field:value`
    if let Some((name, raw)) = token.split_once(':') {
        let field = field_for(name)?;
        if raw.is_empty() {
            return None;
        }
        return Some(field_term(field, raw, negated));
    }

    // A bare word searches every text field — OR within the clause, which is
    // exactly what a clause is for.
    let rules = FREE_TEXT_FIELDS
        .iter()
        .map(|f| Rule {
            field: *f,
            op: if negated {
                Operator::NotContains
            } else {
                Operator::Contains
            },
            value: Value::Text(token.to_string()),
        })
        .collect();
    Some(Clause { rules })
}

/// `!bpm>128` means "not greater than 128", i.e. `<=`.
fn flip(op: Operator) -> Operator {
    match op {
        Operator::GreaterThan => Operator::LessOrEqual,
        Operator::LessThan => Operator::GreaterOrEqual,
        Operator::GreaterOrEqual => Operator::LessThan,
        Operator::LessOrEqual => Operator::GreaterThan,
        other => other,
    }
}

fn split_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn clause(field: Field, operator: Operator, value: Value) -> Clause {
    Clause {
        rules: vec![Rule {
            field,
            op: operator,
            value,
        }],
    }
}

/// `field:value`, once the field is known.
fn field_term(field: Field, raw: &str, negated: bool) -> Clause {
    // `None` is the literal keyword, per the spec — not a value to match.
    if raw.eq_ignore_ascii_case("none") {
        return clause(
            field,
            if negated {
                Operator::IsNotNone
            } else {
                Operator::IsNone
            },
            Value::None,
        );
    }

    match field {
        Field::Tags => {
            let tags = split_tags(raw);
            clause(
                field,
                if negated {
                    Operator::HasNone
                } else {
                    Operator::HasAny
                },
                Value::Tags(tags),
            )
        }
        // Key equality is notation-aware in the evaluator — `4A` finds `Abm` —
        // which is exactly why this parses to a rule rather than a string
        // compare here.
        Field::MusicalKey => clause(
            field,
            if negated {
                Operator::NotEquals
            } else {
                Operator::Equals
            },
            Value::Text(raw.to_string()),
        ),
        Field::Bpm
        | Field::Rating
        | Field::Year
        | Field::DurationSecs
        | Field::BitRate
        | Field::SampleRate
        | Field::PlayCount
        | Field::Energy => {
            if let Some((lo, hi)) = parse_range(raw) {
                clause(field, Operator::Between, Value::Range(lo, hi))
            } else if let Some(n) = parse_number(raw) {
                clause(
                    field,
                    if negated {
                        Operator::NotEquals
                    } else {
                        Operator::Equals
                    },
                    Value::Number(n),
                )
            } else {
                // Not a number: fall back to text so `bpm:fast` finds nothing
                // rather than silently matching everything.
                clause(field, Operator::Equals, Value::Text(raw.to_string()))
            }
        }
        _ => clause(
            field,
            if negated {
                Operator::NotContains
            } else {
                Operator::Contains
            },
            Value::Text(raw.to_string()),
        ),
    }
}

/// Parse a search box query into a smartlist the evaluator can run.
///
/// Terms are ANDed — each word you add narrows the result, which is what a
/// search box is for. Within a bare word, the text fields are ORed.
///
/// A query that parses to nothing returns a smartlist with no clauses, which
/// matches everything: an unparseable search should show the library, not an
/// empty screen with no explanation.
pub fn parse(query: &str) -> Smartlist {
    let clauses = tokenise(query)
        .iter()
        .filter_map(|t| parse_term(t))
        .collect();
    Smartlist {
        id: String::new(),
        name: "Search".to_string(),
        parent_folder_id: None,
        combinator: Combinator::All,
        clauses,
        created_at: 0,
        updated_at: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only_rule(query: &str) -> Rule {
        let list = parse(query);
        assert_eq!(list.clauses.len(), 1, "expected one clause for {query:?}");
        assert_eq!(list.clauses[0].rules.len(), 1, "expected one rule");
        list.clauses[0].rules[0].clone()
    }

    #[test]
    fn a_bare_word_searches_every_text_field() {
        let list = parse("deadmau5");
        assert_eq!(list.clauses.len(), 1);
        // OR within the clause — that is what a clause is.
        assert_eq!(list.clauses[0].rules.len(), FREE_TEXT_FIELDS.len());
        assert!(list.clauses[0]
            .rules
            .iter()
            .all(|r| r.op == Operator::Contains));
    }

    #[test]
    fn terms_are_anded_so_each_word_narrows() {
        let list = parse("deadmau5 strobe");
        assert_eq!(list.clauses.len(), 2);
        assert_eq!(list.combinator, Combinator::All);
    }

    #[test]
    fn a_quoted_phrase_stays_together() {
        let list = parse("\"the strobe remix\"");
        assert_eq!(list.clauses.len(), 1);
        assert_eq!(
            list.clauses[0].rules[0].value,
            Value::Text("the strobe remix".into())
        );
    }

    #[test]
    fn a_field_prefix_narrows_to_that_field() {
        let rule = only_rule("artist:deadmau5");
        assert_eq!(rule.field, Field::Artist);
        assert_eq!(rule.op, Operator::Contains);
    }

    #[test]
    fn field_aliases_are_the_names_a_dj_would_type() {
        assert_eq!(only_rule("tempo>128").field, Field::Bpm);
        assert_eq!(only_rule("year:2024").field, Field::Year);
        assert_eq!(only_rule("time:180-300").field, Field::DurationSecs);
        assert_eq!(only_rule("stars:5").field, Field::Rating);
    }

    #[test]
    fn comparisons_parse_to_numeric_operators() {
        assert_eq!(only_rule("bpm>128").op, Operator::GreaterThan);
        assert_eq!(only_rule("bpm<128").op, Operator::LessThan);
        assert_eq!(only_rule("bpm>=128").op, Operator::GreaterOrEqual);
        assert_eq!(only_rule("bpm<=128").op, Operator::LessOrEqual);
        assert_eq!(only_rule("bpm>128").value, Value::Number(128.0));
    }

    #[test]
    fn a_negated_comparison_is_the_opposite_comparison() {
        // The rule model has no "not" wrapper, and does not need one.
        assert_eq!(only_rule("!bpm>128").op, Operator::LessOrEqual);
        assert_eq!(only_rule("!bpm<=128").op, Operator::GreaterThan);
    }

    #[test]
    fn a_range_is_inclusive_and_order_insensitive() {
        assert_eq!(only_rule("bpm:120-130").op, Operator::Between);
        assert_eq!(only_rule("bpm:120-130").value, Value::Range(120.0, 130.0));
        // Someone who typed it backwards meant the same range.
        assert_eq!(only_rule("bpm:130-120").value, Value::Range(120.0, 130.0));
    }

    #[test]
    fn none_is_the_literal_keyword_not_a_value_to_match() {
        let rule = only_rule("genre:None");
        assert_eq!(rule.op, Operator::IsNone);
        assert_eq!(rule.value, Value::None);
        // Case-insensitive, and negatable.
        assert_eq!(only_rule("genre:none").op, Operator::IsNone);
        assert_eq!(only_rule("!genre:None").op, Operator::IsNotNone);
    }

    #[test]
    fn key_search_parses_to_equality_so_the_evaluator_can_be_notation_aware() {
        // The whole reason this is a rule and not a string compare here: the
        // evaluator makes 4A equal Abm.
        let rule = only_rule("key:4A");
        assert_eq!(rule.field, Field::MusicalKey);
        assert_eq!(rule.op, Operator::Equals);
        assert_eq!(rule.value, Value::Text("4A".into()));
    }

    #[test]
    fn tilde_requires_all_the_tags() {
        let rule = only_rule("~peak,vocal");
        assert_eq!(rule.field, Field::Tags);
        assert_eq!(rule.op, Operator::HasAll);
        assert_eq!(rule.value, Value::Tags(vec!["peak".into(), "vocal".into()]));
    }

    #[test]
    fn a_tag_field_search_is_has_any() {
        // `~` is "all of these"; the plain field form is "any of these".
        assert_eq!(only_rule("tag:peak,vocal").op, Operator::HasAny);
    }

    #[test]
    fn negating_a_tag_search_excludes_them() {
        assert_eq!(only_rule("!~peak").op, Operator::HasNone);
        assert_eq!(only_rule("!tag:peak").op, Operator::HasNone);
    }

    #[test]
    fn a_negated_word_excludes_it_from_every_text_field() {
        let list = parse("!remix");
        assert!(list.clauses[0]
            .rules
            .iter()
            .all(|r| r.op == Operator::NotContains));
    }

    #[test]
    fn an_unknown_field_name_is_dropped_rather_than_guessed() {
        // `remixer:` is a real Lexicon field we do not model. Guessing which
        // field they meant would be worse than ignoring the term.
        assert!(parse("remixer:skrillex").clauses.is_empty());
    }

    #[test]
    fn an_empty_query_matches_everything() {
        assert!(parse("").clauses.is_empty());
        assert!(parse("   ").clauses.is_empty());
    }

    #[test]
    fn a_non_numeric_value_on_a_numeric_field_matches_nothing_not_everything() {
        // `bpm:fast` should find nothing, rather than being dropped and
        // silently widening the search.
        let rule = only_rule("bpm:fast");
        assert_eq!(rule.field, Field::Bpm);
        assert_eq!(rule.value, Value::Text("fast".into()));
    }

    #[test]
    fn a_title_containing_a_dash_is_not_read_as_a_range() {
        let rule = only_rule("title:jump-off");
        assert_eq!(rule.op, Operator::Contains);
        assert_eq!(rule.value, Value::Text("jump-off".into()));
    }

    // ── has_operators ────────────────────────────────────────────────────

    #[test]
    fn plain_text_needs_no_engine() {
        // The browser keeps its instant local match for these, so typing a
        // band's name never waits on a round-trip.
        assert!(!has_operators("deadmau5"));
        assert!(!has_operators("the strobe remix"));
        assert!(!has_operators(""));
    }

    #[test]
    fn anything_with_syntax_does() {
        for q in ["bpm>128", "artist:x", "~peak", "!remix", "key:4A"] {
            assert!(has_operators(q), "{q} should route through the engine");
        }
    }
}
