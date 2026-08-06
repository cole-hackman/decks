//! Text recipes — find/replace, extraction, shortening, character cleanup.

use serde::{Deserialize, Serialize};

/// Case-insensitive find, returning byte offsets so callers can splice.
///
/// Written by hand rather than lower-casing both sides and using `find`,
/// because lower-casing can change a string's byte length (`İ` → `i̇`) and the
/// offset would then point into the wrong place in the original.
fn find_from(haystack: &str, needle: &str, from: usize, case_insensitive: bool) -> Option<usize> {
    if needle.is_empty() || from > haystack.len() {
        return None;
    }
    let hay = &haystack[from..];
    if !case_insensitive {
        return hay.find(needle).map(|i| i + from);
    }
    let needle_lower = needle.to_lowercase();
    for (offset, _) in hay.char_indices() {
        let rest = &hay[offset..];
        if rest.len() < needle.len() && rest.to_lowercase().len() < needle_lower.len() {
            break;
        }
        // Compare a prefix of equal *character* count, so multi-byte
        // characters cannot cause a mid-character slice.
        let candidate: String = rest.chars().take(needle.chars().count()).collect();
        if candidate.to_lowercase() == needle_lower {
            return Some(offset + from);
        }
    }
    None
}

/// Replace every occurrence, honouring the case-insensitive toggle.
pub fn replace_text(input: &str, find: &str, replace: &str, case_insensitive: bool) -> String {
    if find.is_empty() {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(at) = find_from(input, find, cursor, case_insensitive) {
        out.push_str(&input[cursor..at]);
        out.push_str(replace);
        // Advance by the matched *source* length, which under a
        // case-insensitive match is the character count of the needle rather
        // than its byte length.
        let matched: String = input[at..].chars().take(find.chars().count()).collect();
        cursor = at + matched.len();
    }
    out.push_str(&input[cursor..]);
    out
}

/// Remove every occurrence — replace with nothing.
pub fn remove_text(input: &str, text: &str, case_insensitive: bool) -> String {
    replace_text(input, text, "", case_insensitive)
}

/// Abbreviate each word to its first `n` characters. `"Get Lucky"` at 2 gives
/// `"GeLu"` — the words are joined, which is what the manual's example shows.
pub fn shorten_text(input: &str, chars_per_word: usize) -> String {
    if chars_per_word == 0 {
        return String::new();
    }
    input
        .split_whitespace()
        .map(|w| w.chars().take(chars_per_word).collect::<String>())
        .collect()
}

/// A delimiter pair for `Remove Between`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelimiterPair {
    Parentheses,
    Brackets,
    Braces,
    Angles,
    DoubleQuotes,
    SingleQuotes,
}

impl DelimiterPair {
    pub fn chars(self) -> (char, char) {
        match self {
            DelimiterPair::Parentheses => ('(', ')'),
            DelimiterPair::Brackets => ('[', ']'),
            DelimiterPair::Braces => ('{', '}'),
            DelimiterPair::Angles => ('<', '>'),
            DelimiterPair::DoubleQuotes => ('"', '"'),
            DelimiterPair::SingleQuotes => ('\'', '\''),
        }
    }
}

/// Strip text between a delimiter pair, **including** the delimiters.
///
/// Whitespace left behind is collapsed, because `"Track (Original Mix) Live"`
/// otherwise becomes `"Track  Live"` with a double space — visible, and exactly
/// the kind of thing a cleanup recipe is supposed to prevent.
pub fn remove_between(input: &str, pair: DelimiterPair) -> String {
    let (open, close) = pair.chars();
    let symmetric = open == close;

    let mut out = String::with_capacity(input.len());
    let mut depth = 0usize;
    for c in input.chars() {
        if c == open && (!symmetric || depth == 0) {
            depth += 1;
            continue;
        }
        if c == close && depth > 0 {
            depth -= 1;
            continue;
        }
        if depth == 0 {
            out.push(c);
        }
    }
    collapse_spaces(&out)
}

fn collapse_spaces(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_space = false;
    for c in input.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

/// What `extract_text` found and what it left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// The extracted text, `None` when the delimiters were not found.
    pub extracted: Option<String>,
    /// The source with the match removed, when deletion was requested.
    pub remaining_source: String,
}

