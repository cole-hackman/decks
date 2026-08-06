//! Casing recipes.
//!
//! The meaningful difference from `crates/smart-fixes`'s `fix_casing` is the
//! **words-to-ignore list**: Smart Fixes hardcodes an article/preposition list,
//! while a recipe takes the user's own. A DJ whose library is full of `EDM`,
//! `NYC` and `DJ` needs those left alone, and no hardcoded list will ever
//! contain them.

/// Case-insensitive membership, so a user typing `edm` protects `EDM`.
fn ignored(word: &str, ignore: &[String]) -> bool {
    let bare = word.trim_matches(|c: char| !c.is_alphanumeric());
    ignore
        .iter()
        .any(|w| w.eq_ignore_ascii_case(word) || w.eq_ignore_ascii_case(bare))
}

/// Apply `f` to each whitespace-separated word except the ignored ones.
///
/// Splitting on whitespace and rejoining with a single space would destroy
/// intentional spacing, so the original separators are preserved by walking the
/// string rather than using `split_whitespace`.
fn map_words(input: &str, ignore: &[String], f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(input.len());
    let mut word = String::new();

    let flush = |word: &mut String, out: &mut String| {
        if !word.is_empty() {
            if ignored(word, ignore) {
                out.push_str(word);
            } else {
                out.push_str(&f(word));
            }
            word.clear();
        }
    };

    for c in input.chars() {
        if c.is_whitespace() {
            flush(&mut word, &mut out);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

pub fn to_upper(input: &str, ignore: &[String]) -> String {
    map_words(input, ignore, |w| w.to_uppercase())
}

pub fn to_lower(input: &str, ignore: &[String]) -> String {
    map_words(input, ignore, |w| w.to_lowercase())
}

/// Title case: first letter of every word up, the rest down.
///
/// The first alphanumeric character is capitalised rather than simply the first
/// character, so `(original mix)` becomes `(Original Mix)` rather than being
/// left alone because it starts with a bracket.
pub fn to_title(input: &str, ignore: &[String]) -> String {
    map_words(input, ignore, |word| {
        let mut out = String::with_capacity(word.len());
        let mut capitalised = false;
        for c in word.chars() {
            if !capitalised && c.is_alphanumeric() {
                out.extend(c.to_uppercase());
                capitalised = true;
            } else {
                out.extend(c.to_lowercase());
            }
        }
        out
    })
}

/// Sentence case: the first letter of the whole string up, everything else
/// down. Takes no ignore list, matching the spec.
pub fn to_sentence(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut capitalised = false;
    for c in input.chars() {
        if !capitalised && c.is_alphanumeric() {
            out.extend(c.to_uppercase());
            capitalised = true;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ignore(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn upper_and_lower_do_the_obvious_thing() {
        assert_eq!(to_upper("get lucky", &[]), "GET LUCKY");
        assert_eq!(to_lower("GET LUCKY", &[]), "get lucky");
    }

    #[test]
    fn title_case_capitalises_every_word() {
        assert_eq!(to_title("get lucky tonight", &[]), "Get Lucky Tonight");
    }

    #[test]
    fn title_case_lowers_the_rest_of_a_shouted_word() {
        assert_eq!(to_title("GET LUCKY", &[]), "Get Lucky");
    }

    #[test]
    fn title_case_reaches_past_leading_punctuation() {
        // "(original mix)" must become "(Original Mix)", not stay as-is.
        assert_eq!(to_title("(original mix)", &[]), "(Original Mix)");
    }

    #[test]
    fn the_ignore_list_is_the_whole_point() {
        // This is what fix_casing cannot do: protect a user's own acronyms.
        assert_eq!(
            to_title("edm anthems by dj snake", &ignore(&["EDM", "DJ"])),
            "edm Anthems By dj Snake"
        );
    }

    #[test]
    fn the_ignore_list_matches_case_insensitively() {
        // A user typing "edm" expects "EDM" protected too.
        assert_eq!(to_lower("EDM Anthems", &ignore(&["edm"])), "EDM anthems");
    }

    #[test]
    fn an_ignored_word_is_matched_past_its_punctuation() {
        assert_eq!(to_title("live (edm)", &ignore(&["edm"])), "Live (edm)");
    }

    #[test]
    fn original_spacing_survives() {
        // split_whitespace + join would collapse this to one space.
        assert_eq!(to_title("get   lucky", &[]), "Get   Lucky");
        assert_eq!(to_upper("  a  b  ", &[]), "  A  B  ");
    }

    #[test]
    fn sentence_case_capitalises_only_the_first_letter() {
        assert_eq!(to_sentence("GET LUCKY tonight"), "Get lucky tonight");
    }

    #[test]
    fn sentence_case_reaches_past_leading_punctuation() {
        assert_eq!(to_sentence("(get lucky)"), "(Get lucky)");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(to_title("", &[]), "");
        assert_eq!(to_sentence(""), "");
        assert_eq!(to_upper("", &ignore(&["a"])), "");
    }

    #[test]
    fn a_string_with_no_letters_is_untouched() {
        assert_eq!(to_title("123 - 456", &[]), "123 - 456");
        assert_eq!(to_sentence("!!!"), "!!!");
    }

    #[test]
    fn non_ascii_case_conversion_works() {
        assert_eq!(to_upper("café", &[]), "CAFÉ");
        assert_eq!(to_title("ÉCOLE de nuit", &[]), "École De Nuit");
    }
}
