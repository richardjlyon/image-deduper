//! Perceptual hash module for image similarity detection
//!
//! This module provides perceptual hashing functionality for images,
//! allowing for similarity detection even when images have been resized,
//! compressed, or slightly modified.
//!
mod core;
pub mod file_processing;
pub mod formats;
pub mod platform;
pub mod types;

// Reexport core functionality
pub use batch_processor::{process_image_batch, process_images, process_images_in_batches};
pub use core::{
    calculate_enhanced_phash, calculate_phash, compute_cryptographic, ultra_fast_phash,
};

// ----------------------------------

mod utils;

// Public modules that remain unchanged

pub use utils::hash_computation_with_timeout;
pub use utils::*;

#[cfg(test)]
mod processing_tests;
