//! This module provides functionality for processing images in batches, computing cryptographic and perceptual hashes
//! for each image, and handling errors during the process. It includes configuration options for batch processing, memory
//! management, and logging.
//!
//! # Structures
//! - `BatchConfig`: Configuration for batch processing, including thread limits and batch sizes.
//!
//! # Functions
//! - `process_single_image`: Processes a single image, computing both cryptographic and perceptual hashes, and handles errors.
//! - `process_image_batch`: Processes a batch of images in parallel, computes their hashes, and returns the results
//!    along with the error count.
//! - `process_images_in_batches`: Processes images in sequential batches to manage memory usage effectively.
//! - `process_images`: A simple wrapper for backward compatibility that processes images using a default batch size.
//!
//! # Usage
//! This module is designed to handle large sets of images efficiently by processing them in batches and using parallel
//! computation where possible. It also includes detailed logging and memory management to ensure smooth operation even with large datasets.
//!
//! # Example
//! ```rust
//! use std::path::PathBuf;
//! use std::sync::Arc;
//! use std::sync::atomic::AtomicUsize;
//!
//! let images: Vec<PathBuf> = vec![
//!     PathBuf::from("image1.jpg"),
//!     PathBuf::from("image2.jpg"),
//!     // Add more image paths
//! ];
//!
//! let progress_counter = Arc::new(AtomicUsize::new(0));
//! let results = process_images_in_batches(&images, 50, Some(&progress_counter));
//!
//! for result in results {
//!     println!("Processed image: {:?}", result.path);
//! }

use crate::log_hash_error;
use log::info;
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::file_validation::{validate_file_exists, validate_file_size};
use super::hash_processing::{
    compute_cryptographic_hash_with_timeout, compute_perceptual_hash_with_timeout,
};
use super::memory_management::MemoryTracker;
use super::types::ImageHashResult;

/// Configuration for batch processing
#[derive(Clone)]
pub struct BatchConfig {
    /// Maximum number of threads to use
    pub thread_limit: usize,
    /// Maximum number of images per batch
    pub batch_size: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            thread_limit: std::cmp::min(num_cpus::get(), 8),
            batch_size: 30,
        }
    }
}

/// Process a single image
fn process_single_image(
    path: &PathBuf,
    error_counter: &Arc<AtomicUsize>,
    processed_counter: &Arc<AtomicUsize>,
    progress_counter: Option<&Arc<AtomicUsize>>,
) -> Option<ImageHashResult> {
    let start = Instant::now();
    let path_display = path.display().to_string();

    // Log startup
    info!("Starting to process: '{}'", path_display);

    // Validate file exists and get metadata
    let metadata = match validate_file_exists(path, error_counter, progress_counter) {
        Some(metadata) => metadata,
        None => return None,
    };

    // Validate file size
    if !validate_file_size(path, &metadata, error_counter, progress_counter) {
        return None;
    }

    // Process cryptographic hash with timeout
    info!("Computing crypto hash for: '{}'", path_display);
    let crypto_result = compute_cryptographic_hash_with_timeout(path);

    // Only compute perceptual hash if crypto hash succeeded
    let phash_result = if crypto_result.is_ok() {
        info!("Computing perceptual hash for: '{}'", path_display);
        compute_perceptual_hash_with_timeout(path)
    } else {
        // Skip perceptual hash if crypto hash failed
        info!(
            "Skipping perceptual hash due to crypto hash failure for '{}'",
            path_display
        );
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Skipped").into())
    };

    // Process results
    match (crypto_result, phash_result) {
        (Ok(blake3), Ok(phash)) => {
            // Increment progress if counter provided
            if let Some(counter) = progress_counter {
                counter.fetch_add(1, Ordering::Relaxed);
            }

            processed_counter.fetch_add(1, Ordering::Relaxed);
            let processed = processed_counter.load(Ordering::Relaxed);
            let elapsed = start.elapsed();

            // Log progress (only for longer operations or periodically)
            if processed % 20 == 0 || elapsed > std::time::Duration::from_secs(3) {
                info!(
                    "Processed: {} - '{}' in {:.2?}",
                    processed, path_display, elapsed
                );
            }

            Some(ImageHashResult {
                path: path.clone(),
                cryptographic: blake3,
                perceptual: phash,
            })
        }
        (crypto_result, phash_result) => {
            // Count error and increment progress
            error_counter.fetch_add(1, Ordering::Relaxed);
            if let Some(counter) = progress_counter {
                counter.fetch_add(1, Ordering::Relaxed);
            }

            // Log crypto hash error
            if let Err(e) = &crypto_result {
                log_hash_error!(path, &format!("{}", e));
                info!("Crypto hash failed for '{}'", path_display);
            }

            // Log perceptual hash error
            if let Err(e) = &phash_result {
                log_hash_error!(path, &format!("{}", e));
                info!("Perceptual hash failed for '{}'", path_display);
            }

            // Log a summary of the failure
            info!("Failed to process: {}", path_display);

            None
        }
    }
}

