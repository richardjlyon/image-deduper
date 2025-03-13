//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

use std::path::Path;
pub use error::Error;
pub use config::Config;

pub mod discovery;
pub mod processing;
pub mod deduplication;
pub mod action;
pub mod config;
pub mod error;
pub mod safety;
pub mod types;

/// Main entry point for the deduplication process
pub struct ImageDeduper {
    config: Config,
    safety_manager: safety::SafetyManager,
}

impl ImageDeduper {
    /// Create a new ImageDeduper with the provided configuration
    pub fn new(config: Config) -> Self {
        let safety_manager = safety::SafetyManager::new(&config);
        Self { config, safety_manager }
    }

    /// Discover all images in the provided directories
    pub fn discover_images(&self, directories: &[impl AsRef<Path>]) -> Result<Vec<types::ImageFile>, Error> {
        discovery::discover_images(directories, &self.config)
    }

    /// Process images to generate hashes and extract metadata
    pub fn process_images(&self, images: Vec<types::ImageFile>) -> Result<Vec<types::ProcessedImage>, Error> {
        processing::process_images(images, &self.config)
    }

    /// Find duplicate images among the processed images
    pub fn find_duplicates(&self, images: Vec<types::ProcessedImage>) -> Result<Vec<types::DuplicateGroup>, Error> {
        deduplication::find_duplicates(images, &self.config)
    }

    /// Print a preview of actions without making changes
    pub fn preview_actions(&self, duplicate_groups: &[types::DuplicateGroup]) -> Result<(), Error> {
        action::preview_actions(duplicate_groups, &self.config)
    }

    /// Execute deduplication actions based on configuration
    pub fn execute_deduplication(&self, duplicate_groups: &[types::DuplicateGroup]) -> Result<(), Error> {
        action::execute_deduplication(duplicate_groups, &self.safety_manager, &self.config)
    }

    /// Run the full deduplication pipeline
    pub fn run(&self, directories: &[impl AsRef<Path>]) -> Result<(), Error> {
        // Discover images
        let discovered_files = self.discover_images(directories)?;

        // Process images
        let processed_images = self.process_images(discovered_files)?;

        // Find duplicates
        let duplicate_groups = self.find_duplicates(processed_images)?;

        // Take action
        if self.config.dry_run {
            self.preview_actions(&duplicate_groups)
        } else {
            self.execute_deduplication(&duplicate_groups)
        }
    }
}
