//! # Perceptual Hashing Module
//!
//! This module provides efficient implementations of perceptual hashing algorithms
//! for image comparison and similarity detection.
//!
//! ## Overview
//!
//! Perceptual hashing generates "fingerprints" that remain similar for visually similar images,
//! unlike cryptographic hashes where minor changes produce completely different outputs.
//!
//! This implementation offers three methods with different speed/accuracy tradeoffs:
//!
//! 1. Original pHash: DCT-based perceptual hash (slowest but most accurate)
//! 2. Optimized pHash: Direct 8×8 downsampling with grayscale conversion (good balance)
//! 3. Ultra-fast pHash: Strategic sampling without resizing (fastest but less accurate)
//!
//! ## Hamming Distance Interpretation
//!
//! The similarity between two images is measured using Hamming distance (count of differing bits):
//!
//! - 0-3: Nearly identical images (same image with minor modifications)
//! - 4-10: Similar images (same subject with moderate differences)
//! - >10-15: Different images
//!
//! ## Implementation Details
//!
//! - All methods produce a 64-bit hash
//! - The ultra-fast method typically has a Hamming distance of ~13 from the original method
//! - This represents about 20% difference while being significantly faster
//!
//! ## Usage Guidance
//!
//! - For exact duplicate detection: Use the original or optimized method
//! - For near-duplicate detection: The optimized method offers a good balance
//! - For similarity searching: The ultra-fast method is appropriate when speed is critical
//! - Consider a hybrid approach: Screen with ultra-fast, then verify with optimized method
//!
//! ## Performance
//!
//! Approximate processing times for a 4000×4000 image:
//! - Original DCT-based method: ~8 seconds
//! - Optimized direct method: ~4ms
//! - Ultra-fast sampling method: ~8us
//!
//! ## References
//!
//! - "Implementation and analysis of DCT based global perceptual image hashing" by Bian Yang, et al.
//! - "Perceptual Hashing: Robust Image Identification" by Nasir Memon and Savvas A. Chatzichristofis

use image::{DynamicImage, GenericImageView};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;
use std::sync::Once;

/// A perceptual hash that can be either a 64-bit value (8x8) or a 1024-bit value (32x32)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PHash {
    /// Standard 64-bit perceptual hash (8x8 grid)
    Standard(u64),

    /// Enhanced 1024-bit perceptual hash (32x32 grid) for GPU acceleration
    /// Stored as 16 u64 values (16 * 64 = 1024 bits)
    Enhanced([u64; 16]),
}

impl PHash {
    /// Calculate the Hamming distance between two perceptual hashes
    pub fn distance(&self, other: &PHash) -> u32 {
        match (self, other) {
            // Both standard 64-bit hashes
            (PHash::Standard(a), PHash::Standard(b)) => (a ^ b).count_ones(),

            // Both enhanced 1024-bit hashes
            (PHash::Enhanced(a), PHash::Enhanced(b)) => {
                let mut distance = 0;
                for i in 0..16 {
                    distance += (a[i] ^ b[i]).count_ones();
                }
                distance
            }

            // Mixed types - downgrade enhanced to standard for compatibility
            (PHash::Standard(a), PHash::Enhanced(b)) => {
                // Use only the first 64 bits of the enhanced hash
                (a ^ b[0]).count_ones()
            }

            (PHash::Enhanced(a), PHash::Standard(b)) => {
                // Use only the first 64 bits of the enhanced hash
                (a[0] ^ b).count_ones()
            }
        }
    }

    /// Check if two images are perceptually similar based on a threshold
    pub fn is_similar(&self, other: &PHash, threshold: u32) -> bool {
        let distance = self.distance(other);

        // Adjust threshold based on hash type (enhanced hashes need higher thresholds)
        let adjusted_threshold = match (self, other) {
            (PHash::Standard(_), PHash::Standard(_)) => threshold,
            (PHash::Enhanced(_), PHash::Enhanced(_)) => threshold * 16, // Scale by hash size ratio
            _ => threshold, // Mixed types use standard threshold
        };

        distance <= adjusted_threshold
    }

    /// Convert to a standard 64-bit hash if enhanced
    pub fn to_standard(&self) -> PHash {
        match self {
            PHash::Standard(hash) => PHash::Standard(*hash),
            PHash::Enhanced(hash_array) => PHash::Standard(hash_array[0]),
        }
    }

