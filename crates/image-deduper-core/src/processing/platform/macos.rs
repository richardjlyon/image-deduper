use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Once;
use std::thread;
use std::time::Duration;

use log::{debug, error, info};

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

/// Convert HEIC image to PNG using sips and return a hash of the result
///
/// # Arguments
/// * `path` - Path to the image file
/// * `max_size` - Maximum dimension for resizing (use 0 for no resizing)
pub fn convert_with_sips<P: AsRef<Path>>(
    path: P,
    max_size: u32,
) -> Result<PHash, image::ImageError> {
    if !has_sips() {
        return Err(image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "sips command not available",
        )));
    }

    let path_ref = path.as_ref();

    // Verify the source file exists and is readable
    if !path_ref.exists() {
        return Err(image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Source file does not exist: {}", path_ref.display()),
        )));
    }

    // Create a temporary file for the conversion
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    // Always use PNG format
    let target_format = "png";
    let random_name = format!("converted_{}.{}", timestamp, target_format);
    let temp_path = temp_dir.join(random_name);

    // Ensure temp directory exists and is writable
    if !temp_dir.exists() {
        return Err(image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Temp directory does not exist: {}", temp_dir.display()),
        )));
    }

    // Try to convert using sips
    let target_format = "png";
    let mut command = Command::new("sips");
    command.arg("-s").arg("format").arg(target_format);

    // Add resizing if needed
    if max_size > 0 {
        command
            .arg("-s")
            .arg("dpiHeight")
            .arg("72")
            .arg("-s")
            .arg("dpiWidth")
            .arg("72")
            .arg("-Z")
            .arg(max_size.to_string());
    }

    // Add output and input paths
    command
        .arg("--out")
        .arg(&temp_path)
        .arg(path_ref.as_os_str());

    // Execute the command
    let output = command.output();

    match output {
        Ok(output) => {
            // Log stdout and stderr regardless of success
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if output.status.success() {
                // Wait a short time to ensure file is completely written
                thread::sleep(Duration::from_millis(100));

                // Verify the temp file exists after conversion
                if !temp_path.exists() {
                    return Err(image::ImageError::IoError(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("Converted file was not created at: {}", temp_path.display()),
                    )));
                }

                // Get file metadata to confirm it's a valid file with content
                match fs::metadata(&temp_path) {
                    Ok(metadata) => {
                        if metadata.len() == 0 {
                            let _ = std::fs::remove_file(&temp_path);
                            return Err(image::ImageError::IoError(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Converted file has zero size",
                            )));
                        }
                    }
                    Err(e) => {
                        error!("Failed to get metadata for converted file: {}", e);
                    }
                }

                // Try to load the converted file
                match image::open(&temp_path) {
                    Ok(img) => {
                        // Get the hash before deleting the temporary file
                        let result = calculate_phash(&img);

                        // Clean up
                        let _ = std::fs::remove_file(&temp_path);

                        Ok(result)
                    }
                    Err(e) => {
                        error!("Failed to open converted image: {}", e);

                        // Log file details for debugging
                        if let Ok(metadata) = fs::metadata(&temp_path) {
                            debug!("Converted file size: {} bytes", metadata.len());
                        }

                        // Clean up
                        let _ = std::fs::remove_file(&temp_path);

                        Err(image::ImageError::IoError(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Failed to open converted image: {}", e),
                        )))
                    }
                }
            } else {
                // Log specific sips error for diagnosis
                error!(
                    "SIPS conversion failed: status={}, stderr={}, stdout={}",
                    output.status, stderr, stdout
                );

                // Clean up temp file if it exists
                if temp_path.exists() {
                    let _ = std::fs::remove_file(&temp_path);
                }

                Err(image::ImageError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("SIPS conversion failed with status: {}", output.status),
                )))
            }
        }
        Err(e) => {
            error!("Failed to execute SIPS command: {}", e);
            Err(image::ImageError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to execute SIPS command: {}", e),
            )))
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

    // Ensure the path exists
    if !path_ref.exists() {
        return Err(image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Source file does not exist: {}", path_ref.display()),
        )));
    }

    // Build the command
    let mut command = Command::new("qlmanage");
    command
        .arg("-t")
        .arg("-s")
        .arg(size.to_string())
        .arg("-o")
        .arg(&temp_dir)
        .arg(path_ref);

    // Log the command
    let program = command.get_program().to_string_lossy();
    let args: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_string_lossy())
        .collect();
    info!("qlmanage command: {} {}", program, args.join(" "));

    let output = command.output();

    match output {
        Ok(output) => {
            // Log stdout and stderr regardless of success
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !stdout.is_empty() {
                debug!("qlmanage stdout: {}", stdout);
            }

            if !stderr.is_empty() {
                debug!("qlmanage stderr: {}", stderr);
            }

            if output.status.success() {
                // qlmanage creates a thumbnail with predictable name
                let filename = path_ref.file_name().unwrap_or_default().to_string_lossy();
                let thumbnail_path = temp_dir.join(format!("{}.png", filename));

                // Wait a bit for file system operations to complete
                thread::sleep(Duration::from_millis(100));

                if thumbnail_path.exists() {
                    match image::open(&thumbnail_path) {
                        Ok(img) => {
                            let result = calculate_phash(&img);
                            let _ = std::fs::remove_file(&thumbnail_path);
                            Ok(result)
                        }
                        Err(e) => {
                            error!("Failed to open thumbnail: {}", e);
                            let _ = std::fs::remove_file(&thumbnail_path);
                            Err(image::ImageError::IoError(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("Failed to open thumbnail: {}", e),
                            )))
                        }
                    }
                } else {
                    error!(
                        "Thumbnail not found at expected path: {}",
                        thumbnail_path.display()
                    );
                    Err(image::ImageError::IoError(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!(
                            "Thumbnail not found after qlmanage execution: {}",
                            thumbnail_path.display()
                        ),
                    )))
                }
            } else {
                error!(
                    "qlmanage failed: status={}, stderr={}, stdout={}",
                    output.status, stderr, stdout
                );

                Err(image::ImageError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("qlmanage failed with status: {}", output.status),
                )))
            }
        }
        Err(e) => {
            error!("Failed to execute qlmanage command: {}", e);
            Err(image::ImageError::IoError(e))
        }
    }
}
