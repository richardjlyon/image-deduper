use crate::error::{Error, Result};
use std::hash::{Hash, Hasher};
/// General file processing logic
///
use std::path::Path;

use log::warn;

use crate::processing::{calculate_enhanced_phash, calculate_phash, formats};

use super::types::PHash;

/// Calculate a perceptual hash from an image file
/// Uses standard 8x8 hash by default
pub fn phash_from_file<P: AsRef<Path>>(path: P) -> Result<PHash> {
    let path_ref = path.as_ref();

    // Check file extension and use format-specific handler if available
    if let Some(format) = detect_image_format(path_ref) {
        // Try processing with format-specific code
        match format {
            ImageFormat::Heic => return formats::heic::process_heic_image(path_ref),
            ImageFormat::Jpeg => return formats::jpeg::process_jpeg_image(path),
            ImageFormat::Png => return formats::png::process_png_image(path),
            ImageFormat::Tiff => return formats::tiff::process_tiff_image(path_ref),
            ImageFormat::Raw => return formats::raw::process_raw_image(path_ref),
            _ => {
                warn!("No matching image handler");
                return Err(Error::FormatHandling(format!(
                    "No handler available for format: {:?}",
                    format
                )));
            }
        }
    }

    // If no format detected
    Err(Error::FormatHandling(
        "Could not detect image format".to_string(),
    ))
}

/// Calculate an enhanced 1024-bit perceptual hash from an image file (32x32 grid)
pub fn enhanced_phash_from_file<P: AsRef<Path>>(path: P) -> Result<PHash> {
    let path_ref = path.as_ref();

    // Check file extension and use format-specific handler if available
    if let Some(format) = detect_image_format(path_ref) {
        // Special formats currently use standard hash - potential improvement area
        match format {
            ImageFormat::Heic | ImageFormat::Tiff | ImageFormat::Raw => {
                return phash_from_file(path_ref);
            }
            _ => {} // Continue with enhanced processing
        }
    }

    // Handle large image resizing for enhanced hash calculation
    // First try to efficiently get image dimensions without loading the whole image
    if let Ok(reader) = image::io::Reader::open(path_ref) {
        if let Ok(reader) = reader.with_guessed_format() {
            if let Ok((width, height)) = reader.into_dimensions() {
                // If the image is very large, resize it before computing the hash
                if width > 1024 || height > 1024 {
                    log::info!(
                        "Downscaling large image ({}x{}) for enhanced perceptual hash: {}",
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

                    // Load image and resize it to target dimensions
                    if let Ok(img) = image::open(path_ref) {
                        let resized = img.resize(
                            target_width,
                            target_height,
                            image::imageops::FilterType::Lanczos3,
                        );

                        // Compute enhanced hash on resized image
                        return Ok(calculate_enhanced_phash(&resized));
                    }
                }
            }
        }
    }

    // For standard formats or small images, use the regular load path
    match image::open(path_ref) {
        Ok(img) => Ok(calculate_enhanced_phash(&img)),
        Err(e) => Err(Error::Image(e)),
    }
}

/// Process a large image by downscaling it for perceptual hash computation
/// This allows us to handle very large images efficiently without timeouts
pub fn process_large_image<P: AsRef<Path>>(path: P) -> Result<PHash> {
    let path_ref = path.as_ref();

    // Check file size first to determine optimal strategy
    let file_size = if let Ok(metadata) = std::fs::metadata(path_ref) {
        metadata.len()
    } else {
        0 // Default if we can't get the size
    };

    // First try to efficiently get image dimensions without loading the whole image
    let reader = image::io::Reader::open(path_ref)?;
    let reader = reader.with_guessed_format()?;
    let dimensions = reader.into_dimensions();

    // If we can get dimensions directly, use them for efficient resizing decision
    if let Ok((width, height)) = dimensions {
        // If the image is very large, resize it before computing the hash
        if width > 1024 || height > 1024 {
            log::info!(
                "Downscaling large image ({}x{}, {}MB) for perceptual hash computation: {}",
                width,
                height,
                file_size / 1_000_000,
                path_ref.display()
            );

            // Determine target size and resize filter based on file size
            let (max_dimension, filter) = if file_size > 300_000_000 {
                // Extreme optimization for very large files (300MB+)
                (768, image::imageops::FilterType::Nearest)
            } else if file_size > 100_000_000 {
                // Strong optimization for large files (100MB-300MB)
                (896, image::imageops::FilterType::Triangle)
            } else {
                // Standard optimization for moderately large files
                (1024, image::imageops::FilterType::Lanczos3)
            };

            log::info!(
                "Using file-size optimized parameters: max {}px with {} filter: {}",
                max_dimension,
                if file_size > 300_000_000 {
                    "fastest"
                } else if file_size > 100_000_000 {
                    "balanced"
                } else {
                    "quality"
                },
                path_ref.display()
            );

            // Calculate target dimensions maintaining aspect ratio
            let (target_width, target_height) = if width > height {
                let scale = max_dimension as f32 / width as f32;
                (max_dimension, (height as f32 * scale).round() as u32)
            } else {
                let scale = max_dimension as f32 / height as f32;
                ((width as f32 * scale).round() as u32, max_dimension)
            };

            // Load image and resize it to target dimensions
            let img = image::open(path_ref)?;
            let resized = img.resize(target_width, target_height, filter);

            // Compute hash on resized image
            return Ok(calculate_phash(&resized));
        }
    }

    // For smaller images or if we couldn't determine dimensions, use normal path
    let img = image::open(path_ref)?;
    Ok(calculate_phash(&img))
}

/// Generate a fallback hash based on file metadata when image processing fails
pub fn generate_fallback_hash<P: AsRef<Path>>(path: P) -> PHash {
    let path_ref = path.as_ref();
    let filename = path_ref.file_name().unwrap_or_default().to_string_lossy();

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    filename.hash(&mut hasher);

    // Add file size, mod time and other metadata for better uniqueness
    if let Ok(metadata) = std::fs::metadata(path_ref) {
        metadata.len().hash(&mut hasher);
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                duration.as_secs().hash(&mut hasher);
            }
        }
    }

    PHash::Standard(hasher.finish())
}

/// Enum for supported image formats with specialized handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Heic,
    Tiff,
    Raw,
    Jpeg,
    Png,
    Other,
}

/// Detect image format based on file extension
fn detect_image_format<P: AsRef<Path>>(path: P) -> Option<ImageFormat> {
    let path_ref = path.as_ref();
    if let Some(ext) = path_ref.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();

        match ext_lower.as_str() {
            "heic" => Some(ImageFormat::Heic),
            "tif" | "tiff" => Some(ImageFormat::Tiff),
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "png" => Some(ImageFormat::Png),
            "raw" | "dng" | "cr2" | "nef" | "arw" | "orf" | "rw2" | "nrw" | "raf" | "crw"
            | "pef" | "srw" | "x3f" | "rwl" | "3fr" => Some(ImageFormat::Raw),
            _ => Some(ImageFormat::Other),
        }
    } else {
        None
    }
}
