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
    rb_local_deleted   INTEGER DEFAULT 0
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

CREATE TABLE IF NOT EXISTS djmdSongHistory (
    ID        TEXT PRIMARY KEY,
    HistoryID TEXT,
    ContentID TEXT,
    TrackNo   INTEGER
);
