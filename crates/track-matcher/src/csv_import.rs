//! Bulk metadata import from a spreadsheet.
//!
//! Distinct from [`crate::csv_input`], which parses a CSV only to *find* tracks.
//! This one parses a CSV in order to *write fields onto* them: the row carries
//! the values, and the match is how we decide which track they belong to.
//!
//! Two matching strategies, per the spec: on a `Location` column (a file path),
//! or on `Artist` + `Title` together. **At least one must be configured** — a
//! mapping with neither would silently match nothing, and an import that
//! reports "0 rows matched" without saying why is indistinguishable from a
//! broken file.
//!
//! Everything here is pure. Nothing opens a database or reads a file; the
//! library is passed in and the result is a plan. See
//! `docs/lexicon/10-recipes.md §Import Tags From CSV`.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Which CSV columns mean what.
///
/// Column *names*, not indices — a user reorders columns in Excel without
/// thinking about it, and a saved mapping should survive that.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportColumns {
    /// File path column, matched against the track's `FolderPath`.
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    /// `(csv header, track field)` — the values to write.
    #[serde(default)]
    pub fields: Vec<(String, String)>,
}

impl ImportColumns {
    fn matches_on_location(&self) -> bool {
        self.location.as_deref().is_some_and(|c| !c.is_empty())
    }

    fn matches_on_artist_title(&self) -> bool {
        self.artist.as_deref().is_some_and(|c| !c.is_empty())
            && self.title.as_deref().is_some_and(|c| !c.is_empty())
    }
}

/// A track as the importer needs to see it.
#[derive(Debug, Clone)]
pub struct ImportCandidate {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub folder_path: Option<String>,
    /// Current values, so the plan can skip rows that would change nothing.
    pub current: BTreeMap<String, String>,
}

/// One parsed CSV row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRow {
    /// 1-based, counting the header as row 1 — what the spreadsheet shows.
    pub line: usize,
    pub location: Option<String>,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub values: BTreeMap<String, String>,
}

/// How a row was resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RowOutcome {
    /// Matched one track, with changes to write.
    Matched {
        track_id: String,
        track_title: String,
        /// `(field, before, after)`.
        changes: Vec<(String, Option<String>, String)>,
    },
    /// Matched one track, but every value already agreed.
    AlreadyCurrent { track_id: String },
    /// No track matched.
    Unmatched,
    /// Several tracks matched. Deliberately not resolved by picking one — the
    /// spreadsheet cannot say which, and guessing writes the values onto the
    /// wrong track.
    Ambiguous { count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedRow {
    pub row: ImportRow,
    pub outcome: RowOutcome,
}

/// Strip a UTF-8 byte-order mark.
///
/// Excel writes one on "CSV UTF-8" export, and it lands inside the *first
/// header name* — so a mapping that names the first column stops matching, and
/// the failure looks like a typo in the user's mapping rather than a file
/// format detail.
fn strip_bom(input: &str) -> &str {
    input.strip_prefix('\u{feff}').unwrap_or(input)
}

/// The header row, for the column-picker dropdowns.
pub fn headers(input: &str) -> Result<Vec<String>> {
    let mut rdr = csv::Reader::from_reader(strip_bom(input).as_bytes());
    Ok(rdr.headers()?.iter().map(|s| s.to_string()).collect())
}

/// Parse the rows a mapping describes.
///
/// Unknown column names are an error rather than an empty column: a mapping
/// that names a header the file does not have is a mistake the user can fix,
/// and importing blanks over their metadata is not recoverable.
pub fn parse(input: &str, columns: &ImportColumns) -> Result<Vec<ImportRow>> {
    if !columns.matches_on_location() && !columns.matches_on_artist_title() {
        bail!("choose how rows should match: a Location column, or Artist and Title together");
    }
    if columns.fields.is_empty() {
        bail!("choose at least one column to import");
    }

    let mut rdr = csv::Reader::from_reader(strip_bom(input).as_bytes());
    let header = rdr.headers()?.clone();
    let index_of = |name: &str| -> Result<usize> {
        header
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| anyhow!("the file has no column named {name:?}"))
    };

    let location_idx = columns
        .location
        .as_deref()
        .filter(|c| !c.is_empty())
        .map(index_of)
        .transpose()?;
    let artist_idx = columns
        .artist
        .as_deref()
        .filter(|c| !c.is_empty())
        .map(index_of)
        .transpose()?;
    let title_idx = columns
        .title
        .as_deref()
        .filter(|c| !c.is_empty())
        .map(index_of)
        .transpose()?;
    let field_idx: Vec<(usize, String)> = columns
        .fields
        .iter()
        .map(|(header_name, field)| Ok((index_of(header_name)?, field.clone())))
        .collect::<Result<_>>()?;

    let cell = |rec: &csv::StringRecord, idx: Option<usize>| -> Option<String> {
        idx.and_then(|i| rec.get(i))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };

    let mut out = Vec::new();
    for (n, rec) in rdr.records().enumerate() {
        let rec = rec?;
        let mut values = BTreeMap::new();
        for (idx, field) in &field_idx {
            // An empty cell means "leave this field alone", not "clear it".
            // Excel is full of empty cells; treating them as deletions would
            // wipe metadata on every partial import.
            if let Some(value) = rec.get(*idx).map(str::trim).filter(|s| !s.is_empty()) {
                values.insert(field.clone(), value.to_string());
            }
        }
        out.push(ImportRow {
            // +2: one for the header, one because spreadsheets count from 1.
            line: n + 2,
            location: cell(&rec, location_idx),
            artist: cell(&rec, artist_idx),
            title: cell(&rec, title_idx),
            values,
        });
    }
    Ok(out)
}

