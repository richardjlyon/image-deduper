use std::path::PathBuf;

pub mod image_variants;
pub mod test_images;

pub use image_variants::ImageVariant;
pub use test_images::*;

// Constants for test images
const TEST_IMAGES_DIR: &str = "test_images";

// Get the path to the test images directory
pub fn get_test_images_dir() -> PathBuf {
    PathBuf::from(TEST_IMAGES_DIR)
}

// Get the path to the test images subdirectory
pub fn get_test_images_subdir() -> PathBuf {
    get_test_images_dir().join("subdir")
}
