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
use processing::PHash;
use rayon::prelude::*;
use rocksdb::DB;

// -- Standard Library --
use crate::processing::compute_cryptographic;
use crate::processing::perceptual::phash_from_file;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::path::PathBuf;
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

/// Get project root directory
pub fn get_project_root() -> PathBuf {
    std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap())
}

/// Get the default database directory path
pub fn get_default_db_path() -> PathBuf {
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".image-deduper").join("db")
}

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

    /// Get statistics about the database contents
    pub fn get_db_stats(&self) -> Result<(usize, usize)> {
        persistence::get_db_stats(&self.db)
    }

    /// Discover all images in the provided directories
    pub fn discover_images(
        &self,
        directories: &[impl AsRef<Path>],
    ) -> Result<Vec<types::ImageFile>> {
        discovery::discover_images(directories, &self.config)
    }

    /// Run the full deduplication pipeline
    pub fn run(&self, directories: &[impl AsRef<Path>], _force_rescan: bool) -> Result<()> {
        // Discover images
        info!("Discovering images...");
        let images = self.discover_images(directories)?;
        info!("Found {} images", images.len());

        Ok(())
    }

    /// Discover all images in the provided directories
    pub fn process_images(&self, image_files: &[ImageFile], _force_rescan: bool) -> Result<()> {
        // Overall function span
        let _span = tracy_client::span!("process_images");

        // Get current database stats to initialize progress
        let (current_db_count, _) = self.get_db_stats()?;
        info!(
            "Starting with {} images already in database",
            current_db_count
        );

        diagnose_database(&self.db)?;

        // Smaller batch size to reduce memory pressure
        const BATCH_SIZE: usize = 5;

        let total_images = image_files.len();
        let start_time = std::time::Instant::now();

        // Create a progress bar with style, initialized with current database count
        let progress_bar = ProgressBar::new(total_images as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("[{eta}] {bar:40.cyan/blue} {pos}/{len} ({percent}%) {msg}")
                .unwrap()
                .progress_chars("##-"),
        );
        progress_bar.set_message(format!(
            "Computing image hashes ({} existing)...",
            current_db_count
        ));
        progress_bar.set_position(current_db_count as u64);

        // Create a clone for the main thread to use at the end
        let main_progress_bar = progress_bar.clone();

        // Force the progress bar to render immediately
        progress_bar.tick();

        // Create thread-safe counters using parking_lot, which is more efficient than std::sync
        let success_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let failure_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let skip_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(current_db_count));
        let processed_count =
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(current_db_count));
        let window_start_time = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
            start_time.elapsed().as_millis() as u64,
        ));
        let window_start_count =
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(current_db_count));
        let last_flush_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Create a custom RocksDB write options for better throughput
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(false); // Don't force sync to disk on every write
        write_opts.disable_wal(true); // Disable write-ahead logging for better performance

        // Periodically force RocksDB to flush and compact
        let flush_interval = total_images / 20; // Flush every 5% of progress

        // Create a dedicated channel for progress updates to decouple UI from processing
        let (tx, rx) = std::sync::mpsc::channel();

        // Clone counters for the background thread
        let pc = processed_count.clone();
        let wst = window_start_time.clone();
        let wsc = window_start_count.clone();
        let lfc = last_flush_count.clone();

        // Spawn a dedicated high-priority thread for progress bar updates
        let update_handle = std::thread::Builder::new()
            .name("progress-updater".to_string())
            .spawn(move || {
                while let Ok(()) = rx.recv() {
                    // Update progress bar with current progress
                    let current = pc.load(std::sync::atomic::Ordering::Relaxed);
                    progress_bar.set_position(current as u64);

                    // Calculate images per second using a 5-second rolling window
                    let now = start_time.elapsed().as_millis() as u64;
                    let window_start = wst.load(std::sync::atomic::Ordering::Relaxed);
                    let window_count = wsc.load(std::sync::atomic::Ordering::Relaxed);

                    let time_delta = now - window_start;
                    let count_delta = current - window_count;

                    // Reset window if it's been more than 5 seconds
                    if time_delta >= 5000 {
                        wst.store(now, std::sync::atomic::Ordering::Relaxed);
                        wsc.store(current, std::sync::atomic::Ordering::Relaxed);
                    }

                    // Calculate rate over the window
                    let ips = if time_delta > 0 {
                        (count_delta as f64) / (time_delta as f64 / 1000.0)
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
            let _pool_span = tracy_client::span!("pool_processing");
            tracy_client::frame_mark(); // Mark the start of processing

            // Send initial progress update
            let _ = tx.send(());

            image_files
                .par_chunks(BATCH_SIZE)
                .enumerate()
                .for_each(|(chunk_idx, batch)| {
                    let _batch_span = tracy_client::span!("process_batch");
                    tracy_client::frame_mark(); // Mark each batch start

                    // Process each image in the batch in parallel
                    let results: Vec<_> = {
                        let _process_span = tracy_client::span!("process_batch_items");
                        let results = batch
                            .par_iter()
                            .map(|image| {
                                let _image_span = tracy_client::span!("process_single_image");

                                // Check if hashes exist first to avoid unnecessary processing
                                if let Ok(true) = check_hashes(&self.db, &image.path) {
                                    skip_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    return (None, true, false); // (data, skipped, failed)
                                }

                                // Only allocate path string if we need it
                                let path_str = image.path.to_string_lossy();

                                // Compute both hashes
                                match (
                                    compute_cryptographic(&image.path),
                                    phash_from_file(&image.path),
                                ) {
                                    (Ok(c_hash), Ok(p_hash)) => {
                                        // Convert hashes and return data
                                        let c_hash_bytes = blake3_to_vec(c_hash);
                                        let p_hash_bytes = phash_to_vec(&p_hash);

                                        (
                                            Some((
                                                path_str.into_owned(),
                                                c_hash_bytes,
                                                p_hash_bytes,
                                            )),
                                            false,
                                            false,
                                        )
                                    }
                                    _ => {
                                        failure_count
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        processed_count
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        (None, false, true)
                                    }
                                }
                            })
                            .collect();
                        results
                    };

                    // Extract data items and count results in a single pass
                    let (data_items, _batch_stats) = {
                        let _extract_span = tracy_client::span!("extract_batch_data");
                        let mut success = 0;
                        let mut skipped = 0;
                        let mut failed = 0;

                        let items: Vec<_> = results
                            .into_iter() // Use into_iter to consume results
                            .filter_map(|(data, is_skipped, is_failed)| {
                                if is_skipped {
                                    skipped += 1;
                                    None
                                } else if is_failed {
                                    failed += 1;
                                    None
                                } else {
                                    success += 1;
                                    data
                                }
                            })
                            .collect();

                        (items, (success, skipped, failed))
                    };

                    // Process database writes if we have data
                    if !data_items.is_empty() {
                        let _db_write_span = tracy_client::span!("batch_db_write");
                        let results_len = data_items.len();

                        // Create and fill batch operation
                        let batch_op = {
                            let _prepare_span = tracy_client::span!("prepare_batch");
                            let mut batch = rocksdb::WriteBatch::default();

                            for (path_str, c_hash_bytes, p_hash_bytes) in data_items {
                                // Create keys and add to batch
                                let path_c_key =
                                    [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();
                                let path_p_key =
                                    [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();
                                batch.put(&path_c_key, &c_hash_bytes);
                                batch.put(&path_p_key, &p_hash_bytes);

                                success_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            }
                            batch
                        };

                        // Write batch to database
                        if let Err(e) = self.db.write_opt(batch_op, &write_opts) {
                            log::error!("Failed to write batch: {}", e);
                            success_count
                                .fetch_sub(results_len, std::sync::atomic::Ordering::Relaxed);
                            failure_count
                                .fetch_add(results_len, std::sync::atomic::Ordering::Relaxed);
                        }
                    }

                    // Periodically flush and compact RocksDB
                    let total_processed = chunk_idx * BATCH_SIZE;
                    let last_flush = lfc.load(std::sync::atomic::Ordering::Relaxed);

                    // Use saturating subtraction to prevent overflow
                    let progress_since_flush = total_processed.saturating_sub(last_flush);

                    if progress_since_flush >= flush_interval {
                        // Try to update last_flush atomically - only one thread should succeed
                        let current = lfc.load(std::sync::atomic::Ordering::Acquire);
                        if current == last_flush {
                            // Only flush if we successfully update the counter
                            if lfc
                                .compare_exchange(
                                    current,
                                    total_processed,
                                    std::sync::atomic::Ordering::AcqRel,
                                    std::sync::atomic::Ordering::Relaxed,
                                )
                                .is_ok()
                            {
                                let _flush_span = tracy_client::span!("db_maintenance");
                                if let Err(e) = self.db.flush() {
                                    log::warn!("Failed to flush database: {}", e);
                                }
                            }
                        }
                    }

                    // Force memory cleanup every 1000 batches
                    if chunk_idx % 1000 == 0 {
                        let _gc_span = tracy_client::span!("force_gc");
                        let _ = batch; // Use let binding instead of drop for reference
                        std::thread::yield_now();
                    }

                    // Signal progress update and yield
                    let _ = tx.send(());
                    std::thread::yield_now();
                });
        });

        // Final database maintenance
        let _final_maintenance = tracy_client::span!("final_db_maintenance");
        let _ = self.db.flush();
        let _ = self.db.compact_range::<&[u8], &[u8]>(None, None);

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
