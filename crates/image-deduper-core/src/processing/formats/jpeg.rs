use std::path::Path;

use image::GenericImageView;

use crate::processing::{calculate_phash, file_processing::generate_fallback_hash, types::PHash};

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

/// Process a JPEG file with optimizations
pub fn process_jpeg_image<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();

    // Try to directly open the JPEG file
    match image::open(path_ref) {
        Ok(img) => {
            let (width, height) = img.dimensions();

            // Apply special handling for large JPEGs
            if width > 1024 || height > 1024 {
                log::info!(
                    "Downscaling large JPEG ({}x{}) for hash computation: {}",
                    width,
                    height,
                    path_ref.display()
                );

                // Calculate target dimensions maintaining aspect ratio
                let (target_width, target_height) = if width > height {
                    let scale = 1024.0 / width as f32;
                    (1024, (height as f32 * scale).round() as u32)
                } else {
                    let scale = 1024.0 / height as f32;
                    ((width as f32 * scale).round() as u32, 1024)
                };

                // Resize and hash
                let resized = img.resize(
                    target_width,
                    target_height,
                    image::imageops::FilterType::Triangle,
                );
                return Ok(calculate_phash(&resized));
            }

            // Standard processing for normal-sized JPEGs
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
            Err(e)
        }
    }
}
