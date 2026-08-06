use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum ToolRequest {
    LibrarySearch {
        library_path: String,
        query: String,
        limit: Option<usize>,
    },
    LibraryBulkAddIntroCues {
        library_path: String,
        track_ids: Vec<String>,
    },
    LibraryGetTrack {
        library_path: String,
        id: String,
    },
    LibraryListPlaylists {
        library_path: String,
    },
    LibraryGetPlaylist {
        library_path: String,
        id: String,
    },
    LibraryListCues {
        library_path: String,
        track_id: String,
    },
    HealthOrphanScan {
        library_path: String,
    },
    HealthDuplicateScan {
        library_path: String,
    },
    HealthFuzzyDuplicateScan {
        library_path: String,
    },
    HealthBrokenLinkScan {
        library_path: String,
    },
    StagingListChanges {
        library_path: Option<String>,
    },
    ExportAcceptedChanges {
        library_path: String,
        output_path: String,
    },
    LibraryReadFileTags {
        library_path: String,
        track_id: String,
    },
    LibraryAnalyzeTrack {
        library_path: String,
        track_id: String,
    },
    LibraryScanAndProposeMissing {
        library_path: String,
        #[serde(default)]
        fields: Vec<String>,
        limit: Option<usize>,
    },
    SmartlistList {
        library_path: String,
    },
    SmartlistEvaluate {
        library_path: String,
        id: String,
    },
    /// Scan for files that do not decode — the check the existing
    /// `health_broken_link_scan` does not do. `full` decodes every file and
    /// costs about what analysing it costs; `header` is fast and misses
    /// corruption late in a file.
    HealthPlayableScan {
        library_path: String,
        #[serde(default)]
        depth: Option<String>,
    },
    /// Sync runs recorded for a library, newest first, with how much of each
    /// can be put back.
    UndoList {
        library_path: String,
    },
    /// What one run did, and the reason for each change that cannot be undone.
    UndoEntries {
        run_id: String,
    },
    /// Stage a run's inverses. They land as *proposed* changes and still go
    /// through review and Sync — this never writes to `master.db`.
    UndoRun {
        library_path: String,
        run_id: String,
    },
    RelocateScan {
        library_path: String,
        #[serde(default)]
        search_roots: Vec<String>,
    },
    RelocateApply {
        library_path: String,
        track_id: String,
        new_path: String,
    },
}
