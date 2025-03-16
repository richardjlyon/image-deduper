//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rayon::prelude::*;
use std::{path::Path, sync::Arc};

mod error;

// -- Flatten
pub use config::*;
pub use error::{Error, Result};
use indicatif::{ProgressBar, ProgressStyle};
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
        force_rescan: bool,
    ) -> Result<Vec<types::ProcessedImage>> {
        let batch_size = self.config.batch_size.unwrap_or(100);
        let total_images = images.len();

        // Set up progress bar
        let progress = ProgressBar::new(total_images as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );

        // Create a thread-safe progress bar
        let progress = Arc::new(progress);

        // Get database path from config or use default
        let db_path = if self.config.use_database {
            self.config
                .database_path
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("image_deduper.db"))
        } else {
            std::path::PathBuf::from("image_deduper.db")
        };

        // Create a connection pool
        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder()
            .max_size(num_cpus::get() as u32) // One connection per CPU core
            .build(manager)
            .map_err(|e| Error::Unknown(format!("Failed to create connection pool: {}", e)))?;

        // Initialize the database schema
        let conn = pool
            .get()
            .map_err(|e| Error::Unknown(format!("Failed to get database connection: {}", e)))?;
        persistence::initialize_database(&conn)?;

        // Process images in parallel
        let processed: Result<Vec<_>> = images
            .into_par_iter()
            .map(|img| {
                let progress = Arc::clone(&progress);
                let pool = Arc::new(pool.clone());

                let result = if !force_rescan {
                    // Get a connection from the pool
                    let conn = pool.get().map_err(|e| {
                        Error::Unknown(format!("Failed to get database connection: {}", e))
                    })?;

                    // Try to get from database first
                    if let Ok(stored_image) =
                        persistence::get_image_by_path_with_conn(&conn, &img.path)
                    {
                        // Convert Vec<u8> to [u8; 32] for Blake3 hash
                        let chash: [u8; 32] =
                            stored_image.cryptographic_hash.try_into().map_err(|_| {
                                Error::Unknown(format!(
                                    "Invalid cryptographic hash length for {}",
                                    img.path.display()
                                ))
                            })?;

                        Ok(types::ProcessedImage {
                            original: Arc::new(img),
                            cryptographic_hash: chash.into(),
                            perceptual_hash: processing::PHash(stored_image.perceptual_hash),
                        })
                    } else {
                        // Not in database, process it
                        let chash = processing::compute_cryptographic(&img.path)?;
                        let phash = processing::phash_from_file(&img.path)?;

                        let processed_image = types::ProcessedImage {
                            original: Arc::new(img),
                            cryptographic_hash: chash,
                            perceptual_hash: phash,
                        };

                        // Save to database
                        persistence::save_processed_image_with_conn(&conn, &processed_image)?;
                        Ok(processed_image)
                    }
                } else {
                    // Force rescan, always process
                    let conn = pool.get().map_err(|e| {
                        Error::Unknown(format!("Failed to get database connection: {}", e))
                    })?;

                    let chash = processing::compute_cryptographic(&img.path)?;
                    let phash = processing::phash_from_file(&img.path)?;

                    let processed_image = types::ProcessedImage {
                        original: Arc::new(img),
                        cryptographic_hash: chash,
                        perceptual_hash: phash,
                    };

                    // Save to database
                    persistence::save_processed_image_with_conn(&conn, &processed_image)?;
                    Ok(processed_image)
                };

                progress.inc(1);
                result
            })
            .collect();

        let processed = processed?;
        progress.finish_with_message("Processing complete");
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
    pub fn run(&self, directories: &[impl AsRef<Path>], force_rescan: bool) -> Result<()> {
        // Discover images
        info!("Discovering images...");
        let images = self.discover_images(directories)?;
        info!("Found {} images", images.len());

        // Process and persist images
        info!("Processing images...");
        let processed_images = self.process_images(images, force_rescan)?;
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
