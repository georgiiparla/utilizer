//! CS2 capture core. No browser, server, or real-game dependency in tests.

pub mod cfg;
pub mod discovery;
pub mod engine;
pub mod log;
pub mod protocol;
pub mod storage;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}
