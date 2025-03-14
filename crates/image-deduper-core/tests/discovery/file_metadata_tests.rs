mod tests {
    use std::env;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::SystemTime;

    // Import required types from the crate
    use image_deduper_core::types::{ImageFile, ImageFormat};

    // Constants for test image creation
    const TEST_IMG_DIR: &str = "test_images";

    /// Helper function to get the path to the test images directory
    fn get_test_image_dir() -> PathBuf {
        let mut path = env::current_dir().unwrap();
        path.push("tests");
        path.push(TEST_IMG_DIR);
        path
    }

    /// Setup function to create a directory with real test images
    fn setup_real_test_images() -> PathBuf {
        let test_dir = get_test_image_dir();

        // Create the directory if it doesn't exist
        if !test_dir.exists() {
            fs::create_dir_all(&test_dir).unwrap();
        }

        test_dir
    }

    // Implementation of get_file_metadata for testing
    fn get_file_metadata(
        path: &std::path::Path,
    ) -> std::io::Result<(u64, std::time::SystemTime, Option<std::time::SystemTime>)> {
        println!("Getting metadata for: {}", path.display());
        match fs::metadata(path) {
            Ok(metadata) => {
                println!("Successfully got metadata");
                let size = metadata.len();
                let last_modified = metadata.modified()?;
                let created = metadata.created().ok();
                Ok((size, last_modified, created))
            }
            Err(e) => {
                println!("Error getting metadata: {}", e);
                Err(e)
            }
        }
    }

    #[test]
    fn test_get_file_metadata() {
        let test_dir = setup_real_test_images();
        println!("Test directory: {}", test_dir.display());

        // Ensure the directory exists
        fs::create_dir_all(&test_dir).unwrap();
        assert!(
            fs::metadata(&test_dir).is_ok(),
            "Test directory doesn't exist after creation"
        );

        let test_file = test_dir.join("metadata_test.txt");
        println!("Test file path: {}", test_file.display());

        // Create a file with known content size
        let test_content = "This is exactly 30 bytes of text.";
        let mut file = File::create(&test_file).unwrap();
        file.write_all(test_content.as_bytes()).unwrap();

        // Sync file to ensure metadata is updated
        file.sync_all().unwrap();

        // Verify the file exists using Path::exists
        let path = std::path::Path::new(&test_file);
        println!("Test file exists: {}", path.exists());
        assert!(path.exists(), "Test file doesn't exist after creation");

        // Get creation time right after creating the file
        let approx_creation_time = std::time::SystemTime::now();

        // Test the function
        println!("About to call get_file_metadata on {}", test_file.display());
        let result = get_file_metadata(&test_file).unwrap();
        println!("Successfully got file metadata");
        let (size, last_modified, created) = result;

        // Test file size
        assert_eq!(
            size,
            test_content.len() as u64,
            "File size should match content length"
        );

        // Test last_modified time (should be recent)
        let duration_since_modified = std::time::SystemTime::now()
            .duration_since(last_modified)
            .unwrap();
        assert!(
            duration_since_modified.as_secs() < 5,
            "Modified time should be very recent"
        );

        // Test created time if available (platform-dependent)
        if let Some(creation_time) = created {
            let duration_since_creation = std::time::SystemTime::now()
                .duration_since(creation_time)
                .unwrap();
            assert!(
                duration_since_creation.as_secs() < 5,
                "Creation time should be recent"
            );

            // Verify creation time is close to our approximate time
            let time_diff = if approx_creation_time > creation_time {
                approx_creation_time.duration_since(creation_time).unwrap()
            } else {
                creation_time.duration_since(approx_creation_time).unwrap()
            };

            assert!(
                time_diff.as_secs() < 5,
                "Creation time should be close to when we created the file"
            );
        } else {
            // On platforms where creation time isn't available, we should print a note
            println!("Note: File creation time not available on this platform");
        }

        // Test with non-existent file
        let non_existent = test_dir.join("doesnt_exist.txt");
        let error_result = get_file_metadata(&non_existent);
        assert!(
            error_result.is_err(),
            "Should return error for non-existent file"
        );

        // Clean up
        fs::remove_file(test_file).unwrap();
    }

    #[test]
    fn test_image_file_metadata() {
        let test_dir = setup_real_test_images();
        println!("->>Test directory: {}", test_dir.display());
        let jpg_path = test_dir.join("andromeda.jpg");

        // Ensure test image exists
        assert!(jpg_path.exists(), "Test image should exist");

        // Get metadata
        let (size, last_modified, created) = get_file_metadata(&jpg_path).unwrap();

        // The size of the generated test image should be non-zero
        assert!(size > 0, "Image file size should be non-zero");

        // Convert ImageFile type
        let image_file = ImageFile {
            path: jpg_path.clone(),
            size,
            last_modified,
            format: ImageFormat::Jpeg,
            created,
        };

        // test that the created date is reasonable
        assert!(image_file.created.unwrap() < SystemTime::now());

        // Don't perform cleanup if the test might run again
        // This helps prevent the trigger-loop with Bacon
        if option_env!("BACON").is_none() {
            // Only clean up if not running under Bacon
            // cleanup_test_images(); // Uncomment if needed
        }
    }
}