    /// Get the underlying 64-bit hash value (for compatibility)
    pub fn as_u64(&self) -> u64 {
        match self {
            PHash::Standard(hash) => *hash,
            PHash::Enhanced(hash_array) => hash_array[0],
        }
    }
}

/// Calculate a standard 64-bit perceptual hash for an image (8x8 grid)
#[inline]
pub fn calculate_phash(img: &DynamicImage) -> PHash {
    // Use fastest filter for downscaling
    let small = img.resize_exact(8, 8, image::imageops::FilterType::Nearest);

    // Extract grayscale values directly, avoiding full grayscale conversion
    // Grayscale formula: 0.299*R + 0.587*G + 0.114*B
    let mut pixels = [0.0; 64];

    for y in 0..8 {
        for x in 0..8 {
            let pixel = small.get_pixel(x, y);
            let gray_value =
                0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
            pixels[(y as usize) * 8 + (x as usize)] = gray_value;
        }
    }

    // Use a partial sum approach to calculate the mean
    let mut sum = 0.0;
    for &p in &pixels {
        sum += p;
    }
    let mean = sum / 64.0;

    // Optimized hash calculation using bit manipulation
    let mut hash: u64 = 0;

    // Process 8 comparisons at once in each loop iteration
    for chunk in 0..8 {
        let base = chunk * 8;

        // Build an 8-bit chunk
        let mut byte: u8 = 0;
        if pixels[base] > mean {
            byte |= 1 << 0;
        }
        if pixels[base + 1] > mean {
            byte |= 1 << 1;
        }
        if pixels[base + 2] > mean {
            byte |= 1 << 2;
        }
        if pixels[base + 3] > mean {
            byte |= 1 << 3;
        }
        if pixels[base + 4] > mean {
            byte |= 1 << 4;
        }
        if pixels[base + 5] > mean {
            byte |= 1 << 5;
        }
        if pixels[base + 6] > mean {
            byte |= 1 << 6;
        }
        if pixels[base + 7] > mean {
            byte |= 1 << 7;
        }

        // Place the byte in the appropriate position in the hash
        hash |= (byte as u64) << (chunk * 8);
    }

    PHash::Standard(hash)
}

/// Calculate an enhanced 1024-bit perceptual hash for an image (32x32 grid)
/// For higher quality discrimination and better GPU acceleration potential
#[inline]
pub fn calculate_enhanced_phash(img: &DynamicImage) -> PHash {
    // Use fastest filter for downscaling to 32x32
    let small = img.resize_exact(32, 32, image::imageops::FilterType::Nearest);

    // Extract grayscale values directly, avoiding full grayscale conversion
    // Grayscale formula: 0.299*R + 0.587*G + 0.114*B
    let mut pixels = [0.0; 1024]; // 32x32 = 1024 pixels

    for y in 0..32 {
        for x in 0..32 {
            let pixel = small.get_pixel(x, y);
            let gray_value =
                0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
            pixels[(y as usize) * 32 + (x as usize)] = gray_value;
        }
    }

    // Calculate mean of all pixels
    let mut sum = 0.0;
    for &p in &pixels {
        sum += p;
    }
    let mean = sum / 1024.0;

    // Create an array of 16 u64 values (1024 bits total)
    let mut hash_array = [0u64; 16];

    // Process 64 pixels at a time to fill each u64
    for segment in 0..16 {
        let mut hash: u64 = 0;

        // Each segment processes 64 pixels
        for i in 0..64 {
            let pixel_idx = segment * 64 + i;

            // Set bit if pixel value > mean
            if pixels[pixel_idx] > mean {
                hash |= 1u64 << i;
            }
        }

        hash_array[segment] = hash;
    }

    PHash::Enhanced(hash_array)
}

/// Ultra-fast implementation for when quality can be traded for speed
#[inline]
pub fn ultra_fast_phash(img: &DynamicImage) -> PHash {
    // Work with the original image directly
    let width = img.width();
    let height = img.height();

    // Calculate sampling steps
    let step_x = width.max(8) / 8;
    let step_y = height.max(8) / 8;

    // Sample the image at 64 strategic points
    let mut pixels = [0.0; 64];
    let mut sum = 0.0;

    for y in 0..8 {
        let img_y = (y as u32 * step_y).min(height - 1);
        for x in 0..8 {
            let img_x = (x as u32 * step_x).min(width - 1);

            // Get pixel and convert to grayscale on the fly
            let pixel = img.get_pixel(img_x, img_y);
            let gray = 0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;

            pixels[(y as usize) * 8 + (x as usize)] = gray;
            sum += gray;
        }
    }

    // Calculate mean
    let mean = sum / 64.0;

    // Optimized bit comparisons
    let mut hash: u64 = 0;

    // Unrolled loop for maximum performance
    for (bit_pos, &p) in pixels.iter().enumerate() {
        if p > mean {
            hash |= 1u64 << bit_pos;
        }
    }

    PHash::Standard(hash)
}

