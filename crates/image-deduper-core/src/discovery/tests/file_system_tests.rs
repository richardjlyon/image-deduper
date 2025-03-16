use crate::discovery::{discover_images, discover_images_in_directory, has_image_extension};
use crate::{config, Config};
use std::path::{Path, PathBuf};

use super::test_utils::get_or_create_test_images;

fn get_test_image_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_images")
}

/// Tests for the has_image_extension function
#[test]
fn test_is_image_path() {
    assert!(has_image_extension(Path::new("test.jpg")));
    assert!(has_image_extension(Path::new("test.jpeg")));
    assert!(has_image_extension(Path::new("test.png")));
    assert!(has_image_extension(Path::new("test.tiff")));
    assert!(has_image_extension(Path::new("test.heic")));
    assert!(!has_image_extension(Path::new("test.txt")));
    assert!(!has_image_extension(Path::new("test")));
}

/// Test discovering images in a directory
#[test]
fn test_discover_images_in_directory() {
    // Get or create test images directory
    let test_image_base = get_test_image_dir();
    let test_dir = test_image_base.join("heic/IMG-2624x3636");
    let config = Config::default();
    let results = discover_images_in_directory(&test_dir, &config).unwrap();

    // Verify we found the expected number of images (4 in the base directory)
    assert_eq!(results.len(), 22);

    // Check that we only found image files (not text files)
    for image in &results {
        assert!(image.format.is_supported());
    }
}

/// Test the max_depth configuration
#[test]
fn test_discover_images_with_depth_limit() {
    // Get or create test images directory
    let test_dir = get_or_create_test_images();

    // Create a config with depth limit of 1 (no subdirectories)
    let config = config::Config {
        max_depth: Some(1),
        ..Default::default()
    };

    let results = discover_images_in_directory(&test_dir, &config).unwrap();

    // Count images in the base directory vs. subdirectory
    let sub_dir_path = test_dir.join("subdirectory");

    let base_dir_images = results
        .iter()
        .filter(|img| img.path.parent().unwrap() == test_dir)
        .count();

    let sub_dir_images = results
        .iter()
        .filter(|img| img.path.parent().unwrap() == sub_dir_path)
        .count();

    // We should have found images in the base directory
    assert!(base_dir_images >= 4);

    // But no images from subdirectories due to depth limit
    assert_eq!(sub_dir_images, 0);
}

/// Test discovering images from multiple directories
#[test]
fn test_discover_images_multiple_directories() {
    // Discover images in both directories

    let test_image_base = get_test_image_dir();

    let first_dir = test_image_base.join("heic/IMG-2624x3636");
    let second_dir = test_image_base.join("jpg/IMG-2624x3636");
    let directories = vec![first_dir, second_dir];
    let config = Config::default();

    let results = discover_images(&directories, &config).unwrap();

    assert_eq!(results.len(), 44);
}

/// Test handling of nonexistent directories
#[test]
fn test_discover_images_nonexistent_directory() {
    // Get a path to a nonexistent directory
    let nonexistent_dir = Path::new("/nonexistent/directory");
    let config = Config::default();

    // Should return an error
    let result = discover_images_in_directory(nonexistent_dir, &config);
    assert!(result.is_err());
}