/// Process a batch of images and compute their hashes with error handling
/// Returns a tuple of (successful results, error count)
pub fn process_image_batch(
    paths: &[PathBuf],
    progress_counter: Option<&Arc<AtomicUsize>>,
    config: Option<BatchConfig>,
) -> (Vec<ImageHashResult>, usize) {
    // Use default config if none provided
    let config = config.unwrap_or_default();

    // Initialize memory tracker
    let mut memory_tracker = MemoryTracker::new();

    info!("Processing batch of {} images...", paths.len());
    memory_tracker.log_memory("batch start");

    let batch_start = Instant::now();

    // Use atomic counters for thread safety
    let error_counter = Arc::new(AtomicUsize::new(0));
    let processed_counter = Arc::new(AtomicUsize::new(0));

    // Configure thread pool
    let thread_limit = config.thread_limit;
    info!("Using {} threads for image processing", thread_limit);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(thread_limit)
        .build()
        .unwrap();

    // Process images in parallel using a controlled thread pool
    let results: Vec<_> = pool.install(|| {
        paths
            .par_iter()
            .map(|path| {
                process_single_image(path, &error_counter, &processed_counter, progress_counter)
            })
            .filter_map(|r| r)
            .collect()
    });

    let batch_duration = batch_start.elapsed();

    // Log final memory and timing stats
    let (end_mem, mem_diff) = memory_tracker.log_memory("batch completion");

    // Log results
    info!(
        "Batch completed: {} successful, {} errors in {:.2?}",
        results.len(),
        error_counter.load(Ordering::Relaxed),
        batch_duration
    );

    // Log more detailed info
    info!(
        "Memory usage: end={}MB, diff=+{}MB",
        end_mem / 1024 / 1024,
        mem_diff / 1024 / 1024
    );

    // Check results size
    let result_estimate = results.len() * std::mem::size_of::<ImageHashResult>();
    info!("Approximate result size: ~{}KB", result_estimate / 1024);

    (results, error_counter.load(Ordering::Relaxed))
}

/// Process images in batches for better memory management
pub fn process_images_in_batches(
    images: &[PathBuf],
    batch_size: usize,
    progress_counter: Option<&Arc<AtomicUsize>>,
) -> Vec<ImageHashResult> {
    use sysinfo::System;

    // Initialize memory tracking
    let mut system = System::new_all();
    system.refresh_memory();
    let start_mem = system.used_memory() / 1024 / 1024; // Convert to MB
    println!("Initial memory usage for batch processing: {}MB", start_mem);

    let total_images = images.len();
    let mut results = Vec::new(); // Don't pre-allocate to avoid excess memory usage
    let mut total_errors = 0;
    let batch_start = std::time::Instant::now();

    // Set up batch configuration
    let config = BatchConfig {
        thread_limit: std::cmp::min(num_cpus::get(), 6),
        batch_size,
    };

    // Process images in sequential batches to control memory usage
    for (i, chunk) in images.chunks(batch_size).enumerate() {
        // Check memory before this batch
        system.refresh_memory();
        let before_batch_mem = system.used_memory() / 1024 / 1024;
        println!("Memory before batch {}: {}MB", i + 1, before_batch_mem);

        // Process this batch of images
        let (batch_results, errors) =
            process_image_batch(chunk, progress_counter, Some(config.clone()));

        // Track errors
        total_errors += errors;

        // Store results but limit memory usage
        let results_to_keep = std::cmp::min(batch_results.len(), 1000);
        let should_store = results.len() < 1000;

        if should_store {
            results.extend(batch_results.into_iter().take(results_to_keep));
        } else {
            // Drop batch_results explicitly when not storing
            drop(batch_results);
        }

        // Log progress
        info!(
            "Processed batch {}/{} ({} images, {} errors)",
            i + 1,
            (total_images + batch_size - 1) / batch_size,
            chunk.len(),
            errors
        );

        // Memory cleanup and pause between batches
        if i % 2 == 0 {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // Periodic full cleanup
        if i % 10 == 0 && i > 0 {
            // Release memory pressure by clearing and shrinking results
            if !results.is_empty() {
                results.clear();
                results.shrink_to_fit();
            }

            std::thread::sleep(std::time::Duration::from_secs(2));
            info!("Performed full memory cleanup after batch {}", i + 1);
        }
    }

    // Final memory check
    system.refresh_memory();
    let end_mem = system.used_memory() / 1024 / 1024;
    let mem_diff = if end_mem > start_mem {
        end_mem - start_mem
    } else {
        0
    };
    let batch_duration = batch_start.elapsed();

    info!(
        "Processing complete: {} successful, {} errors",
        results.len(),
        total_errors
    );
    info!("Total processing time: {:.2?}", batch_duration);
    info!(
        "Final memory usage: before={}MB, after={}MB, diff=+{}MB",
        start_mem, end_mem, mem_diff
    );

    results
}

/// Simple wrapper for backward compatibility
pub fn process_images(images: &[PathBuf]) -> Vec<ImageHashResult> {
    // Use a reasonable batch size to limit memory usage
    const DEFAULT_BATCH_SIZE: usize = 50;

    process_images_in_batches(images, DEFAULT_BATCH_SIZE, None)
}