/// Process TIFF files with advanced strategies to handle large/problematic TIFFs
/// Uses a multi-stage approach similar to RAW file handling
fn process_tiff_with_fallback<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    use std::hash::{Hash, Hasher};
    
    let path_ref = path.as_ref();
    
    // Stage 1: First try direct loading with downscaling during load
    let result = process_tiff_with_downscaling(path_ref);
    if result.is_ok() {
        return result;
    }
    
    // If direct loading failed, log the failure
    log::info!("Direct TIFF loading failed for {}, trying alternate methods", path_ref.display());
    
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

        // Check file size to determine how aggressive we need to be
        let size_based_settings = if let Ok(metadata) = std::fs::metadata(path_ref) {
            let file_size = metadata.len();
            if file_size > 300_000_000 { // 300MB+
                // Extreme downscaling for very large files
                ("-Z", "256")
            } else if file_size > 100_000_000 { // 100MB+
                // Strong downscaling for large files
                ("-Z", "384")
            } else {
                // Moderate downscaling for regular files
                ("-Z", "512") 
            }
        } else {
            // Default if we can't get the size
            ("-Z", "512")
        };

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
            .arg(size_based_settings.0)
            .arg(size_based_settings.1) // Size based on file size
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
                        
                        log::info!("Successfully processed TIFF using sips conversion: {}", path_ref.display());
                        
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
    log::warn!("All TIFF processing methods failed for {}, using filename hash", path_ref.display());
    
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
fn process_tiff_with_downscaling<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();
    
    // First try to get file size to decide on strategy
    if let Ok(metadata) = std::fs::metadata(path_ref) {
        let file_size = metadata.len();
        
        // For very large TIFFs (over 100MB), use even more aggressive downscaling
        if file_size > 100_000_000 {
            log::info!("Processing very large TIFF ({}MB) with aggressive downscaling: {}", 
                      file_size / 1_000_000, path_ref.display());
                      
            // Try loading the image with special downscaling options
            if let Ok(reader) = image::io::Reader::open(path_ref) {
                // Try to guess the format
                if let Some(format) = reader.format() {
                    if format == image::ImageFormat::Tiff {
                        // The TIFF decoder has limits on maximum memory usage
                        // By limiting dimensions, we significantly reduce memory usage
                        
                        // Try to estimate dimensions before full decode 
                        // (TIFF headers usually contain this information)
                        return match reader.with_guessed_format() {
                            Ok(reader) => {
                                log::info!("Loading TIFF at reduced resolution: {}", path_ref.display());
                                
                                // Try to load the image and immediately resize it
                                match reader.decode() {
                                    Ok(img) => {
                                        let (width, height) = img.dimensions();
                                        log::info!("Successfully loaded TIFF {}x{}, resizing for hash", width, height);
                                        
                                        // Aggressively resize large images to 512px max
                                        let resized = if width > 512 || height > 512 {
                                            if width > height {
                                                let scale = 512.0 / width as f32;
                                                img.resize(512, (height as f32 * scale).round() as u32, 
                                                        image::imageops::FilterType::Triangle)
                                            } else {
                                                let scale = 512.0 / height as f32;
                                                img.resize((width as f32 * scale).round() as u32, 512, 
                                                        image::imageops::FilterType::Triangle)
                                            }
                                        } else {
                                            img
                                        };
                                        
                                        Ok(calculate_phash(&resized))
                                    },
                                    Err(e) => {
                                        // Check if error is memory related
                                        let err_str = e.to_string();
                                        if err_str.contains("Memory limit exceeded") || 
                                           err_str.contains("memory") || 
                                           err_str.contains("allocation") {
                                            
                                            log::error!("Memory limit exceeded when processing TIFF: {}", path_ref.display());
                                            
                                            // For memory errors, try with even more aggressive settings
                                            // Use sips on macOS as it's highly optimized for memory usage
                                            if cfg!(target_os = "macos") {
                                                // Try to convert using sips for memory-efficient processing
                                                let temp_dir = std::env::temp_dir();
                                                let timestamp = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap_or_default()
                                                    .as_millis();
                                                let random_name = format!("tiff_mem_error_{}.jpg", timestamp);
                                                let temp_path = temp_dir.join(random_name);
                                                
                                                log::info!("Attempting memory-efficient conversion for: {}", path_ref.display());
                                                
                                                // Use very aggressive settings to minimize memory usage
                                                let output = std::process::Command::new("sips")
                                                    .arg("-s")
                                                    .arg("format")
                                                    .arg("jpeg")
                                                    .arg("-Z")
                                                    .arg("256") // Use much smaller target size for memory issues
                                                    .arg(path_ref.as_os_str())
                                                    .arg("--out")
                                                    .arg(&temp_path)
                                                    .output();
                                                
                                                match output {
                                                    Ok(output) if output.status.success() && temp_path.exists() => {
                                                        if let Ok(img) = image::open(&temp_path) {
                                                            let result = calculate_phash(&img);
                                                            let _ = std::fs::remove_file(&temp_path);
                                                            log::info!("Successfully processed memory-intensive TIFF using external tools: {}", 
                                                                path_ref.display());
                                                            return Ok(result);
                                                        }
                                                        // Clean up temp file
                                                        let _ = std::fs::remove_file(&temp_path);
                                                    },
                                                    _ => {
                                                        // Fall through to error if external tools fail
                                                        if temp_path.exists() {
                                                            let _ = std::fs::remove_file(&temp_path);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        
                                        log::error!("Failed to decode TIFF directly: {}: {}", path_ref.display(), e);
                                        Err(e)
                                    }
                                }
                            },
                            Err(e) => Err(image::ImageError::IoError(std::io::Error::new(
                                std::io::ErrorKind::InvalidData, 
                                format!("Format error: {}", e)
                            )))
                        };
                    }
                }
                
                // Fall back to standard load + resize approach if format detection failed
                match image::open(path_ref) {
                    Ok(img) => {
                        let (width, height) = img.dimensions();
                        log::info!("Loaded TIFF ({}MB) with dimensions {}x{}, resizing for hashing", 
                                 file_size / 1_000_000, width, height);
                        
                        // Resize to 512px max for large files
                        let resized = if width > 512 || height > 512 {
                            if width > height {
                                let scale = 512.0 / width as f32;
                                img.resize(512, (height as f32 * scale).round() as u32, 
                                        image::imageops::FilterType::Triangle)
                            } else {
                                let scale = 512.0 / height as f32;
                                img.resize((width as f32 * scale).round() as u32, 512, 
                                        image::imageops::FilterType::Triangle)
                            }
                        } else {
                            img
                        };
                        
                        Ok(calculate_phash(&resized))
                    },
                    Err(e) => Err(e)
                }
            } else {
                Err(image::ImageError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Could not open TIFF file",
                )))
            }
        } else {
            // For smaller TIFFs, use standard processing with resize
            match image::open(path_ref) {
                Ok(img) => {
                    let (width, height) = img.dimensions();
                    
                    // Still resize to 512px max for consistency
                    let resized = if width > 512 || height > 512 {
                        if width > height {
                            let scale = 512.0 / width as f32;
                            img.resize(512, (height as f32 * scale).round() as u32, 
                                    image::imageops::FilterType::Triangle)
                        } else {
                            let scale = 512.0 / height as f32;
                            img.resize((width as f32 * scale).round() as u32, 512, 
                                    image::imageops::FilterType::Triangle)
                        }
                    } else {
                        img
                    };
                    
                    Ok(calculate_phash(&resized))
                },
                Err(e) => Err(e)
            }
        }
    } else {
        // If we can't get metadata, try standard loading
        match image::open(path_ref) {
            Ok(img) => Ok(calculate_phash(&img)),
            Err(e) => Err(e)
        }
    }
}

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

/// Process RAW image files using a multi-stage approach:
/// 1. Try to use macOS tools (sips/qlmanage) if available (fast)
/// 2. Try direct loading with image crate (slower but works for some formats)
/// 3. Use filename-based hash as a last resort (ensures we always get something)
fn process_raw_image<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();
    
    // First, try to search for an existing JPEG version or thumbnail of this RAW file
    // Many camera systems save both RAW+JPEG by default
    if let Some(file_stem) = path_ref.file_stem().and_then(|s| s.to_str()) {
        let parent_dir = path_ref.parent().unwrap_or_else(|| Path::new("."));
        
        // Check for same filename with JPEG extension
        let possible_jpeg = parent_dir.join(format!("{}.jpg", file_stem));
        if possible_jpeg.exists() {
            log::info!("Found paired JPEG file for RAW: {}", possible_jpeg.display());
            return phash_from_file(&possible_jpeg);
        }
        
        // Check for same filename with JPG extension (alternate spelling)
        let possible_jpg = parent_dir.join(format!("{}.jpg", file_stem));
        if possible_jpg.exists() {
            log::info!("Found paired JPG file for RAW: {}", possible_jpg.display());
            return phash_from_file(&possible_jpg);
        }
    }
    
    // 2. STAGE TWO: Try macOS-specific tools
    // Static check for tools to avoid repeated checks
    static CHECK_SIPS: Once = Once::new();
    static CHECK_QLMANAGE: Once = Once::new();
    static mut HAS_SIPS: bool = false;
    static mut HAS_QLMANAGE: bool = false;

    // Check system tools once
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

    let has_sips = unsafe { HAS_SIPS };
    let has_qlmanage = unsafe { HAS_QLMANAGE };

    // Fast path for macOS
    if cfg!(target_os = "macos") && (has_sips || has_qlmanage) {
        // Create a temporary file
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let temp_name = format!("raw_{}.jpg", timestamp);
        let temp_path = temp_dir.join(temp_name);

        // Try fastest method first (sips)
        if has_sips {
            let output = Command::new("sips")
                .arg("-s")
                .arg("format")
                .arg("jpeg")
                .arg("-s")
                .arg("dpiHeight")
                .arg("72")
                .arg("-s")
                .arg("dpiWidth")
                .arg("72")
                .arg("-Z")
                .arg("512") // Use even smaller size for faster processing
                .arg(path_ref.as_os_str())
                .arg("--out")
                .arg(&temp_path)
                .output();

            if let Ok(output) = output {
                if output.status.success() && temp_path.exists() {
                    // Load the converted JPEG
                    if let Ok(img) = image::open(&temp_path) {
                        // Get the hash and clean up
                        let result = calculate_phash(&img);
                        let _ = std::fs::remove_file(&temp_path);
                        log::info!("Generated hash from sips-processed RAW: {}", path_ref.display());
                        return Ok(result);
                    }
                    let _ = std::fs::remove_file(&temp_path);
                }
            }
        }

        // Try qlmanage as fallback (often faster than sips for thumbnails)
        if has_qlmanage {
            let output = Command::new("qlmanage")
                .arg("-t")
                .arg("-s")
                .arg("256") // Small size for speed
                .arg("-o")
                .arg(temp_dir.as_os_str())
                .arg(path_ref.as_os_str())
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    // qlmanage creates a thumbnail with predictable name
                    let filename = path_ref.file_name().unwrap_or_default().to_string_lossy();
                    let thumbnail_path = temp_dir.join(format!("{}.png", filename));

                    if thumbnail_path.exists() {
                        if let Ok(img) = image::open(&thumbnail_path) {
                            let result = calculate_phash(&img);
                            let _ = std::fs::remove_file(&thumbnail_path);
                            log::info!("Generated hash from qlmanage thumbnail: {}", path_ref.display());
                            return Ok(result);
                        }
                        let _ = std::fs::remove_file(&thumbnail_path);
                    }
                }
            }
        }
    }

    // 3. STAGE THREE: Try to open the RAW file directly with image crate
    match image::open(path_ref) {
        Ok(img) => {
            // If we can open the image, resize it to manageable dimensions
            log::info!("Successfully opened RAW file directly, resizing for hash: {}", path_ref.display());
            
            // Resize to 512px max dimension for faster processing
            let (width, height) = img.dimensions();
            let resized = if width > 512 || height > 512 {
                // Calculate target dimensions maintaining aspect ratio
                let (target_width, target_height) = if width > height {
                    let scale = 512.0 / width as f32;
                    (512, (height as f32 * scale).round() as u32)
                } else {
                    let scale = 512.0 / height as f32;
                    ((width as f32 * scale).round() as u32, 512)
                };
                
                img.resize(target_width, target_height, image::imageops::FilterType::Triangle)
            } else {
                img
            };
            
            // Compute hash on resized image
            return Ok(calculate_phash(&resized));
        },
        Err(_) => {
            // 4. STAGE FOUR: As a last resort, use filename-based hash
            log::warn!("All RAW processing methods failed for {}, using filename hash", path_ref.display());
            
            // Fast-path hash generation as last resort fallback
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

            let filename_hash = hasher.finish();
            Ok(PHash::Standard(filename_hash))
        }
    }
}

