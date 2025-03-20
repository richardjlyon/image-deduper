/// Utility to detect and fix incorrect image file suffixes
/// This will detect files with incorrect extensions, particularly HEIC files with .jpg extensions
/// and rename them to have the correct extension.
use image_deduper_core::logging;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;
use walkdir::WalkDir;

// Magic bytes for various image formats
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47];
const GIF_MAGIC: &[u8] = &[0x47, 0x49, 0x46, 0x38];
const WEBP_MAGIC: &[u8] = &[0x52, 0x49, 0x46, 0x46]; // RIFF header, with WEBP at offset 8

// HEIC can have several signature patterns
fn is_heic_format(buffer: &[u8]) -> bool {
    if buffer.len() < 12 {
        return false;
    }

    // Check for various HEIC signatures
    (buffer[4..8] == [b'f', b't', b'y', b'p'])
        || (buffer[4..8] == [b'h', b'e', b'i', b'c'])
        || (buffer[4..8] == [b'h', b'e', b'i', b'f'])
        || (buffer[4..8] == [b'm', b'i', b'f', b'1'])
}

fn detect_image_format(path: &Path) -> Option<&'static str> {
    if let Ok(mut file) = fs::File::open(path) {
        let mut buffer = [0; 12];
        if file.read_exact(&mut buffer).is_ok() {
            // Check for HEIC first
            if is_heic_format(&buffer) {
                return Some("heic");
            }

            // Check for JPEG
            if buffer.starts_with(JPEG_MAGIC) {
                return Some("jpg");
            }

            // Check for PNG
            if buffer.starts_with(PNG_MAGIC) {
                return Some("png");
            }

            // Check for GIF
            if buffer.starts_with(GIF_MAGIC) {
                return Some("gif");
            }

            // Check for WebP (RIFF header + "WEBP" at offset 8)
            if buffer.starts_with(WEBP_MAGIC) && buffer.len() >= 12 && &buffer[8..12] == b"WEBP" {
                return Some("webp");
            }
        }
    }
    None
}

fn extension_matches_format(path: &Path, detected_format: &str) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();

        match detected_format {
            "jpg" => ext_str == "jpg" || ext_str == "jpeg" || ext_str == "jpe" || ext_str == "jfif",
            "png" => ext_str == "png",
            "gif" => ext_str == "gif",
            "webp" => ext_str == "webp",
            "heic" => ext_str == "heic" || ext_str == "heif",
            _ => false,
        }
    } else {
        false
    }
}

fn rename_with_correct_extension(
    path: &Path,
    detected_format: &str,
    dry_run: bool,
) -> Result<PathBuf, String> {
    // Get the stem (filename without extension)
    let stem = path
        .file_stem()
        .ok_or_else(|| format!("Could not get file stem for {}", path.display()))?
        .to_string_lossy();

    // Create new path with correct extension
    let parent = path.parent().unwrap_or(Path::new(""));
    let new_path = parent.join(format!("{}.{}", stem, detected_format));

    // Ensure we don't overwrite existing files
    if new_path.exists() {
        return Err(format!(
            "Cannot rename: target file already exists: {}",
            new_path.display()
        ));
    }

    if !dry_run {
        // Perform the rename
        fs::rename(path, &new_path).map_err(|e| format!("Failed to rename file: {}", e))?;

        println!("Renamed: {} -> {}", path.display(), new_path.display());
    } else {
        println!("Would rename: {} -> {}", path.display(), new_path.display());
    }

    Ok(new_path)
}

fn process_image(path: &Path, dry_run: bool) -> Result<(), String> {
    // Check if the path exists and is a file
    if !path.exists() || !path.is_file() {
        return Err(format!(
            "Path does not exist or is not a file: {}",
            path.display()
        ));
    }

    // Detect the actual format based on magic bytes
    if let Some(detected_format) = detect_image_format(path) {
        // Check if the extension matches the detected format
        if !extension_matches_format(path, detected_format) {
            println!(
                "Found file with incorrect extension: {} (actual format: {})",
                path.display(),
                detected_format
            );

            // Rename the file with the correct extension
            rename_with_correct_extension(path, detected_format, dry_run)?;
        }
    } else {
        println!(
            "Warning: Could not determine format for file: {}",
            path.display()
        );
    }

    Ok(())
}

fn process_directory(dir_path: &Path, recursive: bool, dry_run: bool) -> Result<(), String> {
    let walker = if recursive {
        WalkDir::new(dir_path)
    } else {
        WalkDir::new(dir_path).max_depth(1)
    };

    let mut processed = 0;
    let mut renamed = 0;

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Only process files that look like images
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if [
                "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "jpe", "jfif",
            ]
            .contains(&ext_str.as_ref())
            {
                processed += 1;

                match process_image(path, dry_run) {
                    Ok(()) => {
                        if !extension_matches_format(
                            path,
                            &detect_image_format(path).unwrap_or("unknown"),
                        ) {
                            renamed += 1;
                        }
                    }
                    Err(e) => println!("Error processing {}: {}", path.display(), e),
                }
            }
        }
    }

    println!(
        "Processed {} image files, renamed {} files with incorrect extensions",
        processed, renamed
    );

    Ok(())
}

fn print_usage() {
    println!("Usage: fixsuffix [OPTIONS] <PATH> [<PATH>...]");
    println!("Options:");
    println!("  --help           Show this help message");
    println!("  --recursive, -r  Process directories recursively");
    println!("  --dry-run        Don't actually rename files, just show what would be done");
    println!("");
    println!("Examples:");
    println!("  fixsuffix image.jpg                    # Check and fix a single file");
    println!(
        "  fixsuffix --recursive ~/Pictures       # Process all images in Pictures recursively"
    );
    println!("  fixsuffix --dry-run ~/Photos/*.jpg     # Preview changes without renaming");
}

fn main() {
    // Initialize logging
    if let Err(e) = logging::init_logger() {
        eprintln!("Warning: Could not initialize logger: {}", e);
    }

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) {
        print_usage();
        return;
    }

    let mut paths = Vec::new();
    let mut recursive = false;
    let mut dry_run = false;

    // Parse command line arguments
    for arg in &args[1..] {
        match arg.as_str() {
            "--recursive" | "-r" => recursive = true,
            "--dry-run" => dry_run = true,
            _ => {
                if !arg.starts_with("-") {
                    paths.push(PathBuf::from(arg));
                } else {
                    eprintln!("Unknown option: {}", arg);
                    print_usage();
                    process::exit(1);
                }
            }
        }
    }

    if paths.is_empty() {
        eprintln!("Error: No paths specified");
        print_usage();
        process::exit(1);
    }

    println!("Image Extension Fixer");
    println!(
        "Mode: {}",
        if dry_run {
            "Dry run (no changes)"
        } else {
            "Live run"
        }
    );
    println!(
        "Processing {} path(s){}...",
        paths.len(),
        if recursive { " recursively" } else { "" }
    );

    for path in &paths {
        if path.is_dir() {
            if let Err(e) = process_directory(path, recursive, dry_run) {
                eprintln!("Error processing directory {}: {}", path.display(), e);
            }
        } else {
            if let Err(e) = process_image(path, dry_run) {
                eprintln!("Error processing file {}: {}", path.display(), e);
            }
        }
    }

    println!("Done!");
}
