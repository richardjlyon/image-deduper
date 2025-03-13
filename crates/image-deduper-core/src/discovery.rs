use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::types::{ImageFile, ImageFormat};

/// Discover images in the provided directories
pub fn discover_images<P: AsRef<Path>>(
    directories: &[P],
    config: &Config,
) -> Result<Vec<ImageFile>> {
    // Convert to a collection of PathBufs first
    let paths: Vec<PathBuf> = directories
        .iter()
        .map(|dir| dir.as_ref().to_path_buf())
        .collect();

    // Now we can use par_iter on a concrete type
    let image_files: Result<Vec<_>> = paths
        .par_iter()
        .map(|dir| discover_images_in_directory(dir, config))
        .collect::<Vec<Result<Vec<ImageFile>>>>()
        .into_iter()
        .try_fold(Vec::new(), |mut acc, result| {
            acc.extend(result?);
            Ok(acc)
        });

    image_files
}

/// Discover images in a single directory
fn discover_images_in_directory(directory: &Path, config: &Config) -> Result<Vec<ImageFile>> {
    // Check if directory exists
    if !directory.exists() {
        return Err(Error::FileNotFound(directory.to_path_buf()));
    }

    // Determine max depth for directory traversal
    let max_depth = config.max_depth.unwrap_or(std::usize::MAX);

    // Walk directory and collect image files
    let mut image_files = Vec::new();

    for entry in WalkDir::new(directory)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Check if file has an image extension
        if let Some(format) = get_image_format(path) {
            // Skip unsupported formats unless explicitly enabled
            if !format.is_supported() && !config.process_unsupported_formats {
                continue;
            }

            // Get file metadata
            match get_file_metadata(path) {
                Ok((size, last_modified, created)) => {
                    let image_file = ImageFile {
                        path: path.to_path_buf(),
                        size,
                        last_modified,
                        format,
                        created,
                    };

                    image_files.push(image_file);
                }
                Err(e) => {
                    // Log error but continue with other files
                    eprintln!("Error reading metadata for {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(image_files)
}

/// Get image format from file extension
fn get_image_format(path: &Path) -> Option<ImageFormat> {
    let ext_opt = path.extension().and_then(|ext| ext.to_str());

    ext_opt.map(|ext| {
        let format = ImageFormat::from_extension(ext);
        format
    })
}

/// Get file metadata
fn get_file_metadata(
    path: &Path,
) -> io::Result<(u64, std::time::SystemTime, Option<std::time::SystemTime>)> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    let last_modified = metadata.modified()?;
    let created = metadata.created().ok();

    Ok((size, last_modified, created))
}

/// Returns if the given path has an image extension
pub fn is_image_path(path: &Path) -> bool {
    match get_image_format(path) {
        Some(format) => format.is_supported(),
        None => false,
    }
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_test_image(dir: &Path, name: &str, ext: &str) -> PathBuf {
        let file_path = dir.join(format!("{}.{}", name, ext));
        let mut file = File::create(&file_path).unwrap();
        // Write some dummy data to simulate an image
        file.write_all(b"DUMMY IMAGE DATA").unwrap();
        file_path
    }

    fn setup_test_directory() -> (tempfile::TempDir, Vec<PathBuf>) {
        // Create a temporary directory for test files
        let dir = tempdir().unwrap();

        // Create a subdirectory
        let subdir_path = dir.path().join("subdir");
        fs::create_dir(&subdir_path).unwrap();

        // Create various image files
        let files = vec![
            create_test_image(dir.path(), "image1", "jpg"),
            create_test_image(dir.path(), "image2", "png"),
            create_test_image(dir.path(), "image3", "tiff"),
            create_test_image(dir.path(), "image4", "heic"),
            create_test_image(&subdir_path, "subdir_image1", "jpg"),
            create_test_image(&subdir_path, "subdir_image2", "png"),
        ];

        // Create a non-image file
        let non_image_path = dir.path().join("document.txt");
        let mut file = File::create(&non_image_path).unwrap();
        file.write_all(b"NOT AN IMAGE").unwrap();

        (dir, files)
    }

    #[test]
    fn test_is_image_path() {
        assert!(is_image_path(Path::new("test.jpg")));
        assert!(is_image_path(Path::new("test.jpeg")));
        assert!(is_image_path(Path::new("test.png")));
        assert!(is_image_path(Path::new("test.tiff")));
        assert!(is_image_path(Path::new("test.heic")));
        assert!(!is_image_path(Path::new("test.txt")));
        assert!(!is_image_path(Path::new("test")));
    }

    #[test]
    fn test_discover_images_in_directory() {
        let (dir, files) = setup_test_directory();
        let config = Config::default();

        let discovered = discover_images_in_directory(dir.path(), &config).unwrap();

        // We should find all 6 image files (4 in root + 2 in subdir)
        assert_eq!(discovered.len(), 6);

        // Check that we found all the expected files
        let discovered_paths: Vec<PathBuf> = discovered.iter().map(|f| f.path.clone()).collect();
        for file_path in &files {
            assert!(discovered_paths.contains(file_path));
        }

        // Check that the txt file was not included
        assert!(!discovered_paths.contains(&dir.path().join("document.txt")));
    }

    #[test]
    fn test_discover_images_with_depth_limit() {
        let (dir, _) = setup_test_directory();

        // Create config with max_depth of 1 (only root directory)
        let mut config = Config::default();
        config.max_depth = Some(1);

        let discovered = discover_images_in_directory(dir.path(), &config).unwrap();

        // We should only find the 4 image files in the root directory
        assert_eq!(discovered.len(), 4);

        // All discovered files should be directly in the root directory
        for file in &discovered {
            assert_eq!(file.path.parent().unwrap(), dir.path());
        }
    }

    #[test]
    fn test_discover_images_nonexistent_directory() {
        let config = Config::default();
        let result = discover_images_in_directory(Path::new("/path/that/does/not/exist"), &config);

        // Should return a FileNotFound error
        assert!(matches!(result, Err(Error::FileNotFound(_))));
    }

    #[test]
    fn test_discover_images_multiple_directories() {
        // Create two temporary directories
        let (dir1, files1) = setup_test_directory();
        let (dir2, files2) = setup_test_directory();

        let config = Config::default();
        let directories = vec![dir1.path(), dir2.path()];

        let discovered = discover_images(&directories, &config).unwrap();

        // We should find all images from both directories (6 + 6 = 12)
        assert_eq!(discovered.len(), 12);

        // Check that we found all the expected files from both directories
        let discovered_paths: Vec<PathBuf> = discovered.iter().map(|f| f.path.clone()).collect();

        for file_path in files1.iter().chain(files2.iter()) {
            assert!(discovered_paths.contains(file_path));
        }
    }
}
