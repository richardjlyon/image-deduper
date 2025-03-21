/// Demonstrates the discovery of images in a directory.
use std::path::PathBuf;

use image_deduper_core::logging;
use image_deduper_core::Config;
use image_deduper_core::ImageDeduper;
use image_deduper_core::LogLevel;
use image_deduper_core::Result;

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
    // Initialize remote logging
    logging::init_logger(false)?;

    println!("Initializing image deduplication process...");
    println!("Press Ctrl+C to gracefully stop processing");

    // Create configuration with specific values
    let config = Config {
        database_name: Some(String::from("test_image_hash_db")),
        excluded_directories: vec![PathBuf::from(
            "/Volumes/SamsungT9/Mylio_22c15a/Generated Images.bundle",
        )],
        reinitialise_database: true,
        log_level: LogLevel::Debug,
        ..Default::default()
    };

    let deduper = ImageDeduper::new(config);

    println!("Discovering images...");
    let images = deduper.discover_images(&[PathBuf::from("/Volumes/SamsungT9/Mylio_22c15a")])?;

    println!("Found {} images", images.len());

    Ok(())
}
