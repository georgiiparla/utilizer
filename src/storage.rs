use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

use crate::{
    Result, invalid,
    protocol::{Capture, Position, valid_slug},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub id: i64,
    pub map_code: String,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingCapture {
    pub id: i64,
    pub capture: Capture,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spot {
    pub id: i64,
    pub playlist_id: i64,
    pub ordinal: i64,
    pub description: String,
    pub link: Option<String>,
    pub position: Position,
}

pub struct Store {
    connection: Connection,
}

fn position(row: &Row<'_>, start: usize) -> rusqlite::Result<Position> {
    Ok(Position {
        origin: [row.get(start)?, row.get(start + 1)?, row.get(start + 2)?],
        angles: [
            row.get(start + 3)?,
            row.get(start + 4)?,
            row.get(start + 5)?,
        ],
    })
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: u32 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        match version {
            0 => transaction.execute_batch(include_str!("../migrations/001_initial.sql"))?,
            1 => {}
            _ => {
                return Err(invalid(format!(
                    "Database version {version} is newer than this Utilizer supports"
                ))
                .into());
            }
        }
        transaction.commit()?;
        Ok(Self { connection })
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        Ok(())
    }

    pub fn create_playlist(&self, map_code: &str, slug: &str, name: &str) -> Result<Playlist> {
        if !valid_slug(map_code) || !valid_slug(slug) || name.trim().is_empty() {
            return Err(
                invalid("Playlist needs a valid map, unique slug, and nonempty name").into(),
            );
        }
        self.connection.execute(
            "INSERT INTO playlists (map_code, slug, name) VALUES (?1, ?2, ?3)",
            params![map_code, slug, name.trim()],
        )?;
        Ok(Playlist {
            id: self.connection.last_insert_rowid(),
            map_code: map_code.into(),
            slug: slug.into(),
            name: name.trim().into(),
        })
    }

    pub fn list_playlists(&self) -> Result<Vec<Playlist>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, map_code, slug, name FROM playlists ORDER BY id")?;
        Ok(statement
            .query_map([], |r| {
                Ok(Playlist {
                    id: r.get(0)?,
                    map_code: r.get(1)?,
                    slug: r.get(2)?,
                    name: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn record_capture(&self, capture: &Capture) -> Result<i64> {
        if !capture.position.is_valid()
            || capture
                .playlist_slug
                .as_deref()
                .is_some_and(|s| !valid_slug(s))
        {
            return Err(invalid("Invalid capture").into());
        }
        let [x, y, z] = capture.position.origin;
        let [pitch, yaw, roll] = capture.position.angles;
        self.connection.execute("INSERT INTO captures (playlist_slug,x,y,z,pitch,yaw,roll) VALUES (?1,?2,?3,?4,?5,?6,?7)", params![capture.playlist_slug,x,y,z,pitch,yaw,roll])?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Commit a reader batch together so a retry cannot hide or duplicate an
    /// earlier successful insert when a later capture fails.
    pub(crate) fn record_captures(&self, captures: &[Capture]) -> Result<Vec<i64>> {
        // The connection stays private to Store; nested transactions are checked
        // by SQLite, allowing the existing synchronous &self recording API.
        let transaction = self.connection.unchecked_transaction()?;
        let ids = captures
            .iter()
            .map(|capture| self.record_capture(capture))
            .collect::<Result<Vec<_>>>()?;
        transaction.commit()?;
        Ok(ids)
    }

    pub fn list_captures(&self) -> Result<Vec<PendingCapture>> {
        let mut statement = self.connection.prepare("SELECT id,playlist_slug,x,y,z,pitch,yaw,roll FROM captures WHERE accepted=0 ORDER BY id")?;
        Ok(statement
            .query_map([], |r| {
                Ok(PendingCapture {
                    id: r.get(0)?,
                    capture: Capture {
                        playlist_slug: r.get(1)?,
                        position: position(r, 2)?,
                    },
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Returns the existing spot on repeated acceptance into the same playlist.
    /// A retry cannot move an already accepted capture to a different playlist.
    pub fn accept_capture(
        &mut self,
        capture_id: i64,
        playlist_id: i64,
        description: &str,
        link: Option<&str>,
    ) -> Result<i64> {
        if description.trim().is_empty() {
            return Err(invalid("A spot needs a description").into());
        }
        if let Some(link) = link
            && !(link.starts_with("https://") || link.starts_with("http://"))
        {
            return Err(invalid("Reference links must use http or https").into());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((id, existing_playlist)) = transaction
            .query_row(
                "SELECT id, playlist_id FROM spots WHERE capture_id=?1",
                [capture_id],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )
            .optional()?
        {
            if existing_playlist != playlist_id {
                return Err(invalid("Capture already accepted into another playlist").into());
            }
            return Ok(id);
        }
        let count = transaction.execute("INSERT INTO spots (playlist_id,capture_id,ordinal,description,link,x,y,z,pitch,yaw,roll) SELECT ?1,id,(SELECT COALESCE(MAX(ordinal),0)+1 FROM spots WHERE playlist_id=?1),?3,?4,x,y,z,pitch,yaw,roll FROM captures WHERE id=?2 AND accepted=0", params![playlist_id,capture_id,description.trim(),link])?;
        if count != 1 {
            return Err(invalid("Pending capture does not exist").into());
        }
        let id = transaction.last_insert_rowid();
        transaction.execute("UPDATE captures SET accepted=1 WHERE id=?1", [capture_id])?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn load_spots(&self, playlist_id: i64) -> Result<Vec<Spot>> {
        let mut statement = self.connection.prepare("SELECT id,playlist_id,ordinal,description,link,x,y,z,pitch,yaw,roll FROM spots WHERE playlist_id=?1 ORDER BY ordinal")?;
        Ok(statement
            .query_map([playlist_id], |r| {
                Ok(Spot {
                    id: r.get(0)?,
                    playlist_id: r.get(1)?,
                    ordinal: r.get(2)?,
                    description: r.get(3)?,
                    link: r.get(4)?,
                    position: position(r, 5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }
}
