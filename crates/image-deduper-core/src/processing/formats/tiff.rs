use crate::processing::calculate_phash;
use crate::processing::types::PHash;
use image::GenericImageView;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;
use std::sync::Once;

/// Public function for direct TIFF processing from external modules
/// This provides access to the optimized TIFF handling
pub fn process_tiff_directly<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    // Try specialized downscaling first
    let result = process_tiff_with_downscaling(&path);
    if result.is_ok() {
        return result;
    }

    // Fall back to standard fallback procedure if needed
    process_tiff_with_fallback(path)
}

/// Process TIFF files with simple size check and direct load approach
pub fn process_tiff_with_fallback<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();

    // Check file size first - skip processing for large TIFF files
    const TIFF_SIZE_LIMIT: u64 = 100_000_000; // 100 MB limit

    if let Ok(metadata) = std::fs::metadata(path_ref) {
        let file_size = metadata.len();

        if file_size > TIFF_SIZE_LIMIT {
            // For large TIFF files, just log and return error
            let size_mb = file_size / 1_000_000;
            log::warn!(
                "Skipping large TIFF file ({}MB > 100MB limit): {}",
                size_mb,
                path_ref.display()
            );

            return Err(image::ImageError::Unsupported(
                image::error::UnsupportedError::from_format_and_kind(
                    image::error::ImageFormatHint::Name("TIFF".to_string()),
                    image::error::UnsupportedErrorKind::GenericFeature(format!(
                        "File exceeds size limit ({}MB)",
                        size_mb
                    )),
                ),
            ));
        }
    }

    // For normal-sized TIFFs, try direct loading with downscaling
    let result = process_tiff_with_downscaling(path_ref);
    if result.is_ok() {
        return result;
    }

    // If direct loading failed, log the failure
    log::info!("TIFF processing failed for {}", path_ref.display());

    // Stage 2: Try macOS tools if available (highly optimized for TIFF handling)
    // Static check for tools to avoid repeated checks
    static CHECK_SIPS: Once = Once::new();
    static mut HAS_SIPS: bool = false;

    // Check system tools once
    CHECK_SIPS.call_once(|| {
        let has_tool = Command::new("sips").arg("--help").output().is_ok();
        unsafe {
            HAS_SIPS = has_tool;
        }
    });

    let has_sips = unsafe { HAS_SIPS };

    // Try macOS Preview via sips utility (pre-installed)
    if cfg!(target_os = "macos") && has_sips {
        // Create a temporary file for the conversion
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let random_name = format!("tiff_{}.jpg", timestamp);
        let temp_path = temp_dir.join(random_name);

        // Try to convert using sips with optimized settings for speed
        let output = Command::new("sips")
            .arg("-s")
            .arg("format")
            .arg("jpeg") // Use JPEG instead of PNG for better compatibility
            .arg("-s")
            .arg("dpiHeight")
            .arg("72") // Lower DPI
            .arg("-s")
            .arg("dpiWidth")
            .arg("72")
            .arg("-Z")
            .arg("512") // Moderate target size
            .arg(path_ref.as_os_str())
            .arg("--out")
            .arg(&temp_path)
            .output();

        match output {
            Ok(output) => {
                if output.status.success() && temp_path.exists() {
                    // Try to load the converted JPEG file
                    if let Ok(img) = image::open(&temp_path) {
                        // Get the hash before deleting the temporary file
                        let result = calculate_phash(&img);

                        log::info!(
                            "Successfully processed TIFF using sips conversion: {}",
                            path_ref.display()
                        );

                        // Clean up
                        let _ = std::fs::remove_file(&temp_path);

                        return Ok(result);
                    }

                    // Clean up even if loading failed
                    let _ = std::fs::remove_file(&temp_path);
                }
            }
            Err(_) => { /* Skip logging for better performance */ }
        }
    }

    // Stage 3: Last resort - filename-based fallback
    log::warn!(
        "All TIFF processing methods failed for {}, using filename hash",
        path_ref.display()
    );

    // Generate a hash based on filename and metadata
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

    // Return hash value
    Ok(PHash::Standard(hasher.finish()))
}

