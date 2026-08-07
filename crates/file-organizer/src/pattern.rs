//! The `%field%` filename pattern language.
//!
//! Three constructs, and that is the whole grammar:
//!
//! - `%field%` interpolates a track field.
//! - Anything else is literal text.
//! - `{ … }` marks an **optional segment**: everything inside is emitted only
//!   if every field referenced inside it has a value.
//!
//! The optional segment is the whole point. `%artist% - %title% (%key%)` on a
//! keyless track produces `Daft Punk - Get Lucky ()` — the stray parentheses
//! are exactly the kind of mess that makes bulk renaming untrustworthy.
//! `%artist% - %title% {(%key%)}` produces `Daft Punk - Get Lucky` instead.

use std::collections::HashMap;

/// A parsed pattern. Parse once, apply to many tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Literal(String),
    Field(String),
    /// Emitted only when every `Field` inside resolves to a non-empty value.
    Optional(Vec<Node>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternError {
    UnclosedOptional,
    UnexpectedCloseBrace,
    /// A `%` with no closing `%`.
    UnterminatedField,
    NestedOptional,
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternError::UnclosedOptional => write!(f, "unclosed '{{' in pattern"),
            PatternError::UnexpectedCloseBrace => write!(f, "unmatched '}}' in pattern"),
            PatternError::UnterminatedField => write!(f, "unterminated %field% in pattern"),
            PatternError::NestedOptional => {
                write!(f, "optional segments cannot be nested")
            }
        }
    }
}

impl std::error::Error for PatternError {}

impl Pattern {
    /// Parse a pattern string.
    ///
    /// Optional segments deliberately do not nest: one level covers every
    /// documented use, and rejecting nesting gives a clear error instead of
    /// silently doing something surprising.
    pub fn parse(input: &str) -> Result<Self, PatternError> {
        let mut nodes = Vec::new();
        let mut optional: Option<Vec<Node>> = None;
        let mut literal = String::new();
        let mut chars = input.chars().peekable();

        // Close off any pending literal into the right target.
        macro_rules! flush {
            () => {
                if !literal.is_empty() {
                    let node = Node::Literal(std::mem::take(&mut literal));
                    match optional.as_mut() {
                        Some(inner) => inner.push(node),
                        None => nodes.push(node),
                    }
                }
            };
        }

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if optional.is_some() {
                        return Err(PatternError::NestedOptional);
                    }
                    flush!();
                    optional = Some(Vec::new());
                }
                '}' => {
                    let Some(mut inner) = optional.take() else {
                        return Err(PatternError::UnexpectedCloseBrace);
                    };
                    if !literal.is_empty() {
                        inner.push(Node::Literal(std::mem::take(&mut literal)));
                    }
                    nodes.push(Node::Optional(inner));
                }
                '%' => {
                    let mut name = String::new();
                    let mut closed = false;
                    for c in chars.by_ref() {
                        if c == '%' {
                            closed = true;
                            break;
                        }
                        name.push(c);
                    }
                    if !closed {
                        return Err(PatternError::UnterminatedField);
                    }
                    flush!();
                    let node = Node::Field(name.trim().to_string());
                    match optional.as_mut() {
                        Some(inner) => inner.push(node),
                        None => nodes.push(node),
                    }
                }
                _ => literal.push(c),
            }
        }

        if optional.is_some() {
            return Err(PatternError::UnclosedOptional);
        }
        flush!();
        Ok(Pattern { nodes })
    }

    /// Every field name the pattern references, so callers can check for typos
    /// against the known vocabulary before running a rename over 10,000 files.
    pub fn field_names(&self) -> Vec<&str> {
        fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a str>) {
            for n in nodes {
                match n {
                    Node::Field(name) => out.push(name.as_str()),
                    Node::Optional(inner) => walk(inner, out),
                    Node::Literal(_) => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.nodes, &mut out);
        out
    }

    /// Render the pattern against a field map.
    ///
    /// A field that is absent, empty or whitespace-only counts as having no
    /// value: outside an optional segment it renders as nothing, and inside one
    /// it suppresses the whole segment.
    ///
    /// The result is trimmed. A dropped optional segment nearly always leaves
    /// the space that separated it behind — `%artist% - %title% {(%key%)}` on a
    /// keyless track — and the manual's own worked example shows that space
    /// gone.
    pub fn render(&self, fields: &HashMap<String, String>) -> String {
        fn value<'a>(fields: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
            fields.get(name).map(|s| s.trim()).filter(|s| !s.is_empty())
        }

        let mut out = String::new();
        for node in &self.nodes {
            match node {
                Node::Literal(text) => out.push_str(text),
                Node::Field(name) => {
                    if let Some(v) = value(fields, name) {
                        out.push_str(v);
                    }
                }
                Node::Optional(inner) => {
                    // Suppress the whole segment unless every field inside has
                    // a value. A segment containing no fields at all is treated
                    // as literal text and always emitted.
                    let all_present = inner.iter().all(|n| match n {
                        Node::Field(name) => value(fields, name).is_some(),
                        _ => true,
                    });
                    if !all_present {
                        continue;
                    }
                    for n in inner {
                        match n {
                            Node::Literal(text) => out.push_str(text),
                            Node::Field(name) => {
                                if let Some(v) = value(fields, name) {
                                    out.push_str(v);
                                }
                            }
                            Node::Optional(_) => {}
                        }
                    }
                }
            }
        }
        out.trim().to_string()
    }
}

