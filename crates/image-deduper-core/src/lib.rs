//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

// -- External Dependencies --

use log::{info, warn};
use persistence::diagnose_database;
use rocksdb::DB;
use sysinfo::System;

// -- Standard Library --
use std::path::PathBuf;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc, Mutex,
    },
    time::Instant,
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

/// Memory usage tracker
pub struct MemoryTracker {
    system: Mutex<System>,
    start_memory: u64,
    peak_memory: AtomicUsize,
    last_check: Mutex<Instant>,
}

impl MemoryTracker {
    /// Create a new memory tracker
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let total_used = system.used_memory();

        Self {
            system: Mutex::new(system),
            start_memory: total_used,
            peak_memory: AtomicUsize::new(total_used as usize),
            last_check: Mutex::new(Instant::now()),
        }
    }

    /// Update memory usage statistics and log if significant changes detected
    pub fn update(&self) -> (u64, u64) {
        let mut system = self.system.lock().unwrap();
        system.refresh_memory();

        let current_used = system.used_memory();
        let usage_diff = if current_used > self.start_memory {
            current_used - self.start_memory
        } else {
            0
        };

        // Update peak memory
        let peak = self.peak_memory.load(std::sync::atomic::Ordering::Relaxed) as u64;
        if current_used > peak {
            self.peak_memory
                .store(current_used as usize, std::sync::atomic::Ordering::Relaxed);
        }

        // Only log if enough time has passed since last check
        let mut last_check = self.last_check.lock().unwrap();
        if last_check.elapsed().as_secs() >= 5 {
            // Log memory usage in MB
            info!(
                "Memory usage: current={}MB, diff=+{}MB, peak={}MB",
                current_used / 1024 / 1024,
                usage_diff / 1024 / 1024,
                self.peak_memory.load(std::sync::atomic::Ordering::Relaxed) as u64 / 1024 / 1024
            );
            *last_check = Instant::now();
        }

        (current_used, usage_diff)
    }

    /// Get peak memory usage in MB
    pub fn peak_mb(&self) -> u64 {
        self.peak_memory.load(std::sync::atomic::Ordering::Relaxed) as u64 / 1024 / 1024
    }

    /// Get current memory usage diff in MB
    pub fn current_diff_mb(&self) -> i64 {
        let mut system = self.system.lock().unwrap();
        system.refresh_memory();

        let current_used = system.used_memory();
        ((current_used as i64) - (self.start_memory as i64)) / 1024 / 1024
    }
}

/// Main entry point for the deduplication process
pub struct ImageDeduper {
    config: Config,
    db: DB,
    _safety_manager: safety::SafetyManager,
    _shutdown_requested: Arc<AtomicBool>,
    memory_tracker: Arc<MemoryTracker>,
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
        let memory_tracker = Arc::new(MemoryTracker::new());

        Self {
            config,
            db,
            _safety_manager,
            _shutdown_requested: Arc::new(AtomicBool::new(false)),
            memory_tracker,
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
    pub fn process_images(&self, image_files: &[ImageFile], force_rescan: bool) -> Result<()> {
        // Get current database stats
        let (current_db_count, _) = self.get_db_stats()?;
        info!(
            "Starting with {} images already in database",
            current_db_count
        );

        // Log the exact count of images we're processing
        info!(
            "Processing {} images from supplied collection",
            image_files.len()
        );

        // Perform a database diagnosis
        diagnose_database(&self.db)?;

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
            let new_paths = persistence::filter_new_images(&self.db, &image_paths)?;
            info!("Found {} new images to process", new_paths.len());
            new_paths
        };

        if paths_to_process.is_empty() {
            info!("No new images to process");
            return Ok(());
        }

        // Choose batch size based on available memory
        // Smaller batch size for stability, can be increased later for performance
        const BATCH_SIZE: usize = 50;

        // Create a smaller batch size for more frequent checkpoints
        let effective_batch_size = std::cmp::min(BATCH_SIZE, 20);

        // Get stats for the progress tracker
        let already_processed = current_db_count;
        let total_images = image_files.len();
        let total_batches =
            (paths_to_process.len() + effective_batch_size - 1) / effective_batch_size;

        // Create progress tracker with:
        // - Total = already in DB + total images passed in
        // - Initial position = already in DB
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
                processing::process_image_batch(image_batch, Some(&processed_count));

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

                match persistence::batch_insert_hashes(&self.db, &batch_results.0) {
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
                // Flush memtable to disk
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
                self.db.compact_range::<&[u8], &[u8]>(None, None);
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
        match persistence::maintain_database(&self.db) {
            Ok(_) => info!("Final database maintenance completed successfully"),
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
        let (final_db_count, _) = match self.get_db_stats() {
            Ok(stats) => stats,
            Err(_) => (0, 0), // Couldn't determine stats
        };

        let new_entries = final_db_count.saturating_sub(current_db_count);

        // Log final stats to file
        info!("Processing completed:");
        info!("  - Total processed: {}", total_processed);
        info!("  - Successful: {}", total_successful);
        info!("  - Errors: {}", total_errors);
        info!("  - New entries in database: {}", new_entries);
        info!("  - Peak memory usage: {}MB", self.memory_tracker.peak_mb());

        Ok(())
    }
}

// Helper functions moved to db.rs for better organization
