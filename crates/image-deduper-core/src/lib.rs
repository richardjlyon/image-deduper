//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

// -- External Dependencies --

use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use persistence::check_hashes;
use persistence::diagnose_database;
use persistence::insert_hashes;
use processing::PHash;
use rayon::prelude::*;
use rocksdb::DB;

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
    db: DB,
    _safety_manager: safety::SafetyManager,
    _shutdown_requested: Arc<AtomicBool>,
}

impl ImageDeduper {
    /// Create a new ImageDeduper with the provided configuration
    pub fn new(config: Config) -> Self {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_cpus::get())
            .build_global()
            .unwrap();
        let db = persistence::rocksdb(&config).unwrap();
        let _safety_manager = safety::SafetyManager::new(&config);

        Self {
            config,
            db,
            _safety_manager,
            _shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Discover all images in the provided directories
    pub fn discover_images(
        &self,
        directories: &[impl AsRef<Path>],
    ) -> Result<Vec<types::ImageFile>> {
        discovery::discover_images(directories, &self.config)
    }

    /// Run the full deduplication pipeline
    pub fn run(&self, directories: &[impl AsRef<Path>], force_rescan: bool) -> Result<()> {
        // Discover images
        info!("Discovering images...");
        let images = self.discover_images(directories)?;
        info!("Found {} images", images.len());

        // Process and persist images
        // info!("Processing images...");
        // let processed_images = self.process_images(images, force_rescan)?;
        // info!("Processed {} images", processed_images.len());

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
    pub fn process_images(&self, image_files: &[ImageFile], _force_rescan: bool) -> Result<()> {
        diagnose_database(&self.db)?;

        const BATCH_SIZE: usize = 50;

        let total_images = image_files.len();
        let start_time = std::time::Instant::now();

        // Create a progress bar with style
        let progress_bar = ProgressBar::new(total_images as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("[{eta}] {bar:40.cyan/blue} {pos}/{len} ({percent}%) {msg}")
                .unwrap()
                .progress_chars("##-"),
        );
        progress_bar.set_message("Computing image hashes...");

        // Create a clone for the main thread to use at the end
        let main_progress_bar = progress_bar.clone();

        // Force the progress bar to render immediately
        progress_bar.tick();

        // Create thread-safe counters using parking_lot, which is more efficient than std::sync
        let success_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let failure_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let skip_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let processed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let start_time_arc = std::sync::Arc::new(start_time);

        // Create a custom RocksDB write options for better throughput
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(false); // Don't force sync to disk on every write

        // Create a dedicated channel for progress updates to decouple UI from processing
        let (tx, rx) = std::sync::mpsc::channel();

        // Clone counters for the background thread
        let pc = processed_count.clone();
        let st = start_time_arc.clone();

        // Spawn a dedicated high-priority thread for progress bar updates
        let update_handle = std::thread::Builder::new()
            .name("progress-updater".to_string())
            .spawn(move || {
                while let Ok(()) = rx.recv() {
                    // Update progress bar with current progress
                    let current = pc.load(std::sync::atomic::Ordering::Relaxed);
                    progress_bar.set_position(current as u64);

                    // Calculate images per second
                    let elapsed_secs = st.elapsed().as_secs_f64();
                    let ips = if elapsed_secs > 0.0 {
                        current as f64 / elapsed_secs
                    } else {
                        0.0
                    };

                    // Update message with processing rate
                    progress_bar.set_message(format!("{:.1} images/sec", ips));

                    // Sleep briefly to prevent too-frequent updates
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }

                // Final update before thread exits
                let current = pc.load(std::sync::atomic::Ordering::Relaxed);
                progress_bar.set_position(current as u64);
            })
            .expect("Failed to create progress update thread");

        // Create a Rayon thread pool with a specific number of threads
        // Use num_cpus - 1 to leave one CPU for the UI thread
        let num_threads = std::cmp::max(1, num_cpus::get() - 1);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .expect("Failed to build thread pool");

        // Process all chunks on the custom thread pool
        pool.install(|| {
            // Send initial progress update
            let _ = tx.send(());

            image_files.par_chunks(BATCH_SIZE).for_each(|batch| {
                // Process each image in the batch in parallel
                let results: Vec<_> = batch
                    .par_iter()
                    .map(|image| {
                        // Check if hashes exist
                        if let Ok(true) = check_hashes(&self.db, &image.path) {
                            skip_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            return (None, true, false); // (data, skipped, failed)
                        }

                        // Compute both hashes
                        let c_hash_result = compute_cryptographic(&image.path);
                        let p_hash_result = phash_from_file(&image.path);

                        match (c_hash_result, p_hash_result) {
                            (Ok(c_hash), Ok(p_hash)) => {
                                // Convert hashes
                                let c_hash_bytes = blake3_to_vec(c_hash);
                                let p_hash_bytes = phash_to_vec(&p_hash);
                                let path_str = image.path.to_string_lossy().into_owned();

                                // Return data for batch insertion with status flags
                                (Some((path_str, c_hash_bytes, p_hash_bytes)), false, false)
                            }
                            _ => {
                                failure_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                (None, false, true)
                            }
                        }
                    })
                    .collect();

                // Count the results by status type
                let mut batch_success = 0;
                let mut batch_skipped = 0;
                let mut batch_failed = 0;

                // Extract just the data items for database operations
                let data_items: Vec<_> = results
                    .iter()
                    .filter_map(|(data, skipped, failed)| {
                        if *skipped {
                            batch_skipped += 1;
                            None
                        } else if *failed {
                            batch_failed += 1;
                            None
                        } else {
                            batch_success += 1;
                            data.clone()
                        }
                    })
                    .collect();

                // Batch database writes - one transaction per batch chunk
                if !data_items.is_empty() {
                    let mut batch_op = rocksdb::WriteBatch::default();
                    let results_len = data_items.len();

                    // Add all writes to the batch
                    for (path_str, c_hash_bytes, p_hash_bytes) in data_items {
                        // Path->hash mappings
                        let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();
                        let path_p_key = [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();
                        batch_op.put(&path_c_key, &c_hash_bytes);
                        batch_op.put(&path_p_key, &p_hash_bytes);

                        success_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }

                    // Write the entire batch at once with custom options
                    if let Err(e) = self.db.write_opt(batch_op, &write_opts) {
                        log::error!("Failed to write batch: {}", e);
                        // If batch write fails, count all as failures
                        success_count.fetch_sub(results_len, std::sync::atomic::Ordering::Relaxed);
                        failure_count.fetch_add(results_len, std::sync::atomic::Ordering::Relaxed);
                    }
                }

                // Signal the progress update thread after each batch
                let _ = tx.send(());

                // Explicitly yield to allow other threads to run (helps with progress updates)
                std::thread::yield_now();
            });
        });

        // Signal completion and wait for update thread to finish
        drop(tx);
        let _ = update_handle.join();

        // Make sure the progress bar shows completion
        let total_elapsed_secs = start_time.elapsed().as_secs_f64();
        let final_ips = if total_elapsed_secs > 0.0 {
            processed_count.load(std::sync::atomic::Ordering::Relaxed) as f64 / total_elapsed_secs
        } else {
            0.0
        };

        main_progress_bar.finish_with_message(format!(
            "Completed! Processed at {:.1} images/sec",
            final_ips
        ));

        Ok(())
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
