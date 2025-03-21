//! GPU-accelerated interface for perceptual hashing
//!
//! This module provides a unified interface for GPU-accelerated perceptual hashing,
//! automatically falling back to CPU when GPU is not available or not enabled.

use crate::processing::file_processing;
use crate::{processing::types::PHash, Config};
use image::DynamicImage;
use std::path::Path;

/// Calculate perceptual hash using CPU implementation (keeping GPU code for future reference)
/// This function always uses the CPU implementation based on benchmarking results
pub fn phash_from_file<P: AsRef<Path>>(
    _config: &Config,
    path: P,
) -> Result<PHash, image::ImageError> {
    // Always use CPU implementation as it's faster in benchmarks
    return file_processing::phash_from_file(path);

    // The code below is retained for future reference but currently disabled
    /*
    // Check if GPU acceleration is enabled in config
    if !config.use_gpu_acceleration {
        // Use standard CPU implementation if GPU is disabled
        return crate::processing::perceptual::phash_from_file(path);
    }

    // Use enhanced GPU implementation on macOS with Metal
    #[cfg(target_os = "macos")]
    {
        // Check image dimensions first to decide on enhanced vs standard hash
        if let Ok(img) = image::image_dimensions(path.as_ref()) {
            let (width, height) = img;

            // For large images, use enhanced hash with GPU acceleration
            if width >= 4096 && height >= 4096 {
                return crate::processing::metal_phash::gpu_phash_from_file(path);
            }

            // For smaller images, use standard hash
            return crate::processing::perceptual::phash_from_file(path);
        } else {
            // Fallback to standard hash if we can't get dimensions
            return crate::processing::perceptual::phash_from_file(path);
        }
    }

    // Use standard CPU implementation on non-macOS platforms
    #[cfg(not(target_os = "macos"))]
    {
        return crate::processing::perceptual::phash_from_file(path);
    }
    */
}

/// Calculate perceptual hash from an image using CPU implementation
/// GPU code is retained for future reference but disabled
pub fn phash_from_img(_config: &Config, img: &DynamicImage) -> PHash {
    // Always use CPU implementation as it's faster in benchmarks
    return crate::processing::perceptual_hash::phash_from_img(img);

    // The code below is retained for future reference but currently disabled
    /*
    // Check if GPU acceleration is enabled in config
    if !config.use_gpu_acceleration {
        // Use CPU implementation if GPU is disabled
        return crate::processing::perceptual::phash_from_img(img);
    }

    // Use GPU implementation with fallback to CPU if available
    #[cfg(target_os = "macos")]
    {
        // Get image dimensions
        let (width, height) = img.dimensions();

        // For very large images, use enhanced hash with GPU acceleration
        if width >= 4096 && height >= 4096 {
            return crate::processing::metal_phash::gpu_accelerated_phash(img);
        }

        // For smaller images, use standard hash
        return crate::processing::perceptual::phash_from_img(img);
    }

    // Use CPU implementation on non-macOS platforms
    #[cfg(not(target_os = "macos"))]
    {
        return crate::processing::perceptual::phash_from_img(img);
    }
    */
}