/// Characters no mainstream filesystem accepts in a name, plus the ones
/// Windows additionally forbids. Replaced rather than stripped so words do not
/// run together.
const ILLEGAL: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Make a rendered pattern safe to use as a filename component.
///
/// Renaming is a filesystem write over a user's library, so this is deliberately
/// conservative: illegal characters become `-`, control characters go, runs of
/// whitespace collapse, and leading/trailing dots and spaces are trimmed
/// (Windows silently drops trailing dots, which would desynchronise the name we
/// think we wrote from the one on disk).
pub fn sanitize_component(input: &str) -> String {
    let replaced: String = input
        .chars()
        .map(|c| {
            if ILLEGAL.contains(&c) {
                '-'
            } else if c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();

    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(|c: char| c == '.' || c.is_whitespace());
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn render(pattern: &str, pairs: &[(&str, &str)]) -> String {
        Pattern::parse(pattern).unwrap().render(&fields(pairs))
    }

    // The worked examples from the Lexicon manual, verbatim.

    #[test]
    fn artist_dash_title() {
        assert_eq!(
            render(
                "%artist% - %title%",
                &[("artist", "Daft Punk"), ("title", "Get Lucky")]
            ),
            "Daft Punk - Get Lucky"
        );
    }

    #[test]
    fn optional_key_in_parentheses_present() {
        assert_eq!(
            render(
                "%artist% - %title% {(%key%)}",
                &[
                    ("artist", "Daft Punk"),
                    ("title", "Get Lucky"),
                    ("key", "12M")
                ]
            ),
            "Daft Punk - Get Lucky (12M)"
        );
    }

    #[test]
    fn optional_key_takes_its_parentheses_with_it_when_absent() {
        // The whole point of the construct: no stray "()" left behind.
        assert_eq!(
            render(
                "%artist% - %title% {(%key%)}",
                &[("artist", "Daft Punk"), ("title", "Get Lucky")]
            ),
            "Daft Punk - Get Lucky"
        );
    }

    #[test]
    fn without_braces_an_absent_field_leaves_its_punctuation_behind() {
        // Documented contrast case — this is what the braces exist to avoid.
        assert_eq!(
            render(
                "%artist% - %title% (%key%)",
                &[("artist", "Daft Punk"), ("title", "Get Lucky")]
            ),
            "Daft Punk - Get Lucky ()"
        );
    }

    #[test]
    fn two_adjacent_optional_segments() {
        assert_eq!(
            render(
                "%artist% - %title% {%key%}{|%bpm%}",
                &[
                    ("artist", "Daft Punk"),
                    ("title", "Get Lucky"),
                    ("key", "12M"),
                    ("bpm", "128")
                ]
            ),
            "Daft Punk - Get Lucky 12M|128"
        );
    }

    #[test]
    fn adjacent_optionals_drop_independently() {
        assert_eq!(
            render(
                "%artist% - %title% {%key%}{|%bpm%}",
                &[
                    ("artist", "Daft Punk"),
                    ("title", "Get Lucky"),
                    ("bpm", "128")
                ]
            ),
            "Daft Punk - Get Lucky |128"
        );
    }

    #[test]
    fn leading_optional_segment() {
        assert_eq!(
            render(
                "{%genre% -} %artist% - %title%",
                &[
                    ("genre", "Pop"),
                    ("artist", "Daft Punk"),
                    ("title", "Get Lucky")
                ]
            ),
            "Pop - Daft Punk - Get Lucky"
        );
        // The separator the dropped segment left behind is trimmed away.
        assert_eq!(
            render(
                "{%genre% -} %artist% - %title%",
                &[("artist", "Daft Punk"), ("title", "Get Lucky")]
            ),
            "Daft Punk - Get Lucky"
        );
    }

    #[test]
    fn static_text_passes_through() {
        assert_eq!(
            render(
                "(Favorites) %artist% - %title%",
                &[("artist", "Daft Punk"), ("title", "Get Lucky")]
            ),
            "(Favorites) Daft Punk - Get Lucky"
        );
    }

    // Semantics.

    #[test]
    fn whitespace_only_values_count_as_absent() {
        assert_eq!(
            render("%artist%{ (%key%)}", &[("artist", "A"), ("key", "   ")]),
            "A"
        );
    }

    #[test]
    fn a_missing_field_outside_braces_renders_as_nothing() {
        assert_eq!(render("%artist%%title%", &[("artist", "A")]), "A");
    }

    #[test]
    fn an_optional_segment_needs_every_field_inside_it() {
        assert_eq!(
            render("{%key% %bpm%}", &[("key", "12M")]),
            "",
            "one missing field suppresses the whole segment"
        );
        assert_eq!(
            render("{%key% %bpm%}", &[("key", "12M"), ("bpm", "128")]),
            "12M 128"
        );
    }

    #[test]
    fn an_optional_segment_with_no_fields_is_literal_text() {
        assert_eq!(render("{hello}", &[]), "hello");
    }

    #[test]
    fn field_names_are_trimmed() {
        assert_eq!(render("%  artist  %", &[("artist", "A")]), "A");
    }

    #[test]
    fn reports_the_fields_a_pattern_uses() {
        let p = Pattern::parse("%artist% - %title% {(%key%)}").unwrap();
        assert_eq!(p.field_names(), vec!["artist", "title", "key"]);
    }

    // Parse errors.

    #[test]
    fn rejects_unclosed_optional() {
        assert_eq!(
            Pattern::parse("%artist% {(%key%)").unwrap_err(),
            PatternError::UnclosedOptional
        );
    }

    #[test]
    fn rejects_stray_close_brace() {
        assert_eq!(
            Pattern::parse("%artist%}").unwrap_err(),
            PatternError::UnexpectedCloseBrace
        );
    }

    #[test]
    fn rejects_unterminated_field() {
        assert_eq!(
            Pattern::parse("%artist").unwrap_err(),
            PatternError::UnterminatedField
        );
    }

    #[test]
    fn rejects_nested_optionals() {
        assert_eq!(
            Pattern::parse("{a{b}}").unwrap_err(),
            PatternError::NestedOptional
        );
    }

    #[test]
    fn an_empty_pattern_renders_empty() {
        assert_eq!(render("", &[]), "");
    }

    // Sanitisation.

    #[test]
    fn illegal_characters_become_dashes_rather_than_vanishing() {
        assert_eq!(
            sanitize_component("AC/DC: Back<In>Black"),
            "AC-DC- Back-In-Black"
        );
    }

    #[test]
    fn control_characters_and_whitespace_runs_collapse() {
        assert_eq!(sanitize_component("a\u{0}\u{1}b   c"), "a b c");
    }

    #[test]
    fn trailing_dots_and_spaces_are_trimmed() {
        // Windows silently drops these, which would desynchronise the name we
        // think we wrote from the one on disk.
        assert_eq!(sanitize_component("  Track name.  "), "Track name");
    }

    #[test]
    fn sanitising_an_empty_string_is_empty() {
        assert_eq!(sanitize_component("   "), "");
        assert_eq!(sanitize_component("..."), "");
    }
}
