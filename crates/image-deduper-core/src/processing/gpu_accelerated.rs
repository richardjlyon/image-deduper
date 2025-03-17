//! GPU-accelerated interface for perceptual hashing
//!
//! This module provides a unified interface for GPU-accelerated perceptual hashing,
//! automatically falling back to CPU when GPU is not available or not enabled.

use std::path::Path;
use image::DynamicImage;
use crate::processing::perceptual::PHash;
use crate::Config;

/// Calculate perceptual hash using GPU if available and enabled in config
pub fn phash_from_file<P: AsRef<Path>>(config: &Config, path: P) -> Result<PHash, image::ImageError> {
    // Check if GPU acceleration is enabled in config
    if !config.use_gpu_acceleration {
        // Use CPU implementation if GPU is disabled
        return crate::processing::perceptual::phash_from_file(path);
    }
    
    // Use GPU implementation with fallback to CPU if available
    #[cfg(target_os = "macos")]
    {
        return crate::processing::metal_phash::gpu_phash_from_file(path);
    }
    
    // Use CPU implementation on non-macOS platforms
    #[cfg(not(target_os = "macos"))]
    {
        return crate::processing::perceptual::phash_from_file(path);
    }
}

/// Calculate perceptual hash from an image using GPU if available and enabled in config
pub fn phash_from_img(config: &Config, img: &DynamicImage) -> PHash {
    // Check if GPU acceleration is enabled in config
    if !config.use_gpu_acceleration {
        // Use CPU implementation if GPU is disabled
        return crate::processing::perceptual::phash_from_img(img);
    }
    
    // Use GPU implementation with fallback to CPU if available
    #[cfg(target_os = "macos")]
    {
        return crate::processing::metal_phash::gpu_accelerated_phash(img);
    }
    
    // Use CPU implementation on non-macOS platforms
    #[cfg(not(target_os = "macos"))]
    {
        return crate::processing::perceptual::phash_from_img(img);
    }
}