// Core modules
mod cryptographic;
mod file_validation;
mod hash_processing;
mod timeout_utils;
mod memory_management;
mod batch_processor;
pub mod types;

// Public modules that remain unchanged
pub mod gpu_accelerated;
#[cfg(target_os = "macos")]
pub mod metal_phash;
pub mod perceptual;
pub mod progress;
pub mod process_images; // Keep for backward compatibility

// Expose cryptographic hash calculations
pub use cryptographic::*;

// Expose perceptual hash 
pub use perceptual::{ultra_fast_phash, PHash};

// Expose GPU accelerated functions
pub use gpu_accelerated::phash_from_file as gpu_phash_from_file;
pub use gpu_accelerated::phash_from_img as gpu_phash_from_img;

// Reexport core functionality
pub use batch_processor::{
    process_image_batch, 
    process_images, 
    process_images_in_batches,
};
pub use types::ImageHashResult;
pub use progress::ProgressTracker;

#[cfg(test)]
mod tests;
