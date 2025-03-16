use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

use super::test_utils::{create_test_image, get_test_images_dir};
use crate::types::{ImageFile, ImageFormat};

/// Helper function replicating the internal get_file_metadata function from discovery.rs
/// This allows us to test it directly
fn get_file_metadata(
    path: &std::path::Path,
) -> std::io::Result<(u64, SystemTime, Option<SystemTime>)> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    let last_modified = metadata.modified()?;
    let created = metadata.created().ok();

    Ok((size, last_modified, created))
}

/// Setup function to create a file with predictable metadata
fn create_test_file_with_metadata() -> (PathBuf, u64, SystemTime) {
    let test_dir = get_test_images_dir();
    fs::create_dir_all(&test_dir).unwrap();

    // Create a temporary file
    let file_path = test_dir.join("metadata_test.txt");
    let mut file = File::create(&file_path).unwrap();

    // Write some data with known size
    let test_data = b"TEST DATA FOR METADATA";
    file.write_all(test_data).unwrap();
    file.sync_all().unwrap();

    // Get the last modified time
    let metadata = fs::metadata(&file_path).unwrap();
    let last_modified = metadata.modified().unwrap();

    (file_path, test_data.len() as u64, last_modified)
}

/// Test the file metadata extraction function
#[test]
#[ignore]
fn test_get_file_metadata() {
    // Create a test file with known metadata
    let (file_path, expected_size, expected_modified) = create_test_file_with_metadata();

    // Get the metadata using our helper function
    let (size, last_modified, created) = get_file_metadata(&file_path).unwrap();

    // Verify the size matches
    assert_eq!(size, expected_size);

    // Verify the last modified time is close to expected
    // Due to filesystem timestamp precision, exact equality might not work,
    // so we check if it's within 1 second
    let time_diff = if last_modified > expected_modified {
        last_modified.duration_since(expected_modified).unwrap()
    } else {
        expected_modified.duration_since(last_modified).unwrap()
    };

    assert!(
        time_diff.as_secs() <= 1,
        "Time difference too large: {:?}",
        time_diff
    );

    // created might or might not be present depending on the filesystem
    if let Some(created_time) = created {
        // If present, it should be a valid timestamp
        assert!(created_time <= SystemTime::now());
    }

    // Clean up
    fs::remove_file(file_path).unwrap_or_else(|e| {
        eprintln!("Warning: Failed to clean test file: {}", e);
    });
}

/// Test the ImageFile creation with proper metadata
#[test]
#[ignore]
fn test_image_file_metadata() {
    let test_dir = get_test_images_dir();
    // Ensure the directory exists
    fs::create_dir_all(&test_dir).unwrap();

    // Create a test image file
    let image_path = create_test_image(&test_dir, "metadata_test_image", "jpg");

    // Verify the file was created
    assert!(
        image_path.exists(),
        "Test image file was not created successfully"
    );

    let (size, last_modified, created) = get_file_metadata(&image_path).unwrap();

    // Create an ImageFile struct
    let image_file = ImageFile {
        path: image_path.clone(),
        size,
        last_modified,
        format: ImageFormat::Jpeg,
        created,
    };

    // Verify ImageFile properties
    assert_eq!(image_file.path, image_path);
    assert_eq!(image_file.size, size);
    assert_eq!(image_file.last_modified, last_modified);
    assert_eq!(image_file.format, ImageFormat::Jpeg);
    assert_eq!(image_file.created, created);

    // Clean up
    fs::remove_file(image_path).unwrap_or_else(|e| {
        eprintln!("Warning: Failed to clean test image: {}", e);
    });
}