/// Compare paths the way a user means them.
///
/// Separators are interchangeable and case is ignored: a CSV exported on
/// Windows and a library indexed on macOS describe the same file, and refusing
/// to match them would make the Location strategy useless in exactly the case
/// it exists for.
fn path_key(path: &str) -> String {
    path.replace('\\', "/").trim().to_lowercase()
}

fn name_key(artist: Option<&str>, title: &str) -> String {
    format!(
        "{}\u{0}{}",
        crate::normalise::full(artist.unwrap_or("")),
        crate::normalise::title_only(title)
    )
}

/// Plan an import: resolve each row to a track and work out what would change.
///
/// Location wins over Artist + Title when both are configured. A path is an
/// identity; a name is a description, and two different mixes share one.
pub fn plan(rows: &[ImportRow], library: &[ImportCandidate]) -> Vec<PlannedRow> {
    let mut by_path: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, c) in library.iter().enumerate() {
        if let Some(p) = c.folder_path.as_deref() {
            by_path.entry(path_key(p)).or_default().push(i);
        }
        by_name
            .entry(name_key(c.artist.as_deref(), &c.title))
            .or_default()
            .push(i);
    }

    rows.iter()
        .map(|row| {
            let hits: Option<&Vec<usize>> = row
                .location
                .as_deref()
                .and_then(|p| by_path.get(&path_key(p)))
                .or_else(|| {
                    row.title
                        .as_deref()
                        .and_then(|t| by_name.get(&name_key(row.artist.as_deref(), t)))
                });

            let outcome = match hits.map(Vec::as_slice) {
                None | Some([]) => RowOutcome::Unmatched,
                Some([one]) => {
                    let track = &library[*one];
                    let changes: Vec<(String, Option<String>, String)> = row
                        .values
                        .iter()
                        .filter(|(field, value)| {
                            track.current.get(*field).map(String::as_str) != Some(value.as_str())
                        })
                        .map(|(field, value)| {
                            (
                                field.clone(),
                                track.current.get(field).cloned(),
                                value.clone(),
                            )
                        })
                        .collect();
                    if changes.is_empty() {
                        RowOutcome::AlreadyCurrent {
                            track_id: track.id.clone(),
                        }
                    } else {
                        RowOutcome::Matched {
                            track_id: track.id.clone(),
                            track_title: track.title.clone(),
                            changes,
                        }
                    }
                }
                Some(many) => RowOutcome::Ambiguous { count: many.len() },
            };
            PlannedRow {
                row: row.clone(),
                outcome,
            }
        })
        .collect()
}

/// Counts for the report the spec asks for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub rows: usize,
    pub matched: usize,
    pub already_current: usize,
    pub unmatched: usize,
    pub ambiguous: usize,
    pub changes: usize,
}

