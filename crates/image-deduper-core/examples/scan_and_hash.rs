use image_deduper_core::{
    config::{LogLevel, PriorityRule},
    logging, Config, ImageDeduper,
};
use log::{error, info};
use std::path::PathBuf;
fn main() {
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
    
    println!("Process completed successfully!");
}

// Move all your application logic to this function
fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging (now file-only)
    logging::init_logger("./logs")?;
    
    println!("Initializing image deduplication process...");

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
    };

    // Create the deduper instance
    let deduper = ImageDeduper::new(config);
    
    println!("Configuration complete. Starting image scan...");

    // Specify directories to scan (relative to the crate directory)
    let directories = vec![PathBuf::from("/Volumes/SamsungT9/Mylio_22c15a")];
    println!("Scanning directories: {:?}", directories);

    // Run and handle any errors
    match deduper.run(&directories, false) {
        // false = don't force rescan
        Ok(_) => {
            // Log to file
            info!("Successfully processed all images");
            Ok(())
        }
        Err(e) => {
            // Log detailed error to file only
            error!("Error processing images: {}", e);
            // Return Err but it won't be printed to console because
            // this error is handled in the main function
            Err(e.into())
        }
    }
}
