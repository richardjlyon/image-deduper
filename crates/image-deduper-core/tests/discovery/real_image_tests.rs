mod tests {
    use std::path::{Path, PathBuf};

    // Import the items from the crate that are being tested
    use image_deduper_core::discovery::{discover_images_in_directory, has_image_extension};
    use image_deduper_core::Config;

    // Constants for test image directory
    const TEST_IMAGES_DIR: &str = "test_images";

    // Get the path to the test images directory
    fn get_test_images_dir() -> PathBuf {
        PathBuf::from(TEST_IMAGES_DIR)
    }

    // Skip test if images aren't available
    fn skip_if_missing_test_images() -> Option<PathBuf> {
        let test_dir = get_test_images_dir();
        if !test_dir.exists() {
            println!("Test directory not found, skipping test. Run create_test_images first.");
            return None;
        }
        Some(test_dir)
    }

    #[test]
    fn test_is_image_path_with_real_files() {
        // This test doesn't actually require real files, but we'll keep it consistent
        let test_dir = match skip_if_missing_test_images() {
            Some(dir) => dir,
            None => return,
        };

        // Test with real image files
        assert!(has_image_extension(&test_dir.join("image1.jpg")));
        assert!(has_image_extension(&test_dir.join("image2.png")));
        assert!(has_image_extension(&test_dir.join("image3.tiff")));

        // Test with non-image files
        assert!(!has_image_extension(&test_dir.join("document.txt")));

        // Test with non-existent file with an image extension
        assert!(has_image_extension(&test_dir.join("non_existent.jpg")));
    }

    #[test]
    fn test_discover_images_with_real_files() {
        let test_dir = match skip_if_missing_test_images() {
            Some(dir) => dir,
            None => return,
        };

        let config = Config::default();

        // Test discovering images
        let discovered = discover_images_in_directory(&test_dir, &config).unwrap();

        // Print discovered files for debugging
        println!("Discovered {} files:", discovered.len());
        for (i, file) in discovered.iter().enumerate() {
            println!("  {}: {} ({} bytes)", i + 1, file.path.display(), file.size);
        }

        // We should find 6 images (4 in root + 2 in subdir)
        assert_eq!(discovered.len(), 6);

        // Check specific files were found
        let paths: Vec<PathBuf> = discovered.iter().map(|f| f.path.clone()).collect();
        assert!(paths.contains(&test_dir.join("image1.jpg")));
        assert!(paths.contains(&test_dir.join("image2.png")));
        assert!(paths.contains(&test_dir.join("image3.tiff")));
        assert!(paths.contains(&test_dir.join("image4.heic")));

        // Check that txt file was not included
        assert!(!paths.contains(&test_dir.join("document.txt")));

        // Verify we found the images in the subdirectory too
        assert!(paths.contains(&test_dir.join("subdir/subdir_image1.jpg")));
        assert!(paths.contains(&test_dir.join("subdir/subdir_image2.png")));

        // Test max_depth feature
        let mut limited_config = config.clone();
        limited_config.max_depth = Some(1);

        let limited_discovered = discover_images_in_directory(&test_dir, &limited_config).unwrap();
        let limited_paths: Vec<PathBuf> =
            limited_discovered.iter().map(|f| f.path.clone()).collect();

        // Subdirectory images should not be included with depth=1
        assert!(!limited_paths.contains(&test_dir.join("subdir/subdir_image1.jpg")));
        assert!(!limited_paths.contains(&test_dir.join("subdir/subdir_image2.png")));
    }
}
