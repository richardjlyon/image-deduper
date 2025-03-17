use image_deduper_core::{
    config::{LogLevel, PriorityRule},
    logging, Config, ImageDeduper,
};
use log::{error, info};
use std::{path::PathBuf, sync::Arc};
use std::sync::atomic::{AtomicBool, Ordering};
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
    println!("Press Ctrl+C to gracefully stop processing");

    // Setup signal handler for SIGINT (Ctrl+C)
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl+C. Initiating graceful shutdown...");
        r.store(false, Ordering::SeqCst);
    })?;

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
        excluded_directories: vec![PathBuf::from(
            "/Volumes/SamsungT7/mylio-vault-backup-250317/Generated Images.bundle",
        )],
    };

    // Create the deduper instance
    let deduper = Arc::new(ImageDeduper::new(config));

    println!("Configuration complete. Starting image scan...");

    // Specify directories to scan (relative to the crate directory)
    let directories = vec![PathBuf::from(
        "/Volumes/SamsungT7/mylio-vault-backup-250317",
    )];
    println!("Scanning directories: {:?}", directories);

    // Spawn a monitoring thread that checks for shutdown signal
    let deduper_clone = Arc::clone(&deduper);
    let running_ref = running.clone();
    
    std::thread::spawn(move || {
        while running_ref.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        // Signal shutdown to the deduper
        println!("Graceful shutdown in progress. Closing database connections and checkpointing WAL...");
        
        // Get database path from config
        let db_path = PathBuf::from("image-deduper.db");
        
        // First checkpoint the WAL file to ensure all changes are in the main database file
        if std::path::Path::new(&db_path).exists() {
            if let Ok(conn) = rusqlite::Connection::open(&db_path) {
                match conn.query_row("PRAGMA wal_checkpoint(FULL)", [], |_| { Ok(()) }) {
                    Ok(_) => println!("Database WAL checkpoint complete - changes saved to main database file"),
                    Err(e) => println!("Warning: Unable to checkpoint database: {}", e)
                }
            }
        }
        
        // Now request the main shutdown
        deduper_clone.request_shutdown();
        println!("Waiting for current operations to complete...");
    });

    // Run and handle any errors
    match deduper.run(&directories, false) {
        // false = don't force rescan
        Ok(_) => {
            // Log to file
            info!("Successfully processed all images");
            Ok(())
        }
        Err(e) => {
            if e.to_string().contains("Shutdown requested") {
                println!("Processing was gracefully interrupted by user request.");
                info!("Processing gracefully interrupted by user request");
                Ok(())
            } else {
                // Log detailed error to file only
                error!("Error processing images: {}", e);
                // Return Err but it won't be printed to console because
                // this error is handled in the main function
                Err(e.into())
            }
        }
    }
}
