use rusqlite;
use std::path::PathBuf;

/// Result type for persistence operations
pub type PersistenceResult<T> = Result<T, PersistenceError>;

/// Persistence-specific errors
#[derive(Debug)]
pub enum PersistenceError {
    /// SQLite errors
    Database(rusqlite::Error),

    /// File path related errors
    Path(PathBuf, String),

    /// Duplicate entry errors
    Duplicate(String),

    /// Image not found errors
    NotFound(String),

    /// Errors during database initialization
    Initialization(String),

    /// General errors
    Other(String),
}

impl From<rusqlite::Error> for PersistenceError {
    fn from(err: rusqlite::Error) -> Self {
        PersistenceError::Database(err)
    }
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(err) => write!(f, "Database error: {}", err),
            Self::Path(path, msg) => write!(f, "Path error for {}: {}", path.display(), msg),
            Self::Duplicate(msg) => write!(f, "Duplicate entry: {}", msg),
            Self::NotFound(msg) => write!(f, "Entry not found: {}", msg),
            Self::Initialization(msg) => write!(f, "Database initialization error: {}", msg),
            Self::Other(msg) => write!(f, "Persistence error: {}", msg),
        }
    }
}

impl std::error::Error for PersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(err) => Some(err),
            _ => None,
        }
    }
}

// Implement conversion from PersistenceError to the main Error type
impl From<PersistenceError> for crate::Error {
    fn from(err: PersistenceError) -> Self {
        match err {
            PersistenceError::Database(e) => {
                crate::Error::Unknown(format!("Database error: {}", e))
            }
            PersistenceError::Path(path, msg) => crate::Error::FileNotFound(path),
            PersistenceError::NotFound(_) => crate::Error::Unknown("Record not found".to_string()),
            _ => crate::Error::Unknown(format!("Persistence error: {}", err)),
        }
    }
}
