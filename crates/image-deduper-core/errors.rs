use std::path::PathBuf;
use thiserror::Error;

/// Custom error types for the image-deduper library
#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image processing error: {0}")]
    Image(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Failed to process HEIC image: {0}")]
    HeicProcessing(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Invalid configuration: {0}")]
    Configuration(String),

    #[error("Safety check failed: {0}")]
    SafetyCheck(String),

    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),

    #[error("Operation interrupted")]
    Interrupted,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<image::ImageError> for Error {
    fn from(err: image::ImageError) -> Self {
        Error::Image(err.to_string())
    }
}

/// Result type for image-deduper operations
pub type Result<T> = std::result::Result<T, Error>;
