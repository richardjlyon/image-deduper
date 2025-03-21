use std::path::Path;
use std::process::Command;
use std::sync::Once;

use crate::processing::calculate_phash;
use crate::processing::types::PHash;

// Static check for tools to avoid repeated checks
static CHECK_SIPS: Once = Once::new();
static CHECK_QLMANAGE: Once = Once::new();
static mut HAS_SIPS: bool = false;
static mut HAS_QLMANAGE: bool = false;

/// Initialize and check for macOS tools
pub fn init() {
    CHECK_SIPS.call_once(|| {
        let has_tool = Command::new("sips").arg("--help").output().is_ok();
        unsafe {
            HAS_SIPS = has_tool;
        }
    });

    CHECK_QLMANAGE.call_once(|| {
        let has_tool = Command::new("qlmanage").arg("-h").output().is_ok();
        unsafe {
            HAS_QLMANAGE = has_tool;
        }
    });
}

/// Check if sips is available (macOS image processing utility)
pub fn has_sips() -> bool {
    init();
    unsafe { HAS_SIPS }
}

/// Check if qlmanage is available (macOS Quick Look Manager)
pub fn has_qlmanage() -> bool {
    init();
    unsafe { HAS_QLMANAGE }
}

/// Convert image using sips and return a hash of the result
///
/// # Arguments
/// * `path` - Path to the image file
/// * `format` - Target format (e.g., "jpg", "png")
/// * `max_size` - Maximum dimension for resizing
pub fn convert_with_sips<P: AsRef<Path>>(
    path: P,
    format: &str,
    max_size: u32,
) -> Result<PHash, image::ImageError> {
    if !has_sips() {
        return Err(image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "sips command not available",
        )));
    }

    let path_ref = path.as_ref();

    // Create a temporary file for the conversion
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let random_name = format!("converted_{}.{}", timestamp, format);
    let temp_path = temp_dir.join(random_name);

    // Try to convert using sips with optimized settings for speed
    let output = Command::new("sips")
        .arg("-s")
        .arg("format")
        .arg(format)
        .arg("-s")
        .arg("dpiHeight")
        .arg("72") // Lower DPI
        .arg("-s")
        .arg("dpiWidth")
        .arg("72")
        .arg("-Z")
        .arg(max_size.to_string()) // Target size
        .arg(path_ref.as_os_str())
        .arg("--out")
        .arg(&temp_path)
        .output();

    match output {
        Ok(output) => {
            if output.status.success() && temp_path.exists() {
                // Try to load the converted file
                if let Ok(img) = image::open(&temp_path) {
                    // Get the hash before deleting the temporary file
                    let result = calculate_phash(&img);

                    // Clean up
                    let _ = std::fs::remove_file(&temp_path);

                    return Ok(result);
                }

                // Clean up even if loading failed
                let _ = std::fs::remove_file(&temp_path);

                return Err(image::ImageError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Failed to open converted image",
                )));
            } else {
                // Log specific sips error for diagnosis
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    log::error!(
                        "sips failed: status={}, stderr={}, stdout={}",
                        output.status,
                        stderr,
                        stdout
                    );
                }

                // Clean up temp file if it exists
                if temp_path.exists() {
                    let _ = std::fs::remove_file(&temp_path);
                }

                return Err(image::ImageError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("sips conversion failed with status: {}", output.status),
                )));
            }
        }
        Err(e) => {
            return Err(image::ImageError::IoError(e));
        }
    }
}

/// Generate thumbnail using qlmanage and return a hash of the result
pub fn generate_thumbnail_with_qlmanage<P: AsRef<Path>>(
    path: P,
    size: u32,
) -> Result<PHash, image::ImageError> {
    if !has_qlmanage() {
        return Err(image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "qlmanage command not available",
        )));
    }

    let path_ref = path.as_ref();
    let temp_dir = std::env::temp_dir();

    let output = Command::new("qlmanage")
        .arg("-t")
        .arg("-s")
        .arg(size.to_string())
        .arg("-o")
        .arg(temp_dir.as_os_str())
        .arg(path_ref.as_os_str())
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                // qlmanage creates a thumbnail with predictable name
                let filename = path_ref.file_name().unwrap_or_default().to_string_lossy();
                let thumbnail_path = temp_dir.join(format!("{}.png", filename));

                if thumbnail_path.exists() {
                    if let Ok(img) = image::open(&thumbnail_path) {
                        let result = calculate_phash(&img);
                        let _ = std::fs::remove_file(&thumbnail_path);
                        return Ok(result);
                    }
                    let _ = std::fs::remove_file(&thumbnail_path);
                }

                return Err(image::ImageError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Thumbnail not found after qlmanage execution",
                )));
            } else {
                return Err(image::ImageError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("qlmanage failed with status: {}", output.status),
                )));
            }
        }
        Err(e) => {
            return Err(image::ImageError::IoError(e));
        }
    }
}
