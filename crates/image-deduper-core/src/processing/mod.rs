// Core modules
mod batch_processor;
mod crypto_hash;
mod file_validation;
mod hash_processing;
mod memory_management;
mod timeout_utils;
pub mod types;

// Public modules that remain unchanged
pub mod gpu_accelerated;
#[cfg(target_os = "macos")]
pub mod metal_phash;
pub mod perceptual_hash;
pub mod process_images;
pub mod progress; // Keep for backward compatibility

// Expose cryptographic hash calculations
pub use crypto_hash::*;

// Expose perceptual hash
pub use perceptual_hash::{ultra_fast_phash, PHash};

// Expose GPU accelerated functions
pub use gpu_accelerated::phash_from_file as gpu_phash_from_file;
pub use gpu_accelerated::phash_from_img as gpu_phash_from_img;

// Reexport core functionality
pub use batch_processor::{process_image_batch, process_images, process_images_in_batches};
pub use progress::ProgressTracker;
pub use types::ImageHashResult;

#[cfg(test)]
mod tests;
