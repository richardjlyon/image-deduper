pub mod test_image_registry;
pub use test_image_registry::*;

#[cfg(test)]
pub mod test {
    use std::path::PathBuf;

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
}
