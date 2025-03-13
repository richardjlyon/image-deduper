use std::path::PathBuf;

/// Priority rules for choosing which image to keep as original
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Configuration for the image deduplication process
#[derive(Debug, Clone)]
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

    /// Whether to process unsupported image formats
    pub process_unsupported_formats: bool,

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

impl Default for Config {
    fn default() -> Self {
        Self {
            dry_run: true,
            duplicates_dir: PathBuf::from("duplicates"),
            delete_duplicates: false,
            create_symlinks: false,
            phash_threshold: 90,
            generate_thumbnails: true,
            backup_dir: Some(PathBuf::from("backup")),
            max_depth: None,
            process_unsupported_formats: false,
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
