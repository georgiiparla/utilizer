CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE playlists (
    id INTEGER PRIMARY KEY,
    map_code TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL
);
CREATE TABLE captures (
    id INTEGER PRIMARY KEY,
    playlist_slug TEXT,
    x REAL NOT NULL, y REAL NOT NULL, z REAL NOT NULL,
    pitch REAL NOT NULL, yaw REAL NOT NULL, roll REAL NOT NULL,
    accepted INTEGER NOT NULL DEFAULT 0 CHECK (accepted IN (0, 1))
);
CREATE TABLE spots (
    id INTEGER PRIMARY KEY,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id),
    capture_id INTEGER NOT NULL UNIQUE REFERENCES captures(id),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 1),
    description TEXT NOT NULL,
    link TEXT,
    x REAL NOT NULL, y REAL NOT NULL, z REAL NOT NULL,
    pitch REAL NOT NULL, yaw REAL NOT NULL, roll REAL NOT NULL,
    UNIQUE (playlist_id, ordinal)
);
PRAGMA user_version = 1;
