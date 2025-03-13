use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use crate::error::{Error, Result};

/// Configuration for the image deduplication process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Whether to run without making changes
    pub dry_run: bool,

    /// Where to store duplicate images instead of deleting
    pub duplicates_dir: PathBuf,

    /// Whether to delete duplicates instead of moving them
    pub delete_duplicates: bool,

    /// Whether to create symbolic links to originals instead of keeping duplicates
    pub create_symlinks: bool,

    /// Threshold for perceptual hash similarity (0-100)
    pub phash_threshold: u8,

    /// Whether to generate thumbnails for visual comparison
    pub generate_thumbnails: bool,

    /// Backup directory for safety copies
    pub backup_dir: Option<PathBuf>,

    /// Maximum directory depth for scanning
    pub max_depth: Option<usize>,

    /// Number of threads to use for processing (0 = auto)
    pub threads: usize,

    /// Prioritization rules for choosing the original
    pub prioritization: Vec<PriorityRule>,

    /// Whether to use a database to store results
    pub use_database: bool,

    /// Path to the database file
    pub database_path: Option<PathBuf>,

    /// Log level
    pub log_level: LogLevel,
}

/// Rules for prioritizing which image to keep
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriorityRule {
    /// Prefer higher resolution images
    HighestResolution,

    /// Prefer images with earlier creation date
    OldestCreationDate,

    /// Prefer images with specific file format
    PreferredFormat,

    /// Prefer images in specific directories
    PreferredDirectory,

    /// Prefer smallest file size
    SmallestFileSize,

    /// Prefer largest file size
    LargestFileSize,
}

/// Log level for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dry_run: false,
            duplicates_dir: PathBuf::from("duplicates"),
            delete_duplicates: false,
            create_symlinks: false,
            phash_threshold: 90,
            generate_thumbnails: true,
            backup_dir: Some(PathBuf::from("backup")),
            max_depth: None,
            threads: 0, // Auto
            prioritization: vec![
                PriorityRule::HighestResolution,
                PriorityRule::LargestFileSize,
                PriorityRule::OldestCreationDate,
            ],
            use_database: true,
            database_path: Some(PathBuf::from("image-deduper.db")),
            log_level: LogLevel::Info,
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        let file = std::fs::File::open(path)
            .map_err(|e| Error::Configuration(format!("Failed to open config file: {}", e)))?;

        let config: Config = serde_json::from_reader(file)
            .map_err(|e| Error::Configuration(format!("Failed to parse config file: {}", e)))?;

        Ok(config)
    }

    /// Save configuration to a file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let file = std::fs::File::create(path)
            .map_err(|e| Error::Configuration(format!("Failed to create config file: {}", e)))?;

        serde_json::to_writer_pretty(file, self)
            .map_err(|e| Error::Configuration(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Check that we don't have conflicting options
        if self.delete_duplicates && self.create_symlinks {
            return Err(Error::Configuration(
                "Cannot both delete duplicates and create symlinks".to_string()
            ));
        }

        // Check threshold is in valid range
        if self.phash_threshold > 100 {
            return Err(Error::Configuration(
                "Perceptual hash threshold must be between 0 and 100".to_string()
            ));
        }

        // Make sure we have a database path if database is enabled
        if self.use_database && self.database_path.is_none() {
            return Err(Error::Configuration(
                "Database path must be specified if database is enabled".to_string()
            ));
        }

        Ok(())
    }
}