/// Extract the text between a start and end delimiter.
///
/// The end delimiter is searched for *after* the start, so
/// `extract("A (B) C", "(", ")")` yields `B` rather than spanning the string.
pub fn extract_text(
    input: &str,
    start: &str,
    end: &str,
    include_delimiters: bool,
    delete_from_source: bool,
) -> Extraction {
    let no_match = Extraction {
        extracted: None,
        remaining_source: input.to_string(),
    };
    if start.is_empty() || end.is_empty() {
        return no_match;
    }

    let Some(open_at) = find_from(input, start, 0, false) else {
        return no_match;
    };
    let inner_from = open_at + start.len();
    let Some(close_at) = find_from(input, end, inner_from, false) else {
        return no_match;
    };

    let extracted = if include_delimiters {
        input[open_at..close_at + end.len()].to_string()
    } else {
        input[inner_from..close_at].to_string()
    };

    // Deleting always removes the delimiters too — leaving "()" behind would
    // be worse than leaving the whole thing.
    let remaining_source = if delete_from_source {
        let mut s = String::with_capacity(input.len());
        s.push_str(&input[..open_at]);
        s.push_str(&input[close_at + end.len()..]);
        collapse_spaces(&s)
    } else {
        input.to_string()
    };

    Extraction {
        extracted: Some(extracted),
        remaining_source,
    }
}

/// Which class of characters `remove_special_characters` targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialCharacterMode {
    /// Currency, legal and full-width symbols normalised to ASCII; zero-width
    /// characters and diacritics stripped.
    Special,
    /// Emoji, plus their modifiers — skin tones, variation selectors, ZWJ.
    Emojis,
}

/// Symbol substitutions from the spec, applied before the general passes.
const SUBSTITUTIONS: &[(char, &str)] = &[
    ('$', "S"),
    ('@', "A"),
    ('€', "E"),
    ('£', "E"),
    ('¥', "Y"),
    ('®', "(R)"),
    ('™', "(tm)"),
    ('©', "(c)"),
];

/// Characters with no width that survive copy-paste and break exact matching.
fn is_zero_width(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200D}' | '\u{FEFF}' | '\u{2060}')
}

/// Combining marks — what is left after decomposing an accented character.
fn is_combining_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF)
}

/// Full-width forms map to ASCII by a fixed offset.
fn from_full_width(c: char) -> Option<char> {
    let code = c as u32;
    if (0xFF01..=0xFF5E).contains(&code) {
        char::from_u32(code - 0xFEE0)
    } else {
        None
    }
}

fn is_emoji(c: char) -> bool {
    let code = c as u32;
    // Skin-tone modifiers (U+1F3FB–1F3FF) are inside the pictograph range and
    // need no separate arm; the ZWJ and variation selectors do, since they sit
    // far from it and are what stitch multi-codepoint sequences together.
    matches!(code,
        0x1F300..=0x1FAFF   // symbols, pictographs, supplemental, skin tones
        | 0x1F000..=0x1F02F // mahjong / dominoes
        | 0x2600..=0x27BF   // misc symbols and dingbats
        | 0x1F1E6..=0x1F1FF // regional indicators (flags)
        | 0xFE00..=0xFE0F   // variation selectors
        | 0x200D            // zero-width joiner
    )
}

/// Strip or normalise characters that make a library hard to search.
///
/// Diacritic stripping is done by hand for the Latin-1 range rather than
/// pulling in a Unicode normalisation crate: this is a text tidy for DJ
/// metadata, the coverage that matters is European artist names, and a
/// dependency for `é → e` is not a trade worth making.
pub fn remove_special_characters(input: &str, mode: SpecialCharacterMode) -> String {
    match mode {
        SpecialCharacterMode::Emojis => input.chars().filter(|c| !is_emoji(*c)).collect(),
        SpecialCharacterMode::Special => {
            let mut out = String::with_capacity(input.len());
            for c in input.chars() {
                if let Some((_, replacement)) = SUBSTITUTIONS.iter().find(|(from, _)| *from == c) {
                    out.push_str(replacement);
                    continue;
                }
                if is_zero_width(c) || is_combining_mark(c) {
                    continue;
                }
                if let Some(ascii) = from_full_width(c) {
                    out.push(ascii);
                    continue;
                }
                if let Some(plain) = strip_diacritic(c) {
                    out.push(plain);
                    continue;
                }
                out.push(c);
            }
            out
        }
    }
}

/// Latin-1 and Latin Extended-A accented letters → their base letter.
fn strip_diacritic(c: char) -> Option<char> {
    const TABLE: &[(&str, char)] = &[
        ("ÀÁÂÃÄÅĀĂĄ", 'A'),
        ("àáâãäåāăą", 'a'),
        ("ÇĆĈĊČ", 'C'),
        ("çćĉċč", 'c'),
        ("ÈÉÊËĒĔĖĘĚ", 'E'),
        ("èéêëēĕėęě", 'e'),
        ("ÌÍÎÏĨĪĬĮ", 'I'),
        ("ìíîïĩīĭį", 'i'),
        ("ÑŃŅŇ", 'N'),
        ("ñńņň", 'n'),
        ("ÒÓÔÕÖŌŎŐ", 'O'),
        ("òóôõöōŏő", 'o'),
        ("ÙÚÛÜŨŪŬŮŰŲ", 'U'),
        ("ùúûüũūŭůűų", 'u'),
        ("ÝŸ", 'Y'),
        ("ýÿ", 'y'),
        ("ŚŜŞŠ", 'S'),
        ("śŝşš", 's'),
        ("ŹŻŽ", 'Z'),
        ("źżž", 'z'),
    ];
    TABLE
        .iter()
        .find(|(accented, _)| accented.contains(c))
        .map(|(_, plain)| *plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_is_case_sensitive_by_default() {
        assert_eq!(replace_text("Get Lucky", "get", "X", false), "Get Lucky");
        assert_eq!(replace_text("Get Lucky", "Get", "X", false), "X Lucky");
    }

    #[test]
    fn replace_can_ignore_case() {
        assert_eq!(replace_text("Get Lucky", "get", "X", true), "X Lucky");
        assert_eq!(replace_text("GET get GeT", "get", "x", true), "x x x");
    }

    #[test]
    fn replace_handles_every_occurrence() {
        assert_eq!(replace_text("a-a-a", "a", "b", false), "b-b-b");
    }

    #[test]
    fn an_empty_search_changes_nothing_rather_than_looping() {
        assert_eq!(replace_text("abc", "", "x", false), "abc");
    }

    #[test]
    fn replace_survives_multibyte_text() {
        assert_eq!(
            replace_text("café société", "é", "e", false),
            "cafe societe"
        );
        assert_eq!(replace_text("CAFÉ", "é", "e", true), "CAFe");
    }

    #[test]
    fn remove_is_replace_with_nothing() {
        assert_eq!(
            remove_text("Track (Original Mix)", " (Original Mix)", false),
            "Track"
        );
    }

    #[test]
    fn shorten_abbreviates_each_word_and_joins() {
        // The manual's worked example.
        assert_eq!(shorten_text("Get Lucky", 2), "GeLu");
    }

    #[test]
    fn shorten_by_zero_yields_nothing() {
        assert_eq!(shorten_text("Get Lucky", 0), "");
    }

    #[test]
    fn shorten_leaves_short_words_whole() {
        assert_eq!(shorten_text("a bc def", 2), "abcde");
    }

    #[test]
    fn remove_between_strips_the_delimiters_too() {
        assert_eq!(
            remove_between("Track (Original Mix)", DelimiterPair::Parentheses),
            "Track"
        );
    }

    #[test]
    fn remove_between_collapses_the_gap_it_leaves() {
        // "Track  Live" with a double space is exactly what a cleanup recipe
        // should not produce.
        assert_eq!(
            remove_between("Track (Original Mix) Live", DelimiterPair::Parentheses),
            "Track Live"
        );
    }

    #[test]
    fn remove_between_handles_nesting() {
        assert_eq!(
            remove_between("A (B (C) D) E", DelimiterPair::Parentheses),
            "A E"
        );
    }

    #[test]
    fn remove_between_handles_symmetric_delimiters() {
        // Quotes cannot nest, so the second one closes the first.
        assert_eq!(
            remove_between("Say \"hello\" now", DelimiterPair::DoubleQuotes),
            "Say now"
        );
    }

    #[test]
    fn an_unclosed_delimiter_removes_to_the_end_rather_than_erroring() {
        assert_eq!(
            remove_between("Track (Live", DelimiterPair::Parentheses),
            "Track"
        );
    }

    #[test]
    fn every_delimiter_pair_is_mapped() {
        for pair in [
            DelimiterPair::Parentheses,
            DelimiterPair::Brackets,
            DelimiterPair::Braces,
            DelimiterPair::Angles,
            DelimiterPair::DoubleQuotes,
            DelimiterPair::SingleQuotes,
        ] {
            let (o, c) = pair.chars();
            let input = format!("A {o}X{c} B");
            assert_eq!(remove_between(&input, pair), "A B", "pair {pair:?}");
        }
    }

    #[test]
    fn extract_takes_what_is_between_the_delimiters() {
        let got = extract_text("Track (Original Mix)", "(", ")", false, false);
        assert_eq!(got.extracted.as_deref(), Some("Original Mix"));
        assert_eq!(got.remaining_source, "Track (Original Mix)");
    }

    #[test]
    fn extract_can_include_the_delimiters() {
        let got = extract_text("Track (Original Mix)", "(", ")", true, false);
        assert_eq!(got.extracted.as_deref(), Some("(Original Mix)"));
    }

    #[test]
    fn extract_can_delete_from_the_source_and_tidies_after_itself() {
        let got = extract_text("Track (Original Mix) Live", "(", ")", false, true);
        assert_eq!(got.extracted.as_deref(), Some("Original Mix"));
        assert_eq!(got.remaining_source, "Track Live");
    }

    #[test]
    fn extract_finds_the_end_after_the_start_not_anywhere() {
        // A naive search for ")" from position 0 would find the wrong one.
        let got = extract_text("(A) and (B)", "(", ")", false, false);
        assert_eq!(got.extracted.as_deref(), Some("A"));
    }

    #[test]
    fn extract_reports_no_match_rather_than_an_empty_string() {
        // An empty extraction and a missing one mean different things to the
        // caller: one overwrites the target, the other must not.
        let got = extract_text("Track", "(", ")", false, true);
        assert_eq!(got.extracted, None);
        assert_eq!(got.remaining_source, "Track");
    }

    #[test]
    fn extract_with_an_empty_delimiter_matches_nothing() {
        assert_eq!(
            extract_text("Track (x)", "", ")", false, false).extracted,
            None
        );
        assert_eq!(
            extract_text("Track (x)", "(", "", false, false).extracted,
            None
        );
    }

    #[test]
    fn currency_and_legal_symbols_become_ascii() {
        let got = remove_special_characters("$5 @ Café™ ®", SpecialCharacterMode::Special);
        assert_eq!(got, "S5 A Cafe(tm) (R)");
    }

    #[test]
    fn diacritics_are_stripped() {
        assert_eq!(
            remove_special_characters("Émile Zoë Ñuñez", SpecialCharacterMode::Special),
            "Emile Zoe Nunez"
        );
    }

    #[test]
    fn full_width_alphanumerics_normalise_to_ascii() {
        assert_eq!(
            remove_special_characters("ＡＢＣ１２３", SpecialCharacterMode::Special),
            "ABC123"
        );
    }

    #[test]
    fn zero_width_characters_are_stripped() {
        // These survive copy-paste and silently break exact matching.
        let sneaky = "Get\u{200B}Lucky\u{FEFF}";
        assert_eq!(
            remove_special_characters(sneaky, SpecialCharacterMode::Special),
            "GetLucky"
        );
    }

    #[test]
    fn emoji_mode_strips_emoji_and_their_modifiers() {
        assert_eq!(
            remove_special_characters("Party 🎉 time 👍🏽", SpecialCharacterMode::Emojis),
            "Party  time "
        );
    }

    #[test]
    fn emoji_mode_leaves_ordinary_text_alone() {
        assert_eq!(
            remove_special_characters("Café ÀÉÎ", SpecialCharacterMode::Emojis),
            "Café ÀÉÎ"
        );
    }

    #[test]
    fn special_mode_leaves_emoji_alone_and_vice_versa() {
        // The two modes are separate settings in the spec; neither should do
        // the other's job by accident.
        assert_eq!(
            remove_special_characters("🎉", SpecialCharacterMode::Special),
            "🎉"
        );
        assert_eq!(
            remove_special_characters("$", SpecialCharacterMode::Emojis),
            "$"
        );
    }
}
