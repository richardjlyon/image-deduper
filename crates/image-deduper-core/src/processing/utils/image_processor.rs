use crate::log_hash_error;
use crate::processing::utils::hash_computation_with_timeout::{
    compute_cryptographic_hash_with_timeout, compute_perceptual_hash_with_timeout,
};
use crate::processing::{
    file_validation::{validate_file_exists, validate_file_size},
    ImageHashResult,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicUsize, Arc},
    time::Instant,
};

use log::info;

/// Process a single image
pub fn process_single_image(
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
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }

            processed_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let processed = processed_counter.load(std::sync::atomic::Ordering::Relaxed);
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
            error_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Some(counter) = progress_counter {
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
