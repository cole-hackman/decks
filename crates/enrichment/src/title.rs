//! Turning a DJ's filename-shaped title into something a database will match.
//!
//! Per `docs/lexicon/07-health.md §Find Tags & Album Art`, accuracy "depends
//! entirely on clean incoming artist/title tags", and Lexicon points users at
//! Smart Fixes first. That advice is right and this module does not replace it
//! — but a title carrying `(Extended Mix)` is not dirty, it is *normal*, and
//! failing to match it would make the feature useless on the exact library it
//! exists for.

/// Suffixes that describe a *version* rather than the work.
///
/// Kept to version words only. `(feat. X)` is deliberately absent: the featured
/// artist is part of how the recording is credited, and stripping it turns a
/// specific match into an ambiguous one.
const VERSION_WORDS: &[&str] = &[
    "original mix",
    "extended mix",
    "extended",
    "radio edit",
    "radio mix",
    "club mix",
    "club edit",
    "remaster",
    "remastered",
    "remastered version",
    "digital remaster",
    "anniversary edition",
    "deluxe edition",
    "mono version",
    "stereo version",
    "single version",
    "album version",
    "instrumental",
    "dub mix",
    "vip mix",
    "bootleg",
    "edit",
    "remix",
    "rework",
    "refix",
];

/// Split a title into (work, version-text-that-was-removed).
///
/// Only *trailing* bracketed groups are considered, and only when what is
/// inside looks like a version. `Blue (Da Ba Dee)` keeps its bracket; `Blue
/// (Extended Mix)` loses it.
pub fn strip_version(title: &str) -> (String, Vec<String>) {
    let mut work = title.trim().to_string();
    let mut removed = Vec::new();

    loop {
        let Some(group) = trailing_group(&work) else {
            break;
        };
        if !looks_like_version(&group.inner) {
            break;
        }
        removed.push(group.inner.clone());
        work.truncate(group.start);
        work = work.trim_end().to_string();
    }

    // A title that was *entirely* a version marker is not improved by becoming
    // empty; hand back the original rather than an unmatchable blank.
    if work.is_empty() {
        return (title.trim().to_string(), Vec::new());
    }
    removed.reverse();
    (work, removed)
}

struct Group {
    /// Byte index where the opening bracket sits.
    start: usize,
    inner: String,
}

/// The last `(...)`, `[...]` or `{...}` group, if the title ends with one.
fn trailing_group(s: &str) -> Option<Group> {
    let t = s.trim_end();
    let (open, close) = match t.chars().next_back()? {
        ')' => ('(', ')'),
        ']' => ('[', ']'),
        '}' => ('{', '}'),
        _ => return None,
    };
    // Scan backwards for the matching opener, honouring nesting so
    // `A (B (C))` does not stop at the inner one.
    let mut depth = 0usize;
    for (i, c) in t.char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                let inner = t[i + c.len_utf8()..t.len() - close.len_utf8()].to_string();
                return Some(Group { start: i, inner });
            }
        }
    }
    None
}

/// Does this bracketed text describe a version rather than part of the title?
///
/// True when it *ends* with a version word — which is how these are written:
/// "Extended Mix", "Sasha Remix", "2011 Remaster". Matching anywhere in the
/// string would strip `(Dub Be Good to Me)` for containing "dub".
fn looks_like_version(inner: &str) -> bool {
    let lower = inner.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }
    VERSION_WORDS.iter().any(|w| {
        lower == *w
            || lower
                .strip_suffix(w)
                .is_some_and(|head| head.ends_with(' ') || head.is_empty())
    })
}

/// Split `"Artist - Title"` when the tags gave us only one of them.
///
/// Rekordbox libraries built from downloads are full of tracks whose title is
/// the whole filename and whose artist is blank. Searching for that string as a
/// title matches nothing at all.
pub fn split_artist_title(raw: &str) -> Option<(String, String)> {
    // Only the spaced hyphen, and only the first one. A bare `-` appears inside
    // real titles ("Re-Wired"), and a later one usually separates title from a
    // version suffix rather than artist from title.
    let (a, t) = raw.split_once(" - ")?;
    let (a, t) = (a.trim(), t.trim());
    if a.is_empty() || t.is_empty() {
        return None;
    }
    Some((a.to_string(), t.to_string()))
}

