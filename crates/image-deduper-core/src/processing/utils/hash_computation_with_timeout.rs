use crate::log_hash_error;
use crate::processing::file_processing::phash_from_file;
use crate::processing::formats::tiff::process_tiff_directly;
use crate::processing::{compute_cryptographic, types::PHash};
use blake3::Hash as Blake3Hash;
use log::info;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use super::file_validation::get_file_extension;
// use super::perceptual_hash::{phash_from_file, process_tiff_directly, PHash};
use super::{execute_with_timeout, extract_panic_info, get_timeout_duration, HashOperation};
use crate::error::Result;

// Global skip list for problematic files that consistently cause timeouts
// This list persists across multiple function calls to prevent repeated timeouts
static PROBLEMATIC_FILES: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Check if a file is known to be problematic
pub fn is_problematic(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_string();
    if let Ok(skip_list) = PROBLEMATIC_FILES.lock() {
        skip_list.contains(&path_str)
    } else {
        false
    }
}

/// Mark a file as problematic for future reference
pub fn mark_as_problematic(path: &Path) {
    let path_str = path.to_string_lossy().to_string();
    if let Ok(mut skip_list) = PROBLEMATIC_FILES.lock() {
        skip_list.insert(path_str.clone());
        info!(
            "Added {} to problematic files skip list (now {} entries)",
            path_str,
            skip_list.len()
        );
    }
}

/// Compute cryptographic hash with timeout protection
pub fn compute_cryptographic_hash_with_timeout(path: &Path) -> Result<Blake3Hash> {
    let file_ext = get_file_extension(path);
    let timeout = get_timeout_duration(&file_ext, HashOperation::Cryptographic);
    let path_display = path.display().to_string(); // Store for use in closures
    let path_copy = path.to_path_buf(); // Clone for thread safety

    // Run the hash computation with timeout
    // We need to clone again for the inner closure
    let path_inner = path_copy.clone();
    let result = std::panic::catch_unwind(move || {
        execute_with_timeout(&path_copy, "Crypto hash", timeout, move || {
            compute_cryptographic(&path_inner)
        })
    });

    // Handle panic cases
    match result {
        Ok(hash_result) => hash_result?,
        Err(panic_err) => {
            // Log panic information
            let panic_msg = extract_panic_info(panic_err);
            info!(
                "PANIC during crypto hash for '{}': {}",
                path_display, panic_msg
            );

            // Log the panic properly
            let err = std::io::Error::new(std::io::ErrorKind::Other, panic_msg);
            log_hash_error!(path, &err);

            Err(err.into())
        }
    }
}

/// Compute perceptual hash with timeout protection
pub fn compute_perceptual_hash_with_timeout(path: &Path) -> Result<PHash> {
    // Save display path for logging
    let path_display = path.display().to_string();
    let file_ext = get_file_extension(path);

    // Check if this file is in the skip list from previous timeouts
    if is_problematic(path) {
        // Create a filename-based hash instead for previously problematic files
        log::info!("Skipping known problematic file: {}", path_display);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Skipped known problematic file",
        )
        .into());
    }

    // Set timeout based on file extension
    let timeout = get_timeout_duration(&file_ext, HashOperation::Perceptual);

    // Copy path for thread safety
    let path_copy = path.to_path_buf();

    // Create an additional copy of display string for the closure
    let path_display_clone = path_display.clone();

    // Run the hash computation with timeout protection
    let result = std::panic::catch_unwind(move || {
        // Special case for TIFF files
        if file_ext == "tif" || file_ext == "tiff" {
            // Use specialized handler with detailed logging
            if let Ok(metadata) = std::fs::metadata(&path_copy) {
                let file_size = metadata.len() / 1_000_000; // Convert to MB

                if file_size > 100 {
                    log::info!(
                        "Using specialized TIFF handler for large ({}MB) file: {}",
                        file_size,
                        path_display_clone
                    );
                } else {
                    log::info!("Using specialized TIFF handler for: {}", path_display_clone);
                }
            }

            // Clone again for the inner closure
            let path_inner = path_copy.clone();
            execute_with_timeout(&path_copy, "TIFF processing", timeout, move || {
                process_tiff_directly(&path_inner)
            })
        } else {
            // Normal processing for regular files
            // Clone again for the inner closure
            let path_inner = path_copy.clone();
            execute_with_timeout(&path_copy, "Perceptual hash", timeout, move || {
                phash_from_file(&path_inner)
            })
        }
    });

    // Handle panic cases
    match result {
        Ok(hash_result) => {
            match hash_result {
                Ok(hash) => Ok(hash?), // Use ? to unwrap the Result<PHash, ImageError>
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::TimedOut {
                        // Add file to problematic list if it timed out
                        mark_as_problematic(path);
                    }
                    Err(e.into())
                }
            }
        }
        Err(panic_err) => {
            // Extract panic information if possible
            let panic_msg = extract_panic_info(panic_err);

            info!(
                "PANIC during perceptual hash for '{}': {}",
                path_display, panic_msg
            );

            // Log the panic properly
            let err = std::io::Error::new(std::io::ErrorKind::Other, panic_msg);
            log_hash_error!(path, &err);

            Err(err.into())
        }
    }
}
