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
pub fn discover_images_in_directory(directory: &Path, config: &Config) -> Result<Vec<ImageFile>> {
    // Check if directory exists
    if !directory.exists() {
        return Err(Error::FileNotFound(directory.to_path_buf()));
    }

    // Determine max depth for directory traversal
    let max_depth = config.max_depth.unwrap_or(usize::MAX);

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

    ext_opt.map(ImageFormat::from_extension)
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
pub fn has_image_extension(path: &Path) -> bool {
    println!("->> DEBUG: {}", path.display());

    match get_image_format(path) {
        Some(format) => format.is_supported(),
        None => false,
    }
}
