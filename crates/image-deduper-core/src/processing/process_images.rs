// This is a compatibility module that forwards to the new implementation
// It provides the same API as before, but delegates to the new code structure

use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use super::utils::batch_processor::{
    process_image_batch as new_process_image_batch, process_images as new_process_images,
    process_images_in_batches as new_process_images_in_batches,
};

// Use rather than re-export
use super::types::ImageHashResult;

// Make the type public (but no need to re-export it since it's already exported from mod.rs)
pub type ImageHashResultType = ImageHashResult;

/// Process a batch of images and compute their hashes with error handling
/// Returns a tuple of (successful results, error count)
pub fn process_image_batch(
    paths: &[PathBuf],
    progress_counter: Option<&Arc<AtomicUsize>>,
) -> (Vec<ImageHashResult>, usize) {
    // Forward to the new implementation
    new_process_image_batch(paths, progress_counter, None)
}

/// Simplified batch processor that handles chunking for memory efficiency
pub fn process_images_in_batches(
    images: &[PathBuf],
    batch_size: usize,
    progress_counter: Option<&Arc<AtomicUsize>>,
) -> Vec<ImageHashResult> {
    // Forward to the new implementation
    new_process_images_in_batches(images, batch_size, progress_counter)
}

/// Simple wrapper for backward compatibility
pub fn process_images(images: &[PathBuf]) -> Vec<ImageHashResult> {
    // Forward to the new implementation
    new_process_images(images)
}
