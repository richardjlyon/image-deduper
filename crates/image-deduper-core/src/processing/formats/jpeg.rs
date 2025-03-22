use log::info;
use std::path::Path;

use crate::processing::{calculate_phash, file_processing::generate_fallback_hash, types::PHash};

/// Process a JPEG file with corruption recovery
pub fn process_jpeg_image<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    info!("Processing JPEG image");

    // Try to directly open the JPEG file
    let path_ref = path.as_ref();
    match image::open(path_ref) {
        Ok(img) => {
            // Standard processing
            Ok(calculate_phash(&img))
        }
        Err(e) => {
            // Check if this might be a corrupt JPEG
            let error_str = format!("{:?}", e);
            if error_str.contains("first two bytes are not an SOI marker") {
                // Try recovery
                return recover_corrupted_jpeg(path_ref);
            }

            // Return original error
            log::warn!("Failed to recover corrupted JPEG");
            Err(e)
        }
    }
}

/// Attempt to recover a corrupted JPEG file
pub fn recover_corrupted_jpeg<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();

    log::warn!(
        "Attempting to recover corrupted JPEG: {}",
        path_ref.display()
    );

    // Try to find JPEG SOI marker (0xFFD8) in the file
    if let Ok(data) = std::fs::read(path_ref) {
        // Search for JPEG SOI marker (0xFFD8)
        for i in 0..data.len().saturating_sub(1) {
            if data[i] == 0xFF && data[i + 1] == 0xD8 {
                // Found SOI marker, try loading the JPEG from this offset
                if let Ok(img) = image::load_from_memory(&data[i..]) {
                    log::info!(
                        "Recovered JPEG image after skipping {} bytes: {}",
                        i,
                        path_ref.display()
                    );
                    return Ok(calculate_phash(&img));
                }
            }
        }
    }

    // If recovery failed, use fallback hash
    log::warn!(
        "JPEG recovery failed for {}, using fallback hash",
        path_ref.display()
    );
    Ok(generate_fallback_hash(path_ref))
}
