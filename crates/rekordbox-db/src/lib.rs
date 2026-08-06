//! Read-only SQLCipher access to the Rekordbox 7 `master.db`.
//!
//! Vendored and adapted from reklawdbox `src/db.rs` (MIT, Ryan Voitiskis).
//!
//! # Quick start
//!
//! ```no_run
//! use rekordbox_db::RekordboxDb;
//! use std::path::Path;
//!
//! let db = RekordboxDb::open(Path::new("/path/to/master.db")).unwrap();
//! let tracks = db.tracks().unwrap();
//! println!("{} tracks", tracks.len());
//! ```

pub mod anlz;
mod connection;
pub mod quantize;
mod queries;
pub mod types;
pub mod write;

pub use anlz::{DetailPoint, PreviewPoint, WaveformColor};
pub use connection::RekordboxDb;
pub use types::{
    ArtistCount, BeatGridEntry, BrokenMetadataReport, CueKind, DuplicateGroup, DuplicateKind,
    GenreCount, HotCue, LibraryAnalytics, Playlist, PlaylistEntry, PlaylistKind, Track,
};
pub use write::{WriteError, WriteGuard, WriteSession};
