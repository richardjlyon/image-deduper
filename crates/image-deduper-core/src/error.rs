use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = core::result::Result<T, Error>;

/// Custom error types for the image-deduper library
#[derive(Error, Debug)]
pub enum Error {
    /// I/O operation error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Image processing error
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    /// File not found error
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Invalid configuration error
    #[error("Invalid configuration: {0}")]
    Configuration(String),

    /// Safety check failure
    #[error("Safety check failed: {0}")]
    SafetyCheck(String),

    /// Unsupported image format
    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),

    /// Unknown error
    #[error("Unknown error: {0}")]
    Unknown(String),
}
