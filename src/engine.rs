use std::{path::Path, time::Instant};

use crate::{Result, log::LogMonitor, protocol::Parser, storage::Store};

/// One synchronous step used by the executable and integration tests alike.
/// Keep filesystem/parser state here so HTTP handlers need no ingestion logic.
pub struct Engine {
    pub monitor: LogMonitor,
    parser: Parser,
    // Retain captures if SQLite is temporarily unavailable. Do not advance the
    // reader again until these writes succeed.
    unsaved: Vec<crate::protocol::Capture>,
}

impl Engine {
    pub fn start(log_path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            monitor: LogMonitor::start(log_path)?,
            parser: Parser::new(),
            unsaved: Default::default(),
        })
    }

    pub fn step(&mut self, store: &Store, now: Instant) -> Result<Vec<i64>> {
        if !self.unsaved.is_empty() {
            let ids = self.flush(store)?;
            // Report the committed retry before a later read can fail. Schedule
            // another step for bytes whose notification was consumed by retrying.
            self.monitor.wake_signal().notify_one();
            return Ok(ids);
        }
        let batch = self.monitor.poll()?;
        if batch.reset {
            self.parser.reset();
        }
        self.unsaved = self.parser.feed(&batch.bytes, now);
        self.flush(store)
    }

    fn flush(&mut self, store: &Store) -> Result<Vec<i64>> {
        if self.unsaved.is_empty() {
            return Ok(Vec::new());
        }
        let ids = store.record_captures(&self.unsaved)?;
        self.unsaved.clear();
        Ok(ids)
    }
}
