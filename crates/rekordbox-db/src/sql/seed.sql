-- Reference data
INSERT INTO djmdArtist (ID, Name) VALUES (1, 'Artist One'), (2, 'Artist Two');
INSERT INTO djmdAlbum  (ID, Name) VALUES (1, 'Album One'), (2, 'Album Two');
INSERT INTO djmdGenre  (ID, Name) VALUES (1, 'Techno'), (2, 'House');
INSERT INTO djmdKey    (ID, ScaleName, Seq) VALUES (1, '8A', 1), (2, '11B', 2);

-- Tracks: BPM stored as integer × 100
INSERT INTO djmdContent
    (ID, Title, ArtistID, AlbumID, GenreID, KeyID, BPM, Length, Rating, Commnt,
     FolderPath, AnalysisDataPath, DateCreated, rb_local_deleted)
VALUES
    (1, 'Test Track Alpha', 1, 1, 1, 1, 13200, 360, 4, 'alpha comment',
     '/music/alpha.mp3', '/PIONEER/USBANLZ/aa/alpha/ANLZ0000.DAT', '2025-01-01T00:00:00Z', 0),
    (2, 'Test Track Beta',  1, 2, 2, 2, 12800, 240, 3, 'beta comment',
     '/music/beta.mp3',  '/PIONEER/USBANLZ/bb/beta/ANLZ0000.DAT', '2025-06-01T00:00:00Z', 0),
    (3, 'Test Track Gamma', 2, 1, 1, 1, 14000, 420, 5, NULL,
     '/music/gamma.mp3', NULL, '2026-05-19T00:00:00Z', 0),
    (4, 'Deleted Track',    1, 1, 1, 1, 12800, 300, 0, NULL,
     '/music/del.mp3',   NULL, '2026-05-19T00:00:00Z', 1);

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
