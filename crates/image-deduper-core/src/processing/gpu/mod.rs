mod gpu_accelerated;
pub mod metal_phash;

pub use gpu_accelerated::phash_from_file as gpu_phash_from_file;
pub use gpu_accelerated::phash_from_img as gpu_phash_from_img;
