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
pub use core::{calculate_enhanced_phash, calculate_phash, ultra_fast_phash};

// ----------------------------------

mod crypto_hash;
#[cfg(target_os = "macos")]
mod gpu;

mod utils;

// Public modules that remain unchanged

pub mod perceptual_hash;

// Expose cryptographic hash calculations
pub use crypto_hash::*;

pub use gpu::*;
pub use utils::hash_computation_with_timeout;
pub use utils::*;

#[cfg(test)]
mod tests;
