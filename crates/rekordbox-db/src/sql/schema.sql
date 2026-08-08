CREATE TABLE IF NOT EXISTS djmdArtist (
    ID      TEXT PRIMARY KEY,
    Name    TEXT,
    SearchStr TEXT
);

CREATE TABLE IF NOT EXISTS djmdAlbum (
    ID              TEXT PRIMARY KEY,
    Name            TEXT,
    AlbumArtistID   TEXT,
    SearchStr       TEXT
);

CREATE TABLE IF NOT EXISTS djmdGenre (
    ID   TEXT PRIMARY KEY,
    Name TEXT
);

CREATE TABLE IF NOT EXISTS djmdKey (
    ID        TEXT PRIMARY KEY,
    ScaleName TEXT,
    Seq       INTEGER
);

CREATE TABLE IF NOT EXISTS djmdContent (
    ID                 TEXT PRIMARY KEY,
    Title              TEXT,
    ArtistID           TEXT,
    AlbumID            TEXT,
    GenreID            TEXT,
    KeyID              TEXT,
    BPM                INTEGER,
    Length             INTEGER,
    Rating             INTEGER,
    Commnt             TEXT,
    FolderPath         TEXT,
    AnalysisDataPath   TEXT,
    FileType           INTEGER,
    SampleRate         INTEGER,
    BitRate            INTEGER,
    ReleaseYear        INTEGER,
    DJPlayCount        INTEGER,
    DateCreated        TEXT,
    LabelID            TEXT,
    RemixerID          TEXT,
    Subtitle           TEXT,
    ColorID            TEXT,
    rb_local_deleted   INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS djmdLabel (
    ID   TEXT PRIMARY KEY,
    Name TEXT
);

-- Rekordbox keeps the human-readable colour name in `Commnt`, not `Name`.
-- Both are declared here so the reader's COALESCE is exercised against the
-- real shape rather than a tidied-up one.
CREATE TABLE IF NOT EXISTS djmdColor (
    ID        TEXT PRIMARY KEY,
    ColorCode INTEGER,
    SortKey   INTEGER,
    Commnt    TEXT,
    Name      TEXT
);

CREATE TABLE IF NOT EXISTS djmdPlaylist (
    ID        TEXT PRIMARY KEY,
    Seq       INTEGER,
    Name      TEXT,
    Attribute INTEGER DEFAULT 0,
    ParentID  TEXT
);

CREATE TABLE IF NOT EXISTS djmdSongPlaylist (
    ID         TEXT PRIMARY KEY,
    PlaylistID TEXT,
    ContentID  TEXT,
    TrackNo    INTEGER
);

CREATE TABLE IF NOT EXISTS djmdCue (
    ID        TEXT PRIMARY KEY,
    ContentID TEXT,
    InMsec    INTEGER,
    OutMsec   INTEGER,
    Kind      INTEGER DEFAULT 0,
    Color     INTEGER DEFAULT -1,
    Commnt    TEXT
);

-- Play history. Rekordbox logs a "set" per session in djmdHistory, with its
-- tracks in djmdSongHistory — the same shape as playlists, keyed by date.
CREATE TABLE IF NOT EXISTS djmdHistory (
    ID          TEXT PRIMARY KEY,
    Seq         INTEGER,
    Name        TEXT,
    Attribute   INTEGER DEFAULT 0,
    ParentID    TEXT,
    DateCreated TEXT,
    rb_local_deleted INTEGER DEFAULT 0
);

-- MyTags. Rekordbox stores categories and tags in the *same* table, with a
-- category being a row whose ParentID is the root and a tag being one whose
-- ParentID is a category — the same self-referencing shape as playlists and
-- folders. Attribute distinguishes them: 0 = category, 1 = tag.
CREATE TABLE IF NOT EXISTS djmdMyTag (
    ID          TEXT PRIMARY KEY,
    Seq         INTEGER,
    Name        TEXT,
    Attribute   INTEGER DEFAULT 0,
    ParentID    TEXT,
    rb_local_deleted INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS djmdSongMyTag (
    ID        TEXT PRIMARY KEY,
    MyTagID   TEXT,
    ContentID TEXT,
    rb_local_deleted INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS djmdSongHistory (
    ID        TEXT PRIMARY KEY,
    HistoryID TEXT,
    ContentID TEXT,
    TrackNo   INTEGER
);
