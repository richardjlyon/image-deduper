//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

use std::{path::Path, sync::Arc};

mod error;

// -- Flatten
pub use config::*;
pub use error::{Error, Result};
use log::info;
pub use types::*;

// -- Public Modules --
pub mod action;
pub mod config;
pub mod discovery;
pub mod persistence;
pub mod processing;
pub mod safety;
pub mod types;
// pub mod deduplication;
#[cfg(test)]
pub mod test_utils;

/// Main entry point for the deduplication process
pub struct ImageDeduper {
    config: Config,
    _safety_manager: safety::SafetyManager,
}

impl ImageDeduper {
    /// Create a new ImageDeduper with the provided configuration
    pub fn new(config: Config) -> Self {
        let _safety_manager = safety::SafetyManager::new(&config);
        Self {
            config,
            _safety_manager,
        }
    }

    /// Discover all images in the provided directories
    pub fn discover_images(
        &self,
        directories: &[impl AsRef<Path>],
    ) -> Result<Vec<types::ImageFile>> {
        discovery::discover_images(directories, &self.config)
    }

    /// Process images to generate hashes and extract metadata
    pub fn process_images(
        &self,
        images: Vec<types::ImageFile>,
    ) -> Result<Vec<types::ProcessedImage>> {
        let batch_size = self.config.batch_size.unwrap_or(100);
        let mut batch = Vec::with_capacity(batch_size);
        let mut processed = Vec::with_capacity(images.len());
        let total_images = images.len();

        // Get database path from config or use default
        let db_path = if self.config.use_database {
            self.config
                .database_path
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("image_deduper.db"))
        } else {
            std::path::PathBuf::from("image_deduper.db")
        };

        for (i, img) in images.into_iter().enumerate() {
            let chash = processing::compute_cryptographic(&img.path)?;
            let phash = processing::phash_from_file(&img.path)?;

            let processed_image = types::ProcessedImage {
                original: Arc::new(img),
                cryptographic_hash: chash,
                perceptual_hash: phash,
            };
            batch.push(processed_image.clone());
            processed.push(processed_image);

            if batch.len() >= batch_size || i == total_images - 1 {
                if !batch.is_empty() {
                    persistence::save_processed_images(&db_path, &batch)?;
                    batch.clear();
                }
            }
        }

        Ok(processed)
    }

    // /// Find duplicate images among the processed images
    // pub fn find_duplicates(&self, images: Vec<types::ProcessedImage>) -> Result<Vec<types::DuplicateGroup>, Error> {
    //     deduplication::find_duplicates(images, &self.config)
    // }

    // /// Print a preview of actions without making changes
    // pub fn preview_actions(&self, duplicate_groups: &[types::DuplicateGroup]) -> Result<(), Error> {
    //     action::preview_actions(duplicate_groups, &self.config)
    // }

    // /// Execute deduplication actions based on configuration
    // pub fn execute_deduplication(&self, duplicate_groups: &[types::DuplicateGroup]) -> Result<(), Error> {
    //     action::execute_deduplication(duplicate_groups, &self.safety_manager, &self.config)
    // }

    /// Run the full deduplication pipeline
    pub fn run(&self, directories: &[impl AsRef<Path>]) -> Result<()> {
        // Discover images
        info!("Discovering images...");
        let images = self.discover_images(directories)?;
        info!("Found {} images", images.len());

        // Process and persist image
        info!("Processing images...");
        let processed_images = self.process_images(images)?;
        info!("Processed {} images", processed_images.len());

        for img in processed_images {
            println!("{:?}", img.perceptual_hash);
        }

        // // Find duplicates
        // let duplicate_groups = self.find_duplicates(processed_images)?;

        // Take action
        // if self.config.dry_run {
        //     self.preview_actions(&duplicate_groups)
        // } else {
        //     self.execute_deduplication(&duplicate_groups)
        // }

        Ok(())
    }
}
