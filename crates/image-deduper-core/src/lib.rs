//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

// -- External Dependencies --

use log::info;
use persistence::check_hashes;
use persistence::insert_hashes;
use processing::PHash;

// -- Standard Library --
use crate::processing::compute_cryptographic;
use crate::processing::perceptual::phash_from_file;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::time::Instant;
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

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
    _shutdown_requested: Arc<AtomicBool>,
}

impl ImageDeduper {
    /// Create a new ImageDeduper with the provided configuration
    pub fn new(config: Config) -> Self {
        let _safety_manager = safety::SafetyManager::new(&config);
        Self {
            config,
            _safety_manager,
            _shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Run the full deduplication pipeline
    pub fn run(
        &self,
        directories: &[impl AsRef<Path>],
        force_rescan: bool,
    ) -> Result<Vec<types::ProcessedImage>> {
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

        Ok(processed_images)
    }

    /// Discover all images in the provided directories
    pub fn discover_images(
        &self,
        directories: &[impl AsRef<Path>],
    ) -> Result<Vec<types::ImageFile>> {
        discovery::discover_images(directories, &self.config)
    }

    /// Process images and persist them
    pub fn process_images(
        &self,
        images: Vec<types::ImageFile>,
        _force_rescan: bool,
    ) -> Result<Vec<types::ProcessedImage>> {
        info!("Processing images...");

        let db = persistence::rocksdb(&self.config)?;

        println!("Processing {} images with Rayon...", images.len());
        let start = Instant::now();

        // Process 5 images to test the database
        let test_images = images.iter().take(5).collect::<Vec<_>>();
        test_images.par_iter().for_each(|image| {
            // First check if hashes already exist for this path
            match check_hashes(&db, &image.path) {
                Ok(true) => {
                    // Hashes already exist, skip computation
                    log::debug!(
                        "Skipping hash computation for already processed image: {}",
                        image.path.display()
                    );
                }
                Ok(false) | Err(_) => {
                    // Hashes don't exist or error occurred, compute them
                    log::debug!("Computing hashes for: {}", image.path.display());

                    // Compute cryptographic hash
                    let c_hash_result = compute_cryptographic(&image.path);
                    // Compute perceptual hash
                    let p_hash_result = phash_from_file(&image.path);

                    // Proceed only if both hashes computed successfully
                    match (c_hash_result, p_hash_result) {
                        (Ok(c_hash), Ok(p_hash)) => {
                            // Convert both hash values to Vec<u8>
                            let c_hash_bytes = blake3_to_vec(c_hash);
                            let p_hash_bytes = phash_to_vec(&p_hash);

                            // Store the hashes in the database
                            if let Err(e) =
                                insert_hashes(&db, &image.path, &c_hash_bytes, &p_hash_bytes)
                            {
                                log::error!(
                                    "Failed to insert hashes for {}: {}",
                                    image.path.display(),
                                    e
                                );
                            } else {
                                log::debug!(
                                    "Successfully inserted hashes for: {}",
                                    image.path.display()
                                );
                            }
                        }
                        (Err(e), _) => {
                            log::error!(
                                "Failed to compute cryptographic hash for {}: {}",
                                image.path.display(),
                                e
                            );
                        }
                        (_, Err(e)) => {
                            log::error!(
                                "Failed to compute perceptual hash for {}: {}",
                                image.path.display(),
                                e
                            );
                        }
                    }
                }
            }
        });

        // println!(
        //     "Successfully processed {}/{} test images",
        //     test_results.len(),
        //     test_images.len()
        // );

        Ok(vec![])
    }
}

// Helper function to convert blake3::Hash to Vec<u8>
fn blake3_to_vec(hash: blake3::Hash) -> Vec<u8> {
    hash.as_bytes().to_vec()
}

// Helper function to convert PHash to Vec<u8>
fn phash_to_vec(phash: &PHash) -> Vec<u8> {
    match phash {
        PHash::Standard(hash_value) => {
            // Convert u64 to 8 bytes
            hash_value.to_be_bytes().to_vec()
        }
        PHash::Enhanced(hash_array) => {
            // Convert [u64; 16] to 128 bytes
            let mut bytes = Vec::with_capacity(128);
            for &value in hash_array.iter() {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
            bytes
        }
    }
}
