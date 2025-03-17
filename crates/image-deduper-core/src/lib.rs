//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

// -- External Dependencies --
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, trace, warn};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rayon::prelude::*;

// -- Standard Library --
use std::{path::Path, sync::Arc};

// -- Internal Modules --
mod error;

// -- Public Re-exports --
pub use config::*;
pub use error::{Error, Result};
pub use types::*;

// -- Public Modules --
pub mod action;
pub mod config;
pub mod discovery;
pub mod logging;
pub mod persistence;
pub mod processing;
pub mod safety;
pub mod types;
// pub mod deduplication;

// -- Test Modules --
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

    /// Discover all images in the provided directories
    pub fn discover_images(
        &self,
        directories: &[impl AsRef<Path>],
    ) -> Result<Vec<types::ImageFile>> {
        discovery::discover_images(directories, &self.config)
    }

    /// Process images to generate hashes and extract metadata
    fn process_images(
        &self,
        images: Vec<types::ImageFile>,
        force_rescan: bool,
    ) -> Result<Vec<types::ProcessedImage>> {
        let total_images = images.len();
        info!(
            "Starting processing of {} images, force_rescan={}",
            total_images, force_rescan
        );

        // Set up progress bar
        let progress = ProgressBar::new(total_images as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}",
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
        debug!("Using database path: {}", db_path.display());

        // Create a connection pool
        let manager = SqliteConnectionManager::file(&db_path);
        let pool = match Pool::builder()
            .max_size(num_cpus::get() as u32) // One connection per CPU core
            .build(manager)
        {
            Ok(pool) => pool,
            Err(e) => {
                // Use our logger helper for db file operation
                logging::log_file_error(&db_path, "create_connection_pool", &e);
                return Err(Error::Unknown(format!(
                    "Failed to create connection pool: {}",
                    e
                )));
            }
        };

        // Initialize the database schema
        let conn = match pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to get database connection: {}", e);
                return Err(Error::Unknown(format!(
                    "Failed to get database connection: {}",
                    e
                )));
            }
        };

        if let Err(e) = persistence::initialize_database(&conn) {
            error!("Failed to initialize database: {}", e);
            return Err(e.into());
        }
        debug!("Database initialized successfully");

        // Process images in chunks to manage memory usage
        let chunk_size = 1000;
        let mut processed = Vec::with_capacity(total_images);
        let mut failed_images = Vec::new();

        for (chunk_index, chunk) in images.chunks(chunk_size).enumerate() {
            let chunk_message = format!("Processing chunk {}", chunk_index + 1);
            progress.set_message(chunk_message.clone());
            debug!("{} ({} images)", chunk_message, chunk.len());

            let chunk_results: Vec<_> = chunk
                .par_iter()
                .map(|img| {
                    let progress = Arc::clone(&progress);
                    let pool = Arc::new(pool.clone());

                    let result = if !force_rescan {
                        // Get a connection from the pool
                        let conn = match pool.get() {
                            Ok(conn) => conn,
                            Err(e) => {
                                progress.inc(1);
                                error!(
                                    "Failed to get database connection while processing {}: {}",
                                    img.path.display(),
                                    e
                                );
                                return Err(Error::Unknown(format!(
                                    "Failed to get database connection: {}",
                                    e
                                )));
                            }
                        };

                        // Try to get from database first
                        match persistence::get_image_by_path_with_conn(&conn, &img.path) {
                            Ok(stored_image) => {
                                trace!(
                                    "Found image {} in database, using cached data",
                                    img.path.display()
                                );

                                // Convert Vec<u8> to [u8; 32] for Blake3 hash
                                let chash: [u8; 32] =
                                    match stored_image.cryptographic_hash.try_into() {
                                        Ok(hash) => hash,
                                        Err(_) => {
                                            progress.inc(1);
                                            logging::log_hash_error(
                                                &img.path,
                                                &std::io::Error::new(
                                                    std::io::ErrorKind::InvalidData,
                                                    "Invalid cryptographic hash length",
                                                ),
                                            );
                                            return Err(Error::Unknown(format!(
                                                "Invalid cryptographic hash length for {}",
                                                img.path.display()
                                            )));
                                        }
                                    };

                                Ok(types::ProcessedImage {
                                    original: Arc::new(img.clone()),
                                    cryptographic_hash: chash.into(),
                                    perceptual_hash: processing::PHash(
                                        stored_image.perceptual_hash,
                                    ),
                                })
                            }
                            Err(e) => {
                                debug!(
                                    "Image {} not found in database: {}, computing hashes",
                                    img.path.display(),
                                    e
                                );
                                // Not in database, process it
                                match Self::process_single_image(img, &conn) {
                                    Ok(processed_image) => Ok(processed_image),
                                    Err(e) => {
                                        logging::log_file_error(&img.path, "process_image", &e);
                                        Err(e)
                                    }
                                }
                            }
                        }
                    } else {
                        // Force rescan, always process
                        debug!("Force rescanning image {}", img.path.display());
                        let conn = match pool.get() {
                            Ok(conn) => conn,
                            Err(e) => {
                                progress.inc(1);
                                error!(
                                    "Failed to get database connection for {}: {}",
                                    img.path.display(),
                                    e
                                );
                                return Err(Error::Unknown(format!(
                                    "Failed to get database connection: {}",
                                    e
                                )));
                            }
                        };

                        match Self::process_single_image(img, &conn) {
                            Ok(processed_image) => Ok(processed_image),
                            Err(e) => {
                                logging::log_file_error(&img.path, "process_image", &e);
                                Err(e)
                            }
                        }
                    };

                    progress.inc(1);
                    result
                })
                .collect();

            // Separate successful and failed images
            for (img, result) in chunk.iter().zip(chunk_results.into_iter()) {
                match result {
                    Ok(processed_image) => processed.push(processed_image),
                    Err(e) => failed_images.push((img.path.clone(), e)),
                }
            }
        }

        let completion_message = format!(
            "Processing complete - {} succeeded, {} failed",
            processed.len(),
            failed_images.len()
        );
        progress.finish_with_message(completion_message.clone());
        info!("{}", completion_message);

        // Log failed images
        if !failed_images.is_empty() {
            warn!("Failed to process {} images:", failed_images.len());
            for (path, error) in &failed_images {
                logging::log_file_error(path, "process_image", error);
            }
        }

        Ok(processed)
    }

    /// Helper function to process a single image
    fn process_single_image(
        img: &types::ImageFile,
        conn: &r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
    ) -> Result<types::ProcessedImage> {
        // Process the image
        let chash = processing::compute_cryptographic(&img.path)?;
        let phash = processing::phash_from_file(&img.path)?;

        let processed_image = types::ProcessedImage {
            original: Arc::new(img.clone()),
            cryptographic_hash: chash,
            perceptual_hash: phash,
        };

        // Save to database
        if let Err(e) = persistence::save_processed_image_with_conn(conn, &processed_image) {
            info!(
                "Failed to save image {} to database: {}",
                img.path.display(),
                e
            );
            // Continue even if database save fails
        }

        Ok(processed_image)
    }
}