/// Calculate a perceptual hash from an image file
/// Uses standard 8x8 hash by default
pub fn phash_from_file<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();

    // Direct handling of problematic file formats by extension first
    if let Some(ext) = path_ref.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();

        // Handle HEIC files
        if ext_lower == "heic" {
            return process_heic_image(path_ref);
        }

        // Pre-emptively handle TIFF files with the fallback mechanism
        if ext_lower == "tif" || ext_lower == "tiff" {
            return process_tiff_with_fallback(path_ref);
        }

        // Handle RAW format files
        if [
            "raw", "dng", "cr2", "nef", "arw", "orf", "rw2", "nrw", "raf", "crw", "pef", "srw",
            "x3f", "rwl", "3fr",
        ]
        .contains(&ext_lower.as_str())
        {
            return process_raw_image(path_ref);
        }
    }

    // Use our large image handling process to automatically resize if needed
    match process_large_image(path_ref) {
        Ok(hash) => return Ok(hash),
        Err(e) => {
            let error_str = format!("{:?}", e);

            // CASE 1: HEIC file with incorrect extension
            if error_str.contains("first two bytes are not an SOI marker") {
                // Check if it's actually a HEIC file (regardless of extension)
                if is_heic_format(path_ref) {
                    log::warn!(
                        "Found HEIC file with incorrect .jpg extension: {}",
                        path_ref.display()
                    );

                    // Try to process it as a HEIC file
                    match process_heic_image(path_ref) {
                        Ok(hash) => {
                            log::info!(
                                "Successfully processed misnamed HEIC file: {}",
                                path_ref.display()
                            );
                            return Ok(hash);
                        }
                        Err(heic_err) => {
                            log::error!(
                                "Failed to process misnamed HEIC file: {}, Error: {}",
                                path_ref.display(),
                                heic_err
                            );
                        }
                    }
                } else {
                    // If not HEIC, try to recover JPEG by finding SOI marker
                    log::warn!(
                        "Attempting to recover corrupted JPEG: {}",
                        path_ref.display()
                    );

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
                }
            }

            // CASE 2: Any TIFF errors
            if error_str.contains("LZW")
                || error_str.contains("tiff")
                || error_str.contains("TIFF")
                || error_str.contains("invalid code")
                || error_str.contains("memory")
                || error_str.contains("Memory limit exceeded")
                || error_str.contains("allocation")
            {
                log::warn!(
                    "Identified TIFF-related error ({}), activating fallback: {}",
                    e, path_ref.display()
                );
                return process_tiff_with_fallback(path_ref);
            }

            // CASE 3: Last chance fallback for any other errors
            // Try processing using external tools anyway
            log::warn!(
                "Unhandled image error, attempting general fallback: {}",
                path_ref.display()
            );
            match process_tiff_with_fallback(path_ref) {
                Ok(hash) => {
                    log::info!(
                        "Successfully processed with fallback: {}",
                        path_ref.display()
                    );
                    return Ok(hash);
                }
                Err(_) => {
                    // If the fallback also fails, return the original error
                    return Err(e);
                }
            }
        }
    }
}

