// Core modules
mod crypto_hash;
#[cfg(target_os = "macos")]
mod gpu;

pub mod types;
mod utils;

// Public modules that remain unchanged

pub mod perceptual_hash;
pub mod process_images;

// Expose cryptographic hash calculations
pub use crypto_hash::*;

// Expose perceptual hash
pub use perceptual_hash::{ultra_fast_phash, PHash};

// Reexport core functionality
pub use batch_processor::{process_image_batch, process_images, process_images_in_batches};
pub use gpu::*;
pub use types::ImageHashResult;
pub use utils::hash_computation_with_timeout;
pub use utils::*;

#[cfg(test)]
mod tests;
