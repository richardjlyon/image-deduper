//! This module provides utility functions for file validation and processing.
//! It includes functions to check if a file exists, validate its size, and retrieve its extension.

use crate::log_file_error;
use log::info;
use std::path::Path;

/// Check if a file exists and can be accessed
pub fn validate_file_exists<P: AsRef<Path>>(path: P) -> Option<std::fs::Metadata> {
    let path_ref = path.as_ref();

    // Check if file exists
    if !path_ref.exists() {
        // Log the error
        log_file_error!(
            path_ref,
            "check_exists",
            &std::io::Error::new(std::io::ErrorKind::NotFound, "File does not exist")
        );

        return None;
    }

    // Get file metadata
    match std::fs::metadata(path_ref) {
        Ok(metadata) => Some(metadata),
        Err(e) => {
            // Log metadata error
            log_file_error!(path_ref, "metadata", &e);

            None
        }
    }
}

/// Check if file has valid size (not zero and not too large)
pub fn validate_file_size(path: &Path, metadata: &std::fs::Metadata) -> bool {
    let file_size = metadata.len();
    let path_display = path.display().to_string();

    // Check for zero-sized files
    if file_size == 0 {
        // Log zero-sized file
        log_file_error!(
            path,
            "check_size",
            &std::io::Error::new(std::io::ErrorKind::InvalidData, "Zero-sized file")
        );

        return false;
    }

    // For very large files, just log but don't skip
    if file_size > 200_000_000 {
        // 200MB
        info!(
            "Processing large file ({}MB) with resize optimization: '{}'",
            file_size / 1_000_000,
            path_display
        );
    }

    // Handle specialized formats like TIFF
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        if ext_str == "tif" || ext_str == "tiff" {
            if file_size > 100_000_000 {
                // Large TIFF gets special handling through size checks already implemented in perceptual.rs
                info!(
                    "Processing large TIFF file ({}MB) with specialized handler: '{}'",
                    file_size / 1_000_000,
                    path_display
                );
            }
        }
    }

    true
}

/// Get file extension as lowercase string
pub fn get_file_extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}
