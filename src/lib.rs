//! CS2 practice core and local HTML adapter. Tests do not require CS2 or a browser.

pub mod cfg;
#[cfg(feature = "dev")]
pub mod dev;
pub mod discovery;
pub mod engine;
pub mod log;
pub mod maps;
pub mod protocol;
pub mod storage;
pub mod web;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}
