use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = core::result::Result<T, Error>;

/// Custom error types for the image-deduper library
#[derive(Error, Debug)]
pub enum Error {
    /// I/O operation error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] rocksdb::Error),

    /// Image processing error
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    /// File not found error
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Invalid configuration error
    #[error("Invalid configuration: {0}")]
    Configuration(String),

    #[error("HEIC image doesn't have interleaved data")]
    HEICInterleaveError,

    /// Safety check failure
    #[error("Safety check failed: {0}")]
    SafetyCheck(String),

    /// Unsupported image format
    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),

    /// Format handling error
    #[error("Format handling error: {0}")]
    FormatHandling(String),

    /// Unknown error
    #[error("Unknown error: {0}")]
    Unknown(String),
}
