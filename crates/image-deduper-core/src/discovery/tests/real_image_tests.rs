use std::fs;
use std::path::PathBuf;

use super::test_utils::{get_real_images_dir, get_test_images_dir, setup_test_images};
use crate::config::Config;
use crate::discovery::discover_images_in_directory;
use crate::types::ImageFormat;

/// Skip a test if the real images directory is not available
fn skip_if_real_images_missing() -> bool {
    let real_images_dir = get_real_images_dir();
    if !real_images_dir.exists()
        || fs::read_dir(&real_images_dir)
            .map(|d| d.count())
            .unwrap_or(0)
            == 0
    {
        println!("Skipping test: real images directory not found or empty");
        return true;
    }
    false
}

/// Setup function that ensures real test images, or fallback to generated ones
fn setup_real_test_images() -> PathBuf {
    let real_images_dir = get_real_images_dir();

    // If real images directory exists and is not empty, use it
    if real_images_dir.exists()
        && fs::read_dir(&real_images_dir)
            .map(|d| d.count())
            .unwrap_or(0)
            > 0
    {
        return real_images_dir;
    }

    // Otherwise, fall back to creating synthetic test images
    setup_test_images();
    get_test_images_dir()
}

/// Test whether file paths are recognized as images with real files
#[test]
fn test_is_image_path_with_real_files() {
    // Skip test if real images directory is not available
    if skip_if_real_images_missing() {
        return;
    }

    let real_images_dir = get_real_images_dir();
    let mut found_image = false;

    for entry in fs::read_dir(real_images_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        // Only check files
        if path.is_file() {
            let is_image = match path.extension() {
                Some(ext) => {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    ["jpg", "jpeg", "png", "tiff", "tif", "heic"].contains(&ext_str.as_str())
                }
                None => false,
            };

            if is_image {
                found_image = true;
                // Verify the image path is recognized as an image
                assert!(
                    crate::discovery::is_image_path(&path),
                    "Failed to recognize real image: {:?}",
                    path
                );
            }
        }
    }

    // Ensure we actually found and tested at least one image
    assert!(found_image, "No real image files found for testing");
}

/// Test discovering images with real files
#[test]
fn test_discover_images_with_real_files() {
    let test_dir = setup_real_test_images();

    let config = Config::default();
    let result = discover_images_in_directory(&test_dir, &config);

    assert!(
        result.is_ok(),
        "Failed to discover images: {:?}",
        result.err()
    );
    let images = result.unwrap();

    // We should find at least one image
    assert!(!images.is_empty(), "No images found in the test directory");

    // Verify all discovered images have appropriate formats
    for image in &images {
        assert!(
            matches!(
                image.format,
                ImageFormat::Jpeg
                    | ImageFormat::Png
                    | ImageFormat::Tiff
                    | ImageFormat::Heic
                    | ImageFormat::Other(_)
            ),
            "Invalid image format detected: {:?}",
            image.format
        );

        // Verify all paths exist
        assert!(
            image.path.exists(),
            "Discovered image path does not exist: {:?}",
            image.path
        );
    }
}
