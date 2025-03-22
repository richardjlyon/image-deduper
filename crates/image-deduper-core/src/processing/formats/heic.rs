use std::io::Read;
use std::path::Path;

use log::{info, warn};

use crate::processing::types::PHash;
use crate::processing::{calculate_phash, platform};

/// Process HEIC image files
pub fn process_heic_image<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    info!("Processing HEIC image");
    let path_ref = path.as_ref();

    // Create a custom error for HEIC issues
    let heic_error = |msg: &str| -> image::ImageError {
        image::ImageError::Unsupported(image::error::UnsupportedError::from_format_and_kind(
            image::error::ImageFormatHint::Name("HEIC".to_string()),
            image::error::UnsupportedErrorKind::GenericFeature(msg.to_string()),
        ))
    };

    // Try platform-specific approach first (on macOS)
    #[cfg(target_os = "macos")]
    {
        match platform::macos::convert_with_sips(path_ref, 0) {
            Ok(hash) => {
                info!("Processed HEIC with SIPS");
                return Ok(hash);
            }
            Err(e) => {
                warn!("SIPS conversion failed: {:?}", e);
                // You might want to continue to the next conversion method rather than return here
                // return Err(e);
            }
        }
    }

    // Use libheif to read the file
    let path_str = path_ref
        .to_str()
        .ok_or_else(|| heic_error("Invalid path for HEIC file"))?;

    let ctx = libheif_rs::HeifContext::read_from_file(path_str)
        .map_err(|e| heic_error(&format!("Failed to read HEIC: {}", e)))?;

    // Get primary image handle
    let handle = ctx
        .primary_image_handle()
        .map_err(|e| heic_error(&format!("Failed to get HEIC handle: {}", e)))?;

    // Decode the image
    let heif_img = handle
        .decode(
            libheif_rs::ColorSpace::Rgb(libheif_rs::RgbChroma::Rgb),
            None,
        )
        .map_err(|e| heic_error(&format!("Failed to decode HEIC: {}", e)))?;

    // Get dimensions
    let width = heif_img.width();
    let height = heif_img.height();

    // Access the image data
    if let Some(plane) = heif_img.planes().interleaved {
        // Access the raw data
        let pixel_data = plane.data;

        // Create an RGB image
        let img = image::RgbImage::from_raw(width, height, pixel_data.to_vec())
            .ok_or_else(|| heic_error("Failed to create RGB image from HEIC data"))?;

        // Convert to DynamicImage
        let dynamic_img = image::DynamicImage::ImageRgb8(img);

        // For smaller images, compute hash directly
        return Ok(calculate_phash(&dynamic_img));
    } else {
        return Err(heic_error("HEIC image doesn't have interleaved data"));
    }
}

/// Helper function to check if a file is in HEIC format
pub fn is_heic_format<P: AsRef<Path>>(path: P) -> bool {
    info!("Processing HEIC image");
    // Use a block to ensure file is dropped at end of scope
    let result = {
        if let Ok(mut file) = std::fs::File::open(path.as_ref()) {
            let mut buffer = [0; 12];
            if file.read_exact(&mut buffer).is_ok() {
                // HEIC/HEIF format signatures
                if (buffer[4..8] == [b'f', b't', b'y', b'p'])
                    || (buffer[4..8] == [b'h', b'e', b'i', b'c'])
                    || (buffer[4..8] == [b'h', b'e', b'i', b'f'])
                    || (buffer[4..8] == [b'm', b'i', b'f', b'1'])
                {
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    };

    result
}
