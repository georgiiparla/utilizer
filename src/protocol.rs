//! Incremental, bounded parser for the existing Utilizer console protocol.

use std::time::{Duration, Instant};

pub const CAPTURE_MARKER: &str = "[UTILIZER_CAPTURE]";
pub const PLAYLIST_MARKER: &str = "[UTILIZER_PLAYLIST]";
pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub origin: [f64; 3],
    pub angles: [f64; 3],
}

impl Position {
    pub fn is_valid(&self) -> bool {
        self.origin
            .iter()
            .chain(&self.angles)
            .all(|n| n.is_finite())
    }

    /// Accept only a complete pair of commands, never arbitrary cfg statements.
    pub fn parse(text: &str) -> Option<Self> {
        fn triple(text: &str, command: &str) -> Option<[f64; 3]> {
            let mut words = text.split_ascii_whitespace();
            if words.next()? != command {
                return None;
            }
            let values = [
                words.next()?.parse().ok()?,
                words.next()?.parse().ok()?,
                words.next()?.parse().ok()?,
            ];
            words.next().is_none().then_some(values)
        }
        let (origin, angles) = text.split_once(';')?;
        let position = Self {
            origin: triple(origin, "setpos")?,
            angles: triple(angles, "setang")?,
        };
        position.is_valid().then_some(position)
    }

    pub fn command(&self) -> String {
        let [x, y, z] = self.origin;
        let [pitch, yaw, roll] = self.angles;
        format!("setpos {x} {y} {z};setang {pitch} {yaw} {roll}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    pub playlist_slug: Option<String>,
    pub position: Position,
}

#[derive(Default)]
pub struct Parser {
    line: Vec<u8>,
    discarding: bool,
    selected_playlist: Option<String>,
    pending: Option<(Instant, Option<String>)>,
    active_map: Option<String>,
    capture_maps: Vec<Option<String>>,
}

impl Parser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_map(&self) -> Option<&str> {
        self.active_map.as_deref()
    }

    pub(crate) fn take_capture_maps(&mut self) -> Vec<Option<String>> {
        std::mem::take(&mut self.capture_maps)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn expire(&mut self, now: Instant) {
        if self
            .pending
            .as_ref()
            .is_some_and(|(since, _)| now.saturating_duration_since(*since) >= CAPTURE_TIMEOUT)
        {
            self.pending = None;
        }
    }

    /// `now` is injected so timeout tests never have to sleep.
    pub fn feed(&mut self, bytes: &[u8], now: Instant) -> Vec<Capture> {
        self.expire(now);
        self.capture_maps.clear();
        let mut captures = Vec::new();
        for &byte in bytes {
            if byte == b'\n' {
                if !self.discarding {
                    let line = std::mem::take(&mut self.line);
                    // Invalid UTF-8 is ignored, never repaired into a protocol message.
                    if let Ok(text) = std::str::from_utf8(&line)
                        && let Some(capture) = self.process_line(text.trim_end_matches('\r'), now)
                    {
                        self.capture_maps.push(self.active_map.clone());
                        captures.push(capture);
                    }
                }
                self.line.clear();
                self.discarding = false;
            } else if !self.discarding {
                if self.line.len() == MAX_LINE_BYTES {
                    self.line.clear();
                    self.discarding = true;
                    self.pending = None;
                } else {
                    self.line.push(byte);
                }
            }
        }
        captures
    }

    fn process_line(&mut self, line: &str, now: Instant) -> Option<Capture> {
        let text = payload(line);
        if let Some(map) = text
            .strip_prefix("[Client] Map: \"")
            .and_then(|s| s.strip_suffix('"'))
        {
            self.active_map = crate::maps::find(map).map(|m| m.code.to_owned());
            self.selected_playlist = None;
            self.pending = None;
            return None;
        }
        if text.starts_with("Loading map \"")
            || text.starts_with("[Client] Disconnecting from server:")
        {
            self.active_map = None;
            self.selected_playlist = None;
            self.pending = None;
            return None;
        }
        if let Some(slug) = text.strip_prefix(PLAYLIST_MARKER)
            && let Some(slug) = slug.strip_prefix(' ')
        {
            self.pending = None;
            self.selected_playlist = valid_slug(slug).then(|| slug.to_owned());
            return None;
        }
        if text == CAPTURE_MARKER {
            self.pending = Some((now, self.selected_playlist.clone()));
            return None;
        }
        self.pending.as_ref()?;
        let position = Position::parse(text)?;
        let (_, playlist_slug) = self.pending.take()?;
        Some(Capture {
            playlist_slug,
            position,
        })
    }
}

pub fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 128
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Strip only the observed `MM/DD HH:MM:SS ` envelope. Do not trim arbitrary
/// channel prefixes: a chat message containing our marker must remain noise.
fn payload(line: &str) -> &str {
    let b = line.as_bytes();
    if b.len() < 15
        || b[2] != b'/'
        || b[5] != b' '
        || b[8] != b':'
        || b[11] != b':'
        || b[14] != b' '
    {
        return line;
    }
    if ![0, 1, 3, 4, 6, 7, 9, 10, 12, 13]
        .into_iter()
        .all(|i| b[i].is_ascii_digit())
    {
        return line;
    }
    let number = |i| (b[i] - b'0') * 10 + b[i + 1] - b'0';
    if !(1..=12).contains(&number(0))
        || !(1..=31).contains(&number(3))
        || number(6) > 23
        || number(9) > 59
        || number(12) > 59
    {
        return line;
    }
    &line[15..]
}