pub fn report(planned: &[PlannedRow]) -> ImportReport {
    let mut r = ImportReport {
        rows: planned.len(),
        ..Default::default()
    };
    for p in planned {
        match &p.outcome {
            RowOutcome::Matched { changes, .. } => {
                r.matched += 1;
                r.changes += changes.len();
            }
            RowOutcome::AlreadyCurrent { .. } => r.already_current += 1,
            RowOutcome::Unmatched => r.unmatched += 1,
            RowOutcome::Ambiguous { .. } => r.ambiguous += 1,
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> ImportColumns {
        ImportColumns {
            location: None,
            artist: Some("Artist".into()),
            title: Some("Title".into()),
            fields: vec![("Genre".into(), "genre".into())],
        }
    }

    fn candidate(
        id: &str,
        title: &str,
        artist: Option<&str>,
        path: Option<&str>,
    ) -> ImportCandidate {
        ImportCandidate {
            id: id.into(),
            title: title.into(),
            artist: artist.map(String::from),
            folder_path: path.map(String::from),
            current: BTreeMap::new(),
        }
    }

    // ── parsing ─────────────────────────────────────────────────────────────

    #[test]
    fn a_mapping_with_no_matching_strategy_is_refused() {
        // It would match nothing, and "0 rows matched" reads as a broken file.
        let cols = ImportColumns {
            fields: vec![("Genre".into(), "genre".into())],
            ..Default::default()
        };
        let err = parse("Genre\nHouse\n", &cols).unwrap_err().to_string();
        assert!(err.contains("how rows should match"), "{err}");
    }

    #[test]
    fn artist_alone_is_not_a_matching_strategy() {
        let cols = ImportColumns {
            artist: Some("Artist".into()),
            fields: vec![("Genre".into(), "genre".into())],
            ..Default::default()
        };
        assert!(parse("Artist,Genre\na,House\n", &cols).is_err());
    }

    #[test]
    fn a_mapping_with_nothing_to_import_is_refused() {
        let cols = ImportColumns {
            artist: Some("Artist".into()),
            title: Some("Title".into()),
            ..Default::default()
        };
        let err = parse("Artist,Title\na,b\n", &cols).unwrap_err().to_string();
        assert!(err.contains("at least one column to import"), "{err}");
    }

    #[test]
    fn a_column_the_file_does_not_have_is_an_error_not_a_blank() {
        // Importing blanks over good metadata is not recoverable.
        let mut cols = columns();
        cols.fields = vec![("Nope".into(), "genre".into())];
        let err = parse("Artist,Title\na,b\n", &cols).unwrap_err().to_string();
        assert!(err.contains("no column named \"Nope\""), "{err}");
    }

    #[test]
    fn an_excel_byte_order_mark_does_not_break_the_first_column() {
        // Excel's "CSV UTF-8" writes one, and it lands inside the header name.
        let csv = "\u{feff}Artist,Title,Genre\nDaft Punk,Get Lucky,Disco\n";
        assert_eq!(headers(csv).unwrap()[0], "Artist");
        let rows = parse(csv, &columns()).unwrap();
        assert_eq!(rows[0].artist.as_deref(), Some("Daft Punk"));
    }

    #[test]
    fn an_empty_cell_means_leave_it_alone_not_clear_it() {
        // Spreadsheets are full of blanks; treating them as deletions would
        // wipe metadata on every partial import.
        let rows = parse("Artist,Title,Genre\na,b,\n", &columns()).unwrap();
        assert!(rows[0].values.is_empty());
    }

    #[test]
    fn row_numbers_match_what_the_spreadsheet_shows() {
        let rows = parse("Artist,Title,Genre\na,b,House\nc,d,Techno\n", &columns()).unwrap();
        assert_eq!(rows[0].line, 2);
        assert_eq!(rows[1].line, 3);
    }

    #[test]
    fn extra_columns_are_ignored() {
        let rows = parse("Artist,Title,Genre,Notes\na,b,House,whatever\n", &columns()).unwrap();
        assert_eq!(
            rows[0].values.get("genre").map(String::as_str),
            Some("House")
        );
    }

    // ── matching ────────────────────────────────────────────────────────────

    #[test]
    fn artist_and_title_together_find_the_track() {
        let rows = parse(
            "Artist,Title,Genre\nDaft Punk,Get Lucky,Disco\n",
            &columns(),
        )
        .unwrap();
        let lib = vec![candidate("t1", "Get Lucky", Some("Daft Punk"), None)];
        assert!(matches!(
            plan(&rows, &lib)[0].outcome,
            RowOutcome::Matched { .. }
        ));
    }

    #[test]
    fn a_location_match_ignores_separators_and_case() {
        // A CSV exported on Windows describes the same file as a library
        // indexed on macOS.
        let cols = ImportColumns {
            location: Some("Location".into()),
            fields: vec![("Genre".into(), "genre".into())],
            ..Default::default()
        };
        let rows = parse("Location,Genre\nD:\\Music\\A.mp3,House\n", &cols).unwrap();
        let lib = vec![candidate("t1", "A", None, Some("d:/music/a.mp3"))];
        assert!(matches!(
            plan(&rows, &lib)[0].outcome,
            RowOutcome::Matched { .. }
        ));
    }

    #[test]
    fn location_wins_over_artist_and_title() {
        // A path is an identity; a name is a description two mixes can share.
        let cols = ImportColumns {
            location: Some("Location".into()),
            artist: Some("Artist".into()),
            title: Some("Title".into()),
            fields: vec![("Genre".into(), "genre".into())],
        };
        let rows = parse(
            "Location,Artist,Title,Genre\n/music/a.mp3,Wrong,Wrong,House\n",
            &cols,
        )
        .unwrap();
        let lib = vec![
            candidate("by-path", "A", None, Some("/music/a.mp3")),
            candidate("by-name", "Wrong", Some("Wrong"), None),
        ];
        match &plan(&rows, &lib)[0].outcome {
            RowOutcome::Matched { track_id, .. } => assert_eq!(track_id, "by-path"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_row_that_matches_nothing_says_so() {
        let rows = parse("Artist,Title,Genre\nx,y,House\n", &columns()).unwrap();
        let lib = vec![candidate("t1", "Get Lucky", Some("Daft Punk"), None)];
        assert_eq!(plan(&rows, &lib)[0].outcome, RowOutcome::Unmatched);
    }

    #[test]
    fn two_tracks_with_the_same_name_are_ambiguous_not_arbitrary() {
        // Picking one would write the values onto the wrong track, and the
        // spreadsheet has nothing to say about which.
        let rows = parse(
            "Artist,Title,Genre\nDaft Punk,Get Lucky,Disco\n",
            &columns(),
        )
        .unwrap();
        let lib = vec![
            candidate("t1", "Get Lucky", Some("Daft Punk"), None),
            candidate("t2", "Get Lucky", Some("Daft Punk"), None),
        ];
        assert_eq!(
            plan(&rows, &lib)[0].outcome,
            RowOutcome::Ambiguous { count: 2 }
        );
    }

    #[test]
    fn a_row_whose_values_already_agree_is_not_a_change() {
        let rows = parse("Artist,Title,Genre\na,b,House\n", &columns()).unwrap();
        let mut track = candidate("t1", "b", Some("a"), None);
        track.current.insert("genre".into(), "House".into());
        assert!(matches!(
            plan(&rows, &[track])[0].outcome,
            RowOutcome::AlreadyCurrent { .. }
        ));
    }

    #[test]
    fn a_change_carries_the_value_it_replaces() {
        let rows = parse("Artist,Title,Genre\na,b,Techno\n", &columns()).unwrap();
        let mut track = candidate("t1", "b", Some("a"), None);
        track.current.insert("genre".into(), "House".into());
        match &plan(&rows, &[track])[0].outcome {
            RowOutcome::Matched { changes, .. } => {
                assert_eq!(
                    changes[0],
                    (
                        "genre".to_string(),
                        Some("House".to_string()),
                        "Techno".to_string()
                    )
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_report_accounts_for_every_row() {
        let rows = parse(
            "Artist,Title,Genre\na,b,Techno\nx,y,House\nDaft Punk,Get Lucky,Disco\n",
            &columns(),
        )
        .unwrap();
        let lib = vec![
            candidate("t1", "b", Some("a"), None),
            candidate("d1", "Get Lucky", Some("Daft Punk"), None),
            candidate("d2", "Get Lucky", Some("Daft Punk"), None),
        ];
        let r = report(&plan(&rows, &lib));
        assert_eq!(r.rows, 3);
        assert_eq!(r.matched + r.already_current + r.unmatched + r.ambiguous, 3);
        assert_eq!((r.matched, r.unmatched, r.ambiguous), (1, 1, 1));
    }
}
