use std::path::PathBuf;

pub type Result<T> = core::result::Result<T, Error>;

/// Custom error types for the image-deduper library
#[derive(Debug)]
pub enum Error {
    /// I/O operation error
    Io(std::io::Error),

    /// Image processing error
    Image(image::ImageError),

    /// File not found error
    FileNotFound(PathBuf),

    /// Invalid configuration error
    Configuration(String),

    /// Safety check failure
    SafetyCheck(String),

    /// Unsupported image format
    UnsupportedFormat(String),

    /// Unknown error
    Unknown(String),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<image::ImageError> for Error {
    fn from(err: image::ImageError) -> Self {
        Error::Image(err)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        match self {
            Error::Io(err) => write!(fmt, "I/O error: {}", err),
            Error::Image(err) => write!(fmt, "Image processing error: {}", err),
            Error::FileNotFound(path) => write!(fmt, "File not found: {}", path.display()),
            Error::Configuration(msg) => write!(fmt, "Invalid configuration: {}", msg),
            Error::SafetyCheck(msg) => write!(fmt, "Safety check failed: {}", msg),
            Error::UnsupportedFormat(msg) => write!(fmt, "Unsupported image format: {}", msg),
            Error::Unknown(msg) => write!(fmt, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::Image(err) => Some(err),
            _ => None,
        }
    }
}
