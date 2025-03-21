//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

// -- External Dependencies --

use log::{info, warn};
use persistence::ImageHashDB;

// -- Standard Library --
use std::path::PathBuf;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc,
    },
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
pub mod deduplication;
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
    db: ImageHashDB,
    _safety_manager: safety::SafetyManager,
    _shutdown_requested: Arc<AtomicBool>,
    memory_tracker: Arc<MemoryTracker>,
}

impl ImageDeduper {
    /// Create a new ImageDeduper with the provided configuration
    pub fn new(config: Config) -> Self {
        let cpu_count = num_cpus::get();
        // Cap at 8 threads to prevent too many file handles
        let thread_count = std::cmp::min(cpu_count, 8);

        rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build_global()
            .unwrap();

        // Attempt to increase file descriptor limit on Unix platforms
        #[cfg(unix)]
        {
            // Try to set higher file limit if possible
            let _ = Self::increase_file_limit();

            // Check current limit for logging
            if let Ok(limits) = rlimit::getrlimit(rlimit::Resource::NOFILE) {
                log::info!(
                    "File descriptor limits: current={}, maximum={}",
                    limits.0,
                    limits.1
                );
            }
        }

        let db = ImageHashDB::new(&config);
        let memory_tracker = Arc::new(MemoryTracker::new());
        let _safety_manager = safety::SafetyManager::new(&config);
        let _shutdown_requested = Arc::new(AtomicBool::new(false));

        Self {
            config,
            db,
            memory_tracker,
            _safety_manager,
            _shutdown_requested,
        }
    }

