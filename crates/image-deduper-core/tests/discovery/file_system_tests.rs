#[test]
fn test_is_image_path() {
    use image_deduper_core::discovery::has_image_extension;
    use std::path::Path;

    assert!(has_image_extension(Path::new("test.jpg")));
    assert!(has_image_extension(Path::new("test.jpeg")));
    assert!(has_image_extension(Path::new("test.png")));
    assert!(has_image_extension(Path::new("test.tiff")));
    assert!(has_image_extension(Path::new("test.heic")));
    assert!(!has_image_extension(Path::new("test.txt")));
    assert!(!has_image_extension(Path::new("test")));
}

#[test]
fn test_discover_images_in_directory() {
    // use super::super::common;
    use super::super::common::test::get_test_images_dir;
    use image_deduper_core::discovery::discover_images_in_directory;
    use image_deduper_core::Config;
    use std::path::PathBuf;

    // Skip if test directory doesn't exist
    let test_dir = get_test_images_dir();
    if !test_dir.exists() {
        println!("Test directory not found, skipping test. Run create_test_images first.");
        return;
    }

    let config = Config::default();
    let discovered = discover_images_in_directory(&test_dir, &config).unwrap();

    // We should find all 6 image files (4 in root + 2 in subdir)
    assert_eq!(discovered.len(), 6);

    // Check that specific files were found
    let discovered_paths: Vec<PathBuf> = discovered.iter().map(|f| f.path.clone()).collect();

    assert!(discovered_paths.contains(&test_dir.join("image1.jpg")));
    assert!(discovered_paths.contains(&test_dir.join("image2.png")));
    assert!(discovered_paths.contains(&test_dir.join("image3.tiff")));
    assert!(discovered_paths.contains(&test_dir.join("image4.heic")));
    assert!(discovered_paths.contains(&test_dir.join("subdir/subdir_image1.jpg")));
    assert!(discovered_paths.contains(&test_dir.join("subdir/subdir_image2.png")));

    // Check that the txt file was not included
    assert!(!discovered_paths.contains(&test_dir.join("document.txt")));
}

#[test]
fn test_discover_images_with_depth_limit() {
    use super::super::common::test::get_test_images_dir;
    use image_deduper_core::discovery::discover_images_in_directory;
    use image_deduper_core::Config;

    // Skip if test directory doesn't exist
    let test_dir = get_test_images_dir();
    if !test_dir.exists() {
        println!("Test directory not found, skipping test. Run create_test_images first.");
        return;
    }

    // Create config with max_depth of 1 (only root directory)
    let config = Config {
        max_depth: Some(1),
        ..Default::default()
    };

    let discovered = discover_images_in_directory(&test_dir, &config).unwrap();

    // We should only find the 4 image files in the root directory
    assert_eq!(discovered.len(), 4);

    // All discovered files should be directly in the root directory
    for file in &discovered {
        assert_eq!(file.path.parent().unwrap(), test_dir);
    }
}

#[test]
fn test_discover_images_multiple_directories() {
    use super::super::common::test::{get_test_images_dir, get_test_images_subdir};
    use image_deduper_core::discovery::discover_images;
    use image_deduper_core::Config;
    use std::path::PathBuf;

    // Skip if test directory doesn't exist
    let test_dir1 = get_test_images_dir();
    if !test_dir1.exists() {
        println!("Test directory not found, skipping test. Run create_test_images first.");
        return;
    }

    // For testing multiple directories, use the subdirectory as a second directory
    let test_dir2 = get_test_images_subdir();

    let config = Config::default();
    let directories = vec![test_dir1.clone(), test_dir2.clone()];

    let discovered = discover_images(&directories, &config).unwrap();

    // We should find all images (4 in root + 2 in subdir, but when we include subdir directly
    // we're going to get the 2 subdir images counted twice)
    assert_eq!(discovered.len(), 8);

    // Check that specific files were found
    let discovered_paths: Vec<PathBuf> = discovered.iter().map(|f| f.path.clone()).collect();

    assert!(discovered_paths.contains(&test_dir1.join("image1.jpg")));
    assert!(discovered_paths.contains(&test_dir1.join("image2.png")));
    assert!(discovered_paths.contains(&test_dir1.join("image3.tiff")));
    assert!(discovered_paths.contains(&test_dir1.join("image4.heic")));
    assert!(discovered_paths.contains(&test_dir1.join("subdir/subdir_image1.jpg")));
    assert!(discovered_paths.contains(&test_dir1.join("subdir/subdir_image2.png")));
    // The same subdirectory images but accessed directly via test_dir2
    assert!(discovered_paths.contains(&test_dir2.join("subdir_image1.jpg")));
    assert!(discovered_paths.contains(&test_dir2.join("subdir_image2.png")));
}

#[test]
fn test_discover_images_nonexistent_directory() {
    use image_deduper_core::discovery::discover_images_in_directory;
    use image_deduper_core::Config;
    use image_deduper_core::Error;
    use std::path::Path;

    let config = Config::default();
    let result = discover_images_in_directory(Path::new("/path/that/does/not/exist"), &config);

    // Should return a FileNotFound error
    assert!(matches!(result, Err(Error::FileNotFound(_))));
}
