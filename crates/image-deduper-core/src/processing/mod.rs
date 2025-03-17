mod cryptographic;
pub mod perceptual;  // Make perceptual module public
mod process_images;
#[cfg(target_os = "macos")]
pub mod metal_phash;
pub mod gpu_accelerated;

pub use cryptographic::*;
pub use perceptual::{PHash, ultra_fast_phash};  // Only export what won't conflict
pub use process_images::*;
pub use gpu_accelerated::phash_from_file as gpu_phash_from_file;
pub use gpu_accelerated::phash_from_img as gpu_phash_from_img;

#[cfg(test)]
mod tests;