/// Escape a value for a Lucene query, which is what MusicBrainz's search takes.
///
/// Without this, a title containing `:` or `!` — both common — is a syntax
/// error rather than a search, and the whole request 400s.
pub fn lucene_escape(s: &str) -> String {
    const SPECIAL: &[char] = &[
        '\\', '+', '-', '&', '|', '!', '(', ')', '{', '}', '[', ']', '^', '"', '~', '*', '?', ':',
        '/',
    ];
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if SPECIAL.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_suffix_is_stripped() {
        let (work, removed) = strip_version("Around the World (Extended Mix)");
        assert_eq!(work, "Around the World");
        assert_eq!(removed, vec!["Extended Mix"]);
    }

    #[test]
    fn a_named_remix_is_stripped() {
        let (work, _) = strip_version("Strobe (Sasha Involv3r Remix)");
        assert_eq!(work, "Strobe");
    }

    #[test]
    fn a_year_prefixed_remaster_is_stripped() {
        let (work, _) = strip_version("Heroes (2017 Remaster)");
        assert_eq!(work, "Heroes");
    }

    #[test]
    fn brackets_and_braces_count_too() {
        assert_eq!(strip_version("Track [Radio Edit]").0, "Track");
        assert_eq!(strip_version("Track {Club Mix}").0, "Track");
    }

    #[test]
    fn several_suffixes_come_off_in_order() {
        let (work, removed) = strip_version("Song (Remastered) (Radio Edit)");
        assert_eq!(work, "Song");
        assert_eq!(removed, vec!["Remastered", "Radio Edit"]);
    }

    #[test]
    fn a_bracket_that_is_part_of_the_title_survives() {
        // The whole reason this is a suffix-match rather than a contains-match.
        assert_eq!(strip_version("Blue (Da Ba Dee)").0, "Blue (Da Ba Dee)");
        assert_eq!(strip_version("Dub Be Good to Me").0, "Dub Be Good to Me");
    }

    #[test]
    fn a_version_word_inside_a_longer_phrase_is_not_a_suffix() {
        // "Remix" appears, but not as the trailing word.
        assert_eq!(
            strip_version("Song (Remix Contest Winner Announcement)").0,
            "Song (Remix Contest Winner Announcement)"
        );
    }

    #[test]
    fn a_featured_artist_is_kept() {
        // Part of how the recording is credited; dropping it makes the search
        // less specific, not more.
        assert_eq!(
            strip_version("Get Lucky (feat. Pharrell Williams)").0,
            "Get Lucky (feat. Pharrell Williams)"
        );
    }

    #[test]
    fn a_title_that_is_only_a_version_marker_is_left_alone() {
        // Better an odd search than an empty one.
        assert_eq!(strip_version("(Original Mix)").0, "(Original Mix)");
    }

    #[test]
    fn nesting_does_not_confuse_the_scan() {
        let (work, _) = strip_version("Song (Someone (Special) Remix)");
        assert_eq!(work, "Song");
    }

    #[test]
    fn a_title_with_no_brackets_is_unchanged() {
        let (work, removed) = strip_version("Windowlicker");
        assert_eq!(work, "Windowlicker");
        assert!(removed.is_empty());
    }

    #[test]
    fn artist_and_title_split_on_the_spaced_hyphen() {
        assert_eq!(
            split_artist_title("Daft Punk - Around the World"),
            Some(("Daft Punk".into(), "Around the World".into()))
        );
    }

    #[test]
    fn a_hyphenated_word_is_not_a_split() {
        // "Re-Wired" is one word, not an artist and a title.
        assert_eq!(split_artist_title("Re-Wired"), None);
    }

    #[test]
    fn only_the_first_separator_splits() {
        assert_eq!(
            split_artist_title("A - B - C"),
            Some(("A".into(), "B - C".into()))
        );
    }

    #[test]
    fn an_empty_side_is_not_a_split() {
        assert_eq!(split_artist_title(" - Title"), None);
        assert_eq!(split_artist_title("Artist - "), None);
    }

    #[test]
    fn lucene_specials_are_escaped() {
        // A colon in a title is a field separator to Lucene, and turns the
        // whole request into a 400 rather than a search.
        assert_eq!(lucene_escape("A:B"), "A\\:B");
        assert_eq!(lucene_escape("Hey!"), "Hey\\!");
        assert_eq!(lucene_escape("50/50"), "50\\/50");
        assert_eq!(lucene_escape("plain"), "plain");
    }
}
