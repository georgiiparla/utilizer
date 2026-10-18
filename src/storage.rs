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

#[derive(Debug, Clone, PartialEq)]
pub struct MapSpot {
    pub id: i64,
    pub map_code: String,
    pub ordinal: i64,
    pub description: String,
    pub link: Option<String>,
    pub position: Position,
}

fn map_spot(row: &Row<'_>) -> rusqlite::Result<MapSpot> {
    Ok(MapSpot {
        id: row.get(0)?,
        map_code: row.get(1)?,
        ordinal: row.get(2)?,
        description: row.get(3)?,
        link: row.get(4)?,
        position: position(row, 5)?,
    })
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
            0 => transaction.execute_batch(include_str!("schema.sql"))?,
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
        validate_name(name, 80)?;
        if !valid_slug(map_code) || !valid_slug(slug) {
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
    pub(crate) fn record_captures(
        &self,
        captures: &[Capture],
        maps: &[Option<String>],
    ) -> Result<Vec<i64>> {
        // The connection stays private to Store; nested transactions are checked
        // by SQLite, allowing the existing synchronous &self recording API.
        let transaction = self.connection.unchecked_transaction()?;
        let ids = captures
            .iter()
            .enumerate()
            .map(|(index, capture)| {
                let id = self.record_capture(capture)?;
                if let Some(map) = maps.get(index).and_then(Option::as_deref) {
                    self.set_setting(&format!("capture_map_{id}"), map)?;
                }
                Ok(id)
            })
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

    pub fn discard_capture(&self, id: i64) -> Result<()> {
        // Keep the consumed row so SQLite cannot reuse its ID for a new capture.
        // An old form in another browser tab must never accept a different capture.
        // The existing accepted flag means the capture is no longer pending;
        // only a corresponding spot represents acceptance into a playlist.
        self.connection.execute(
            "UPDATE captures SET accepted=1 WHERE id=?1 AND accepted=0",
            [id],
        )?;
        Ok(())
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

    pub fn map_spots(&self, map: &str) -> Result<Vec<MapSpot>> {
        let mut statement = self.connection.prepare("SELECT id,map_code,ordinal,description,link,x,y,z,pitch,yaw,roll FROM map_spots WHERE map_code=?1 ORDER BY ordinal")?;
        Ok(statement
            .query_map([map], map_spot)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn map_spot(&self, id: i64) -> Result<MapSpot> {
        self.connection.query_row("SELECT id,map_code,ordinal,description,link,x,y,z,pitch,yaw,roll FROM map_spots WHERE id=?1", [id], map_spot)
            .optional()?.ok_or_else(|| invalid("Spot no longer exists").into())
    }

    pub fn save_map_spot(
        &self,
        id: Option<i64>,
        map: &str,
        name: &str,
        link: Option<&str>,
        position: Position,
    ) -> Result<i64> {
        validate_map_spot(map, name, link, position)?;
        let [x, y, z] = position.origin;
        let [pitch, yaw, roll] = position.angles;
        if let Some(id) = id {
            if self.connection.execute("UPDATE map_spots SET description=?3,link=?4,x=?5,y=?6,z=?7,pitch=?8,yaw=?9,roll=?10 WHERE id=?1 AND map_code=?2", params![id,map,name.trim(),link,x,y,z,pitch,yaw,roll])? != 1 {
                return Err(invalid("Spot no longer exists on this map").into());
            }
            return Ok(id);
        }
        self.connection.execute("INSERT INTO map_spots(map_code,ordinal,description,link,x,y,z,pitch,yaw,roll) VALUES (?1,(SELECT COALESCE(MAX(ordinal),0)+1 FROM map_spots WHERE map_code=?1),?2,?3,?4,?5,?6,?7,?8,?9)", params![map,name.trim(),link,x,y,z,pitch,yaw,roll])?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn delete_map_spot(&self, id: i64, map: &str) -> Result<()> {
        if self.connection.execute(
            "DELETE FROM map_spots WHERE id=?1 AND map_code=?2",
            params![id, map],
        )? != 1
        {
            return Err(invalid("Spot no longer exists on this map").into());
        }
        Ok(())
    }

    pub fn move_map_spot(&mut self, id: i64, map: &str, earlier: bool) -> Result<()> {
        let spot = self.map_spot(id)?;
        if spot.map_code != map {
            return Err(invalid("Spot belongs to another map").into());
        }
        let spots = self.map_spots(map)?;
        let neighbor = if earlier {
            spots.iter().rev().find(|s| s.ordinal < spot.ordinal)
        } else {
            spots.iter().find(|s| s.ordinal > spot.ordinal)
        };
        let Some(neighbor) = neighbor else {
            return Ok(());
        };
        let spare = spots
            .last()
            .and_then(|s| s.ordinal.checked_add(1))
            .ok_or_else(|| invalid("Spot order out of range"))?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE map_spots SET ordinal=?2 WHERE id=?1",
            params![id, spare],
        )?;
        transaction.execute(
            "UPDATE map_spots SET ordinal=?2 WHERE id=?1",
            params![neighbor.id, spot.ordinal],
        )?;
        transaction.execute(
            "UPDATE map_spots SET ordinal=?2 WHERE id=?1",
            params![id, neighbor.ordinal],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn capture_map(&self, id: i64) -> Result<Option<String>> {
        if let Some(map) = self.setting(&format!("capture_map_{id}"))? {
            return Ok(Some(map));
        }
        let capture = self
            .list_captures()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| invalid("Capture no longer pending"))?;
        Ok(capture
            .capture
            .playlist_slug
            .as_deref()
            .and_then(crate::maps::find)
            .map(|m| m.code.to_owned()))
    }

    pub fn accept_map_capture(
        &mut self,
        id: i64,
        map: &str,
        name: &str,
        link: Option<&str>,
        position: Position,
    ) -> Result<i64> {
        validate_map_spot(map, name, link, position)?;
        if self.capture_map(id)?.as_deref().is_some_and(|m| m != map) {
            return Err(invalid("Capture belongs to another map").into());
        }
        let transaction = self.connection.transaction()?;
        let [x, y, z] = position.origin;
        let [pitch, yaw, roll] = position.angles;
        if transaction.execute("INSERT INTO map_spots(map_code,ordinal,description,link,x,y,z,pitch,yaw,roll,capture_id) SELECT ?2,(SELECT COALESCE(MAX(ordinal),0)+1 FROM map_spots WHERE map_code=?2),?3,?4,?5,?6,?7,?8,?9,?10,id FROM captures WHERE id=?1 AND accepted=0",params![id,map,name.trim(),link,x,y,z,pitch,yaw,roll])? != 1 {
            return Err(invalid("Capture no longer pending").into());
        }
        let spot = transaction.last_insert_rowid();
        transaction.execute("UPDATE captures SET accepted=1 WHERE id=?1", [id])?;
        transaction.commit()?;
        Ok(spot)
    }
}

fn validate_map_spot(map: &str, name: &str, link: Option<&str>, position: Position) -> Result<()> {
    validate_spot(name, link, position)?;
    if crate::maps::find(map).is_none_or(|m| m.calibration.project(position.origin).is_none()) {
        return Err(invalid("Position is outside this supported map").into());
    }
    Ok(())
}

pub fn validate_name(name: &str, limit: usize) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > limit || name.chars().any(char::is_control)
    {
        return Err(invalid(format!(
            "Use a name of 1–{limit} characters without control characters"
        ))
        .into());
    }
    Ok(())
}

pub fn validate_spot(name: &str, link: Option<&str>, position: Position) -> Result<()> {
    validate_name(name, 160)?;
    if !position.is_valid()
        || position.origin.iter().any(|n| n.abs() > 1_000_000.0)
        || position.angles.iter().any(|n| n.abs() > 3600.0)
    {
        return Err(invalid("Position or angles are outside the supported range").into());
    }
    if let Some(link) = link {
        let authority = link
            .strip_prefix("https://")
            .or_else(|| link.strip_prefix("http://"))
            .and_then(|s| s.split(['/', '?', '#']).next());
        if link.len() > 2048
            || link.chars().any(|c| c.is_whitespace() || c.is_control())
            || authority.is_none_or(|s| s.is_empty() || s.contains(['\\', '@']))
        {
            return Err(invalid("Use an http/https reference URL of at most 2048 bytes").into());
        }
    }
    Ok(())
}