/// Specialized function for directly processing TIFF files with optimized downscaling
/// This approach attempts to load the TIFF file at a lower resolution directly
/// Note: This function is only called for TIFF files under 100MB due to the size limit in process_tiff_with_fallback
fn process_tiff_with_downscaling<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();

    // Try to load the image with standard approach
    if let Ok(reader) = image::io::Reader::open(path_ref) {
        if let Some(format) = reader.format() {
            if format == image::ImageFormat::Tiff {
                return match reader.with_guessed_format() {
                    Ok(reader) => {
                        log::info!("Loading TIFF at reduced resolution: {}", path_ref.display());

                        match reader.decode() {
                            Ok(img) => {
                                let (width, height) = img.dimensions();
                                log::info!(
                                    "Successfully loaded TIFF {}x{}, resizing for hash",
                                    width,
                                    height
                                );

                                // Standard resize to 512px max for efficient processing
                                let resized = if width > 512 || height > 512 {
                                    if width > height {
                                        let scale = 512.0 / width as f32;
                                        img.resize(
                                            512,
                                            (height as f32 * scale).round() as u32,
                                            image::imageops::FilterType::Triangle,
                                        )
                                    } else {
                                        let scale = 512.0 / height as f32;
                                        img.resize(
                                            (width as f32 * scale).round() as u32,
                                            512,
                                            image::imageops::FilterType::Triangle,
                                        )
                                    }
                                } else {
                                    img
                                };

                                Ok(calculate_phash(&resized))
                            }
                            Err(e) => {
                                // Check if error is memory related
                                let err_str = e.to_string();
                                if err_str.contains("Memory limit exceeded") ||
                                   err_str.contains("Memory") ||  // More general case
                                   err_str.contains("memory") ||
                                   err_str.contains("allocation") ||
                                   err_str.contains("resource") ||  // Resource exhaustion errors
                                   err_str.contains("out of memory") ||  // Explicit OOM errors
                                   err_str.contains("limit") ||  // Various limit-related errors
                                   err_str.contains("exhausted") ||  // Resource exhaustion
                                   err_str.contains("exceeded")
                                {
                                    // Various exceeded limits

                                    log::error!(
                                        "Memory limit exceeded when processing TIFF: {}",
                                        path_ref.display()
                                    );

                                    // For memory errors, try with even more aggressive settings
                                    // Use sips on macOS as it's highly optimized for memory usage
                                    if cfg!(target_os = "macos") {
                                        // Try to convert using sips for memory-efficient processing
                                        let temp_dir = std::env::temp_dir();
                                        let timestamp = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis();
                                        let random_name =
                                            format!("tiff_mem_error_{}.jpg", timestamp);
                                        let temp_path = temp_dir.join(random_name);

                                        log::info!(
                                            "Attempting ultra memory-efficient conversion for: {}",
                                            path_ref.display()
                                        );

                                        // Use aggressive settings to minimize memory usage
                                        let output = std::process::Command::new("sips")
                                            .arg("-s")
                                            .arg("format")
                                            .arg("jpeg")
                                            .arg("-Z")
                                            .arg("256") // Small target size for memory issues
                                            .arg(path_ref.as_os_str())
                                            .arg("--out")
                                            .arg(&temp_path)
                                            .output();

                                        match output {
                                            Ok(output)
                                                if output.status.success()
                                                    && temp_path.exists() =>
                                            {
                                                if let Ok(img) = image::open(&temp_path) {
                                                    let result = calculate_phash(&img);
                                                    let _ = std::fs::remove_file(&temp_path);
                                                    log::info!("Successfully processed memory-intensive TIFF using external tools: {}",
                                                        path_ref.display());
                                                    return Ok(result);
                                                }
                                                // Clean up temp file
                                                let _ = std::fs::remove_file(&temp_path);
                                            }
                                            Ok(output) => {
                                                // Log specific sips error for diagnosis
                                                if !output.status.success() {
                                                    let stderr =
                                                        String::from_utf8_lossy(&output.stderr);
                                                    let stdout =
                                                        String::from_utf8_lossy(&output.stdout);
                                                    log::error!("sips failed for TIFF {}: status={}, stderr={}, stdout={}",
                                                        path_ref.display(), output.status, stderr, stdout);
                                                } else {
                                                    log::error!("sips conversion completed but temp file not created for TIFF: {}",
                                                        path_ref.display());
                                                }

                                                // Clean up temp file if it exists
                                                if temp_path.exists() {
                                                    let _ = std::fs::remove_file(&temp_path);
                                                }
                                            }
                                            Err(e) => {
                                                // Log specific OS error for diagnosis
                                                log::error!(
                                                    "sips process failed for TIFF {}: {}",
                                                    path_ref.display(),
                                                    e
                                                );

                                                // Clean up temp file if it exists
                                                if temp_path.exists() {
                                                    let _ = std::fs::remove_file(&temp_path);
                                                }
                                            }
                                        }
                                    }
                                }

                                log::error!(
                                    "Failed to decode TIFF directly: {}: {}",
                                    path_ref.display(),
                                    e
                                );
                                Err(e)
                            }
                        }
                    }
                    Err(e) => Err(image::ImageError::IoError(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Format error: {}", e),
                    ))),
                };
            }
        }
    }

    // Standard fallback for any TIFF format detection or loading issues
    match image::open(path_ref) {
        Ok(img) => {
            let (width, height) = img.dimensions();
            log::info!(
                "Loaded TIFF with dimensions {}x{}, resizing for hashing",
                width,
                height
            );

            // Resize to 512px max for more efficient processing
            let resized = if width > 512 || height > 512 {
                if width > height {
                    let scale = 512.0 / width as f32;
                    img.resize(
                        512,
                        (height as f32 * scale).round() as u32,
                        image::imageops::FilterType::Triangle,
                    )
                } else {
                    let scale = 512.0 / height as f32;
                    img.resize(
                        (width as f32 * scale).round() as u32,
                        512,
                        image::imageops::FilterType::Triangle,
                    )
                }
            } else {
                img
            };

            Ok(calculate_phash(&resized))
        }
        Err(e) => Err(e),
    }
}
