-- Reference data
INSERT INTO djmdArtist (ID, Name) VALUES (1, 'Artist One'), (2, 'Artist Two');
INSERT INTO djmdAlbum  (ID, Name) VALUES (1, 'Album One'), (2, 'Album Two');
INSERT INTO djmdGenre  (ID, Name) VALUES (1, 'Techno'), (2, 'House');
INSERT INTO djmdKey    (ID, ScaleName, Seq) VALUES (1, '8A', 1), (2, '11B', 2);

-- Tracks: BPM stored as integer × 100
INSERT INTO djmdLabel (ID, Name) VALUES
    ('lb-1', 'Drumcode'),
    ('lb-2', 'Hessle Audio');

-- Colour names live in `Commnt`; `Name` is left NULL on one row so the
-- reader's COALESCE fallback is exercised in both directions.
INSERT INTO djmdColor (ID, ColorCode, SortKey, Commnt, Name) VALUES
    ('col-1', 1, 1, 'Red',  NULL),
    ('col-2', 2, 2, NULL,   'Blue');

INSERT INTO djmdContent
    (ID, Title, ArtistID, AlbumID, GenreID, KeyID, BPM, Length, Rating, Commnt,
     FolderPath, AnalysisDataPath, DateCreated, LabelID, RemixerID, Subtitle,
     ColorID, rb_local_deleted)
VALUES
    (1, 'Test Track Alpha', 1, 1, 1, 1, 13200, 360, 4, 'alpha comment',
     '/music/alpha.mp3', '/PIONEER/USBANLZ/aa/alpha/ANLZ0000.DAT', '2025-01-01T00:00:00Z',
     'lb-1', 2, 'Extended Mix', 'col-1', 0),
    (2, 'Test Track Beta',  1, 2, 2, 2, 12800, 240, 3, 'beta comment',
     '/music/beta.mp3',  '/PIONEER/USBANLZ/bb/beta/ANLZ0000.DAT', '2025-06-01T00:00:00Z',
     'lb-2', NULL, NULL, 'col-2', 0),
    (3, 'Test Track Gamma', 2, 1, 1, 1, 14000, 420, 5, NULL,
     '/music/gamma.mp3', NULL, '2026-05-19T00:00:00Z',
     NULL, NULL, NULL, NULL, 0),
    (4, 'Deleted Track',    1, 1, 1, 1, 12800, 300, 0, NULL,
     '/music/del.mp3',   NULL, '2026-05-19T00:00:00Z',
     NULL, NULL, NULL, NULL, 1);

-- Playlists
INSERT INTO djmdPlaylist (ID, Seq, Name, Attribute, ParentID) VALUES
    (1, 1, 'Root Folder',   1, NULL),
    (2, 1, 'Techno Set',    0, 1),
    (3, 2, 'House Vibes',   0, 1);

INSERT INTO djmdSongPlaylist (ID, PlaylistID, ContentID, TrackNo) VALUES
    (1, 2, 1, 1),
    (2, 2, 2, 2),
    (3, 3, 3, 1);

-- Cues: Kind 0 = memory cue, Kind 1 = hot cue slot 1
INSERT INTO djmdCue (ID, ContentID, InMsec, OutMsec, Kind, Color, Commnt) VALUES
    (1, 1,  4000, NULL, 0, -1, 'Intro'),
    (2, 1, 32000, NULL, 1,  1, 'Drop'),
    (3, 2, 16000, NULL, 1,  2, 'Build');

-- History: two sessions. The second replays a track from the first, which is
-- what makes the snapshot rule worth testing.
INSERT INTO djmdHistory (ID, Seq, Name, Attribute, ParentID, DateCreated, rb_local_deleted) VALUES
    ('h1', 1, '2026-05-01 Basement', 0, NULL, '2026-05-01T22:00:00Z', 0),
    ('h2', 2, '2026-06-12 Rooftop',  0, NULL, '2026-06-12T21:30:00Z', 0),
    ('h3', 3, 'Deleted Session',     0, NULL, '2026-06-13T21:30:00Z', 1);

INSERT INTO djmdSongHistory (ID, HistoryID, ContentID, TrackNo) VALUES
    ('sh1', 'h1', '1', 1),
    ('sh2', 'h1', '2', 2),
    ('sh3', 'h2', '1', 1),
    ('sh4', 'h2', '3', 2),
    ('sh5', 'h3', '1', 1);

-- MyTags: two categories, four tags, one deleted tag and one deleted link.
INSERT INTO djmdMyTag (ID, Seq, Name, Attribute, ParentID, rb_local_deleted) VALUES
    ('mt-genre',  1, 'Genre',      0, 'root',     0),
    ('mt-vocals', 2, 'Vocals',     0, 'root',     0),
    ('mt-techno', 1, 'Techno',     1, 'mt-genre', 0),
    ('mt-house',  2, 'House',      1, 'mt-genre', 0),
    ('mt-novox',  1, 'No Vocals',  1, 'mt-vocals', 0),
    ('mt-gone',   3, 'Deleted Tag',1, 'mt-genre', 1);

INSERT INTO djmdSongMyTag (ID, MyTagID, ContentID, rb_local_deleted) VALUES
    ('smt1', 'mt-techno', '1', 0),
    ('smt2', 'mt-novox',  '1', 0),
    ('smt3', 'mt-house',  '2', 0),
    ('smt4', 'mt-gone',   '2', 0),
    ('smt5', 'mt-techno', '3', 1);
