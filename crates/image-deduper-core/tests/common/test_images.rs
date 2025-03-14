use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Create a test image with dummy data
pub fn create_test_image(dir: &Path, name: &str, ext: &str) -> PathBuf {
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

    // Create a subdirectory
    let subdir_path = base_dir.join("subdir");
    fs::create_dir_all(&subdir_path).unwrap();

    // Create various image files
    let files = vec![
        create_test_image(base_dir, "image1", "jpg"),
        create_test_image(base_dir, "image2", "png"),
        create_test_image(base_dir, "image3", "tiff"),
        create_test_image(base_dir, "image4", "heic"),
        create_test_image(&subdir_path, "subdir_image1", "jpg"),
        create_test_image(&subdir_path, "subdir_image2", "png"),
    ];

    // Create a non-image file for testing exclusion
    let non_image_path = base_dir.join("document.txt");
    let mut file = File::create(&non_image_path).unwrap();
    file.write_all(b"NOT AN IMAGE").unwrap();

    println!("Created test images in: {}", base_dir.display());
    for file in &files {
        println!("  - {}", file.display());
    }

    files
}

/// Create test images in a directory, removing any existing test images first
pub fn setup_test_images(test_dir: &Path) -> Vec<PathBuf> {
    // Delete the directory if it exists
    if test_dir.exists() {
        println!("Removing existing directory...");
        fs::remove_dir_all(test_dir).unwrap();
    }

    // Create the test images
    let files = create_test_images(test_dir);

    println!("Done! Test images created successfully.");

    files
}
