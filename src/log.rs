//! Filesystem events are hints; the reader's byte cursor is the source of truth.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use same_file::Handle;
use tokio::sync::Notify;

use crate::{Result, invalid};

const CHUNK_BYTES: usize = 256 * 1024;
const ANCHOR_BYTES: usize = 64;

pub struct ReadBatch {
    pub reset: bool,
    pub bytes: Vec<u8>,
    pub more: bool,
}

pub struct TailReader {
    path: PathBuf,
    identity: Option<Handle>,
    offset: u64,
    anchor: Vec<u8>,
    skip_continuation: bool,
}

impl TailReader {
    /// Attach at EOF. A file created later is read from its beginning.
    pub fn attach(path: impl AsRef<Path>) -> Result<Self> {
        let mut reader = Self {
            path: path.as_ref().into(),
            identity: None,
            offset: 0,
            anchor: Vec::new(),
            skip_continuation: false,
        };
        match File::open(&reader.path) {
            Ok(mut file) => {
                reader.offset = file.metadata()?.len();
                reader.anchor = read_anchor(&mut file, reader.offset)?;
                reader.skip_continuation =
                    reader.offset > 0 && reader.anchor.last() != Some(&b'\n');
                reader.identity = Some(Handle::from_file(file)?);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(reader)
    }

    pub fn poll(&mut self) -> Result<ReadBatch> {
        let mut file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let reset = self.identity.take().is_some();
                self.offset = 0;
                self.anchor.clear();
                self.skip_continuation = false;
                return Ok(ReadBatch {
                    reset,
                    bytes: Vec::new(),
                    more: false,
                });
            }
            // Keep the cursor intact on temporary sharing/permission errors.
            Err(error) => return Err(error.into()),
        };
        let identity = Handle::from_file(file.try_clone()?)?;
        let length = file.metadata()?.len();
        let mut reset = self.identity.as_ref() != Some(&identity) || length < self.offset;
        // Detect a truncate-and-regrow even when the new length exceeds the old
        // cursor. The retained handle also prevents replacement-ID reuse.
        if !reset && read_anchor(&mut file, self.offset)? != self.anchor {
            reset = true;
        }
        let start = if reset { 0 } else { self.offset };
        file.seek(SeekFrom::Start(start))?;
        let count = (length - start).min(CHUNK_BYTES as u64);
        let mut bytes = Vec::new();
        (&mut file).take(count).read_to_end(&mut bytes)?;
        let offset = start + bytes.len() as u64;
        // Derive the anchor from the bytes actually read, not from a second
        // potentially racing read of the file.
        let mut anchor = if reset {
            Vec::new()
        } else {
            self.anchor.clone()
        };
        anchor.extend_from_slice(&bytes);
        if anchor.len() > ANCHOR_BYTES {
            anchor.drain(..anchor.len() - ANCHOR_BYTES);
        }
        self.offset = offset;
        self.identity = Some(identity);
        self.anchor = anchor;
        if reset {
            self.skip_continuation = false;
        }
        // If we attached halfway through an existing line, its suffix is not a
        // new complete protocol message. Wait for the next line boundary.
        if self.skip_continuation {
            if let Some(end) = bytes.iter().position(|b| *b == b'\n') {
                bytes.drain(..=end);
                self.skip_continuation = false;
            } else {
                bytes.clear();
            }
        }
        Ok(ReadBatch {
            reset,
            bytes,
            more: offset < length,
        })
    }
}

fn read_anchor(file: &mut File, offset: u64) -> std::io::Result<Vec<u8>> {
    let start = offset.saturating_sub(ANCHOR_BYTES as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = vec![0; (offset - start) as usize];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub struct LogMonitor {
    _watcher: RecommendedWatcher,
    reader: TailReader,
    wake: Arc<Notify>,
    watcher_error: Arc<Mutex<Option<String>>>,
}

impl LogMonitor {
    pub fn start(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let parent = path
            .parent()
            .ok_or_else(|| invalid("Log has no parent directory"))?
            .canonicalize()?;
        let name = path
            .file_name()
            .ok_or_else(|| invalid("Log has no filename"))?
            .to_owned();
        let path = parent.join(&name);
        let wake = Arc::new(Notify::new());
        let watcher_error = Arc::new(Mutex::new(None));
        let callback_wake = wake.clone();
        let callback_error = watcher_error.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                match event {
                    Ok(event) => {
                        if matches!(event.kind, EventKind::Access(_)) {
                            return;
                        }
                        let relevant = event.need_rescan()
                            || event.paths.is_empty()
                            || event.paths.iter().any(|p| {
                                p.file_name().is_some_and(|n| {
                                    if cfg!(windows) {
                                        n.to_string_lossy()
                                            .eq_ignore_ascii_case(&name.to_string_lossy())
                                    } else {
                                        n == name
                                    }
                                })
                            });
                        if relevant {
                            // Notify retains one permit; bursts cannot grow a queue.
                            callback_wake.notify_one();
                        }
                    }
                    Err(error) => {
                        if let Ok(mut slot) = callback_error.lock() {
                            *slot = Some(error.to_string());
                        }
                        callback_wake.notify_one();
                    }
                }
            })?;
        watcher.watch(&parent, RecursiveMode::NonRecursive)?;
        // Watch first, establish EOF second. Changes after this attach remain
        // pending as a permit until the consumer starts waiting.
        let reader = TailReader::attach(path)?;
        Ok(Self {
            _watcher: watcher,
            reader,
            wake,
            watcher_error,
        })
    }

    pub fn wake_signal(&self) -> Arc<Notify> {
        self.wake.clone()
    }

    pub fn poll(&mut self) -> Result<ReadBatch> {
        let batch = self.reader.poll()?;
        if batch.more {
            self.wake.notify_one();
        }
        Ok(batch)
    }

    pub fn take_watcher_error(&self) -> Option<String> {
        self.watcher_error.lock().ok()?.take()
    }
}
