use std::path::PathBuf;

use image_deduper_core::logging;
use image_deduper_core::Config;
use image_deduper_core::ImageDeduper;
use image_deduper_core::LogLevel;
use image_deduper_core::PriorityRule;
/// Demonstrates the discovery of images in a directory.
use image_deduper_core::Result;
use log::info;

fn main() -> Result<()> {
    // Print informative message to user
    println!("Starting image scanning and hashing...");

    // Run the application and handle any errors within this function
    if let Err(e) = run_app() {
        // Log the error to file but not console
        log::error!("Application error: {}", e);
        // Show minimal error to console
        println!("Error: Processing failed. See logs for details.");
        // Exit with a non-zero code to indicate failure
        std::process::exit(1);
    }

    // The detailed messages will be printed by the function itself.
    // This message will be shown only in case of overall success
    println!("Process completed.");

    Ok(())
}

fn run_app() -> Result<()> {
    // Initialize logging (now file-only)
    logging::init_logger("./logs")?;

    println!("Initializing image deduplication process...");
    println!("Press Ctrl+C to gracefully stop processing");

    // Create configuration with specific values
    let config = Config {
        dry_run: true, // Safe default
        duplicates_dir: PathBuf::from("duplicates"),
        delete_duplicates: false,
        create_symlinks: false,
        phash_threshold: 90,
        generate_thumbnails: true,
        backup_dir: Some(PathBuf::from("backup")),
        max_depth: Some(5), // Limit directory depth
        process_unsupported_formats: false,
        threads: num_cpus::get(), // Use all available CPUs
        prioritization: vec![
            PriorityRule::HighestResolution,
            PriorityRule::LargestFileSize,
            PriorityRule::OldestCreationDate,
        ],
        use_database: true,
        database_path: Some(PathBuf::from("image-deduper.db")),
        batch_size: Some(100),
        log_level: LogLevel::Debug,
        use_gpu_acceleration: false, // Enable GPU acceleration
        excluded_directories: vec![PathBuf::from(
            "/Volumes/SamsungT9/Mylio_22c15a/Generated Images.bundle",
        )],
    };

    let deduper = ImageDeduper::new(config);

    println!("Discovering images...");
    let images = deduper.discover_images(&[PathBuf::from("/Volumes/SamsungT9/Mylio_22c15a")])?;

    println!("Found {} images", images.len());

    Ok(())
}