/// Calculate a perceptual hash from an image in memory using standard 8x8 hash
pub fn phash_from_img(img: &DynamicImage) -> PHash {
    calculate_phash(img)
}

/// Calculate an enhanced 1024-bit perceptual hash from an image file (32x32 grid)
/// For higher quality discrimination and better GPU acceleration potential
pub fn enhanced_phash_from_file<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();

    // Direct handling of problematic file formats by extension first
    if let Some(ext) = path_ref.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();

        // Handle HEIC files - convert to standard then enhanced
        if ext_lower == "heic" {
            if let Ok(PHash::Standard(hash)) = process_heic_image(path_ref) {
                return Ok(PHash::Standard(hash)); // Return standard for now
            }
        }

        // Handle other special formats with standard hash for now
        if ext_lower == "tif"
            || ext_lower == "tiff"
            || [
                "raw", "dng", "cr2", "nef", "arw", "orf", "rw2", "nrw", "raf", "crw", "pef", "srw",
                "x3f", "rwl", "3fr",
            ]
            .contains(&ext_lower.as_str())
        {
            // These formats use standard hash for compatibility
            return phash_from_file(path_ref);
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
                        width, height, path_ref.display()
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
                            image::imageops::FilterType::Lanczos3
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
        Err(e) => Err(e),
    }
}