    /// Try to increase the file descriptor limit on Unix systems
    #[cfg(unix)]
    fn increase_file_limit() -> std::result::Result<(), String> {
        // Attempt to raise the file descriptor limit
        match rlimit::getrlimit(rlimit::Resource::NOFILE) {
            Ok((soft, hard)) => {
                // Only try to increase if hard limit is higher than soft limit
                if hard > soft {
                    // Try to raise to hard limit or 4096, whichever is lower
                    let new_soft = std::cmp::min(hard, 4096);
                    if new_soft > soft {
                        if let Err(e) = rlimit::setrlimit(rlimit::Resource::NOFILE, new_soft, hard)
                        {
                            log::warn!("Failed to increase file descriptor limit: {}", e);
                            return Err(e.to_string());
                        } else {
                            log::info!(
                                "Increased file descriptor limit from {} to {}",
                                soft,
                                new_soft
                            );
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to get file descriptor limits: {}", e);
                Err(e.to_string())
            }
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
    pub fn run(&self, directories: &[impl AsRef<Path>], _force_rescan: bool) -> Result<()> {
        // Discover images
        info!("Discovering images...");
        let images = self.discover_images(directories)?;
        info!("Found {} images", images.len());

        Ok(())
    }

    /// Hash and persist all images in the provided directories
    pub fn hash_and_persist(
        &self,
        image_files: &[ImageFile],
        force_rescan: bool,
    ) -> Result<(usize, usize)> {
        // Perform a database diagnosis
        // diagnose_database(&self.db)?;

        // Get image paths from ImageFile objects
        let image_paths: Vec<PathBuf> = image_files.iter().map(|img| img.path.clone()).collect();

        // Determine which images need processing
        let paths_to_process = if force_rescan {
            info!(
                "Force rescan requested - processing all {} images",
                image_paths.len()
            );
            image_paths
        } else {
            // Filter out images already in database
            let new_paths = self.db.find_new_images(&image_paths)?;
            info!("Found {} new images to process", new_paths.len());
            new_paths
        };

        if paths_to_process.is_empty() {
            info!("No new images to process");
            return Ok(self.db.get_db_stats()?);
        }

        // Choose a smaller batch size to prevent file handle exhaustion
        // This helps prevent hitting the "Too many open files" limit
        const BATCH_SIZE: usize = 30;

        // Create a smaller batch size for more frequent checkpoints and better resource management
        let effective_batch_size = std::cmp::min(BATCH_SIZE, 10);

        // Get current database stats
        let (current_db_count, _) = self.db.get_db_stats()?;
        info!(
            "Starting processing with {} images already in database",
            current_db_count
        );

        // Get stats for the progress tracker
        let already_processed = current_db_count;
        let total_images = image_files.len();
        let total_batches =
            (paths_to_process.len() + effective_batch_size - 1) / effective_batch_size;

        // Create progress tracker with:
        // - Total = already in DB + total images passed in
        // - Initial position = already in DB
        info!(
            "Starting process counter: {} {} {}",
            total_images, already_processed, already_processed
        );
        let progress =
            processing::ProgressTracker::new(total_images, already_processed, already_processed, 0);

        // Track success and error counts
        let successful_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));

        // Create a shared progress counter
        let processed_count = Arc::new(AtomicUsize::new(0));

        // Process images in smaller batches to manage memory usage
        for (batch_idx, image_batch) in paths_to_process.chunks(effective_batch_size).enumerate() {
            // Update progress tracker for new batch

            progress.start_batch(image_batch.len(), batch_idx + 1, total_batches);

            // Update memory stats before processing
            let (pre_mem, _) = self.memory_tracker.update();
            info!(
                "Memory before batch {}: {}MB",
                batch_idx + 1,
                pre_mem / 1024 / 1024
            );

            // Process this batch of images
            let batch_results =
                processing::process_image_batch(image_batch, Some(&processed_count), None);

            // Update success and error counts
            let batch_success = batch_results.0.len();
            let batch_errors = batch_results.1;

            // Add to our running counters
            successful_count.fetch_add(batch_success, std::sync::atomic::Ordering::Relaxed);
            error_count.fetch_add(batch_errors, std::sync::atomic::Ordering::Relaxed);

            // Use only the counts from this run - do NOT add already_processed
            let current_successful = successful_count.load(std::sync::atomic::Ordering::Relaxed);
            let current_errors = error_count.load(std::sync::atomic::Ordering::Relaxed);

            // Update the batch progress tracker with this batch's results
            progress.update_batch(
                batch_success + batch_errors,
                format!("{} ok, {} errors", batch_success, batch_errors).as_str(),
            );

            // Complete the batch to calculate processing rate for this specific batch
            progress.complete_batch(batch_success, batch_errors);

            // Update the main progress bar with the overall totals
            progress.increment(current_successful, current_errors);

            // Check memory usage after processing
            let (post_mem, diff) = self.memory_tracker.update();
            info!(
                "Memory after batch {}: {}MB ({}MB change)",
                batch_idx + 1,
                post_mem / 1024 / 1024,
                diff / 1024 / 1024
            );

            // Track batch status with detailed statistics
            info!(
                "Batch {} complete: {} successes, {} errors",
                batch_idx + 1,
                batch_results.0.len(),
                batch_results.1
            );

            // Log file extensions that had errors if there were any errors
            if batch_results.1 > 0 {
                let error_extensions = image_batch
                    .iter()
                    .filter(|p| !batch_results.0.iter().any(|r| &r.path == *p))
                    .filter_map(|p| p.extension().and_then(|e| e.to_str()))
                    .collect::<Vec<_>>();

                if !error_extensions.is_empty() {
                    info!(
                        "Problematic file extensions in batch {}: {:?}",
                        batch_idx + 1,
                        error_extensions
                    );
                }
            }

            // Store the results in the database with database-side error handling
            if !batch_results.0.is_empty() {
                // Use a custom write options to control memory usage
                let mut write_opts = rocksdb::WriteOptions::default();
                write_opts.set_sync(batch_idx % 5 == 0); // Periodically force sync

                match self.db.batch_insert_hashes(&batch_results.0) {
                    Ok(_) => {
                        info!("Successfully inserted {} records", batch_results.0.len());
                    }
                    Err(e) => {
                        warn!("Database insertion error: {}. Continuing...", e);
                    }
                }
            }

            // Force cleanup of batch results
            drop(batch_results);

            // Check memory after database operations
            let (post_db_mem, _) = self.memory_tracker.update();
            let mem_change = (post_db_mem as i64 - post_mem as i64) / 1024 / 1024;
            info!(
                "Memory after DB operations: {}MB ({}MB change from post-processing)",
                post_db_mem / 1024 / 1024,
                mem_change
            );

            // Perform database maintenance more frequently
            if batch_idx % 5 == 0 && batch_idx > 0 {
                info!("Performing database maintenance...");
                match self.db.flush() {
                    Ok(_) => info!("Database flushed successfully"),
                    Err(e) => warn!("Database flush error: {}", e),
                }

                // Free resources
                // RocksDB doesn't have release_cf() method, commenting out for now
                info!("Column family management done via DB's internal mechanisms");

                // Check memory after maintenance
                let (post_maint_mem, _) = self.memory_tracker.update();
                let maint_change = (post_maint_mem as i64 - post_db_mem as i64) / 1024 / 1024;
                info!(
                    "Memory after DB maintenance: {}MB ({}MB change)",
                    post_maint_mem / 1024 / 1024,
                    maint_change
                );
            }

            // More aggressive cleanup every 10 batches
            if batch_idx % 10 == 0 && batch_idx > 0 {
                info!("Performing full database maintenance...");

                // Compact the database to reclaim space
                self.db.compact_range();
                info!("Database compaction complete");

                // Check memory after compaction
                let (post_compact_mem, _) = self.memory_tracker.update();
                let compact_change = (post_compact_mem as i64 - post_db_mem as i64) / 1024 / 1024;
                info!(
                    "Memory after DB compaction: {}MB ({}MB change)",
                    post_compact_mem / 1024 / 1024,
                    compact_change
                );

                // Force longer pause for system recovery
                std::thread::sleep(std::time::Duration::from_secs(3));
            }

            // Pause between each batch regardless of index
            // This helps prevent resource exhaustion
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // Final database maintenance
        match self.db.flush() {
            Ok(_) => {
                self.db.compact_range();
                info!("Final database maintenance completed successfully");
            }
            Err(e) => warn!("Final database maintenance error: {}. Continuing...", e),
        }

        // Final memory check
        let (final_mem, _) = self.memory_tracker.update();
        info!(
            "Final memory usage: {}MB (peak: {}MB)",
            final_mem / 1024 / 1024,
            self.memory_tracker.peak_mb()
        );

        // Get final counts - only from this run, do NOT add already_processed
        let total_processed = processed_count.load(std::sync::atomic::Ordering::Relaxed);
        let total_successful = successful_count.load(std::sync::atomic::Ordering::Relaxed);
        let total_errors = error_count.load(std::sync::atomic::Ordering::Relaxed);

        // Finalize the progress tracker
        progress.finish(total_successful, total_errors);

        // Gather final database statistics
        let (final_db_count, _) = match self.db.get_db_stats() {
            Ok(stats) => stats,
            Err(_) => (0, 0), // Couldn't determine stats
        };

        let new_entries = final_db_count.saturating_sub(current_db_count);

        // Log final stats to file
        info!("Processing completed:");
        info!("- Total processed: {}", total_processed);
        info!("- Successful: {}", total_successful);
        info!("- Errors: {}", total_errors);
        info!("- New entries in database: {}", new_entries);
        info!("- Peak memory usage: {}MB", self.memory_tracker.peak_mb());

        Ok(self.db.get_db_stats()?)
    }
}

// Helper functions moved to db.rs for better organization
