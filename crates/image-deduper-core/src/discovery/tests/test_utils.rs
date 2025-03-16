#![allow(dead_code)]

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Create a test image with dummy data
pub fn create_test_image(dir: &Path, name: &str, ext: &str) -> PathBuf {
    // Ensure the directory exists before creating the file
    fs::create_dir_all(dir).unwrap();

    let file_path = dir.join(format!("{}.{}", name, ext));
    let mut file = File::create(&file_path).unwrap();
    // Write some dummy data to simulate an image
    file.write_all(b"DUMMY IMAGE DATA").unwrap();
    file_path
}

/// Create a set of test images in the specified directory
pub fn create_test_images(base_dir: &Path) -> Vec<PathBuf> {
    // Ensure the base directory exists
    fs::create_dir_all(base_dir).unwrap();

    // Create subdirectories for nested tests
    let sub_dir = base_dir.join("subdirectory");
    fs::create_dir_all(&sub_dir).unwrap();

    // Create test images using iterators and collect
    let created_files = vec![
        // Base directory images
        create_test_image(base_dir, "test_image1", "jpg"),
        create_test_image(base_dir, "test_image2", "png"),
        create_test_image(base_dir, "test_image3", "tiff"),
        create_test_image(base_dir, "test_image4", "heic"),
        create_test_image(base_dir, "not_an_image", "txt"),
        // Subdirectory images
        create_test_image(&sub_dir, "nested_image1", "jpg"),
        create_test_image(&sub_dir, "nested_image2", "png"),
    ];

    created_files
}

/// Get the path to the generated test images directory
pub fn get_test_images_dir() -> PathBuf {
    // Use the new location directly in image-deduper-core
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/generated");

    // Ensure the directory exists
    fs::create_dir_all(&dir).unwrap();

    dir
}

/// Get the path to the real test images directory
pub fn get_real_images_dir() -> PathBuf {
    // Use the new location directly in image-deduper-core
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/real_images");

    // Ensure the directory exists
    fs::create_dir_all(&dir).unwrap();

    dir
}

/// Setup function to create a directory with test images
pub fn setup_test_images() -> PathBuf {
    let test_dir = get_test_images_dir();
    // Clean up any previous test artifacts
    if test_dir.exists() {
        let _ = fs::remove_dir_all(&test_dir);
    }

    // Ensure the directory exists
    fs::create_dir_all(&test_dir).unwrap();

    // Create new test images
    create_test_images(&test_dir);
    test_dir
}

/// Skip test if test images aren't available, creating them if needed
pub fn get_or_create_test_images() -> PathBuf {
    let test_dir = get_test_images_dir();
    if !test_dir.exists() || fs::read_dir(&test_dir).map(|d| d.count()).unwrap_or(0) == 0 {
        setup_test_images()
    } else {
        test_dir
    }
}