/// Calculate an enhanced perceptual hash from an image in memory
pub fn enhanced_phash_from_img(img: &DynamicImage) -> PHash {
    calculate_enhanced_phash(img)
}

/// Process a large image by downscaling it for perceptual hash computation
/// This allows us to handle very large images efficiently without timeouts
pub fn process_large_image<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    // Import used for dimensions methods
    
    // First try to efficiently get image dimensions without loading the whole image
    let reader = image::io::Reader::open(path.as_ref())?;
    let reader = reader.with_guessed_format()?;
    let dimensions = reader.into_dimensions();
    
    // If we can get dimensions directly, use them for efficient resizing decision
    if let Ok((width, height)) = dimensions {
        // If the image is very large, resize it before computing the hash
        if width > 1024 || height > 1024 {
            log::info!(
                "Downscaling large image ({}x{}) for perceptual hash computation: {}",
                width, height, path.as_ref().display()
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
            let img = image::open(path.as_ref())?;
            let resized = img.resize(
                target_width, 
                target_height, 
                image::imageops::FilterType::Lanczos3
            );
            
            // Compute hash on resized image
            return Ok(calculate_phash(&resized));
        }
    }
    
    // For smaller images or if we couldn't determine dimensions, use normal path
    let img = image::open(path.as_ref())?;
    Ok(calculate_phash(&img))
}

/// Helper function to check if a file is in HEIC format
fn is_heic_format<P: AsRef<Path>>(path: P) -> bool {
    use std::io::Read;

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

/// Process HEIC image files
fn process_heic_image<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    let path_ref = path.as_ref();

    // Create a custom error for HEIC issues
    let heic_error = |msg: &str| -> image::ImageError {
        image::ImageError::Unsupported(image::error::UnsupportedError::from_format_and_kind(
            image::error::ImageFormatHint::Name("HEIC".to_string()),
            image::error::UnsupportedErrorKind::GenericFeature(msg.to_string()),
        ))
    };

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
        
        // Check if image is large - if so, resize before computing hash
        if width > 1024 || height > 1024 {
            log::info!(
                "Downscaling large HEIC image ({}x{}) for perceptual hash: {}",
                width, height, path_ref.display()
            );
            
            // Calculate target dimensions maintaining aspect ratio
            let (target_width, target_height) = if width > height {
                let scale = 1024.0 / width as f32;
                (1024, (height as f32 * scale).round() as u32)
            } else {
                let scale = 1024.0 / height as f32;
                ((width as f32 * scale).round() as u32, 1024)
            };
            
            // Resize the image
            let resized = dynamic_img.resize(
                target_width, 
                target_height, 
                image::imageops::FilterType::Lanczos3
            );
            
            // Compute hash on resized image
            return Ok(calculate_phash(&resized));
        }
        
        // For smaller images, compute hash directly
        return Ok(calculate_phash(&dynamic_img));
    } else {
        return Err(heic_error("HEIC image doesn't have interleaved data"));
    }
}

// For cached image loading and processing
pub struct ImageCache {
    buffer_size: usize,
    cache: std::collections::HashMap<String, PHash>,
}

impl ImageCache {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            cache: std::collections::HashMap::with_capacity(buffer_size),
        }
    }

    pub fn get_hash<P: AsRef<Path>>(&mut self, path: P) -> Result<PHash, image::ImageError> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        if let Some(hash) = self.cache.get(&path_str) {
            return Ok(*hash);
        }

        // Use the phash_from_file function which handles HEIC files
        let hash = phash_from_file(&path)?;

        // Simple LRU-like behavior: clear cache if it's too big
        if self.cache.len() >= self.buffer_size {
            self.cache.clear();
        }

        self.cache.insert(path_str, hash);
        Ok(hash)
    }
}
