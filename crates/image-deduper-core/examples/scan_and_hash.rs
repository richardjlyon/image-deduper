use image_deduper_core::{
    config::{LogLevel, PriorityRule},
    logging, Config, ImageDeduper,
};
use log::{error, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{path::PathBuf, sync::Arc};
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

    // The detailed messages will be printed by the function itself.
    // This message will be shown only in case of overall success
    println!("Process completed.");
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
        use_gpu_acceleration: true, // Enable GPU acceleration
        excluded_directories: vec![PathBuf::from(
            "/Volumes/SamsungT9/Mylio_22c15a/Generated Images.bundle",
        )],
    };

    // Create the deduper instance
    let deduper = Arc::new(ImageDeduper::new(config));

    println!("Configuration complete. Starting image scan...");

    // Specify directories to scan (relative to the crate directory)
    let directories = vec![PathBuf::from("/Volumes/SamsungT9/Mylio_22c15a")];
    println!("Scanning directories: {:?}", directories);

    // Spawn a monitoring thread that checks for shutdown signal
    let deduper_clone = Arc::clone(&deduper);
    let running_ref = running.clone();

    // Create a second thread to act as a watchdog for the shutdown process itself
    // This ensures that even if the main shutdown handler gets stuck, we have a backup
    let r2 = running.clone();
    let watchdog = std::thread::spawn(move || {
        // First wait for shutdown signal
        while r2.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
        
        // Once shutdown is requested, set an absolute maximum time limit
        println!("Watchdog activated - will force exit in 30 seconds if necessary");
        std::thread::sleep(std::time::Duration::from_secs(30));
        
        // If we get here, something is badly stuck - force exit
        println!("\n!!! WATCHDOG TRIGGERED !!!");
        println!("Application is not responding to normal shutdown requests.");
        println!("Forcing immediate exit to prevent hang.");
        std::process::exit(1); // Emergency exit
    });

    // Main shutdown handler thread
    std::thread::spawn(move || {
        // Wait for Ctrl+C signal
        while running_ref.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        
        // Signal shutdown to the deduper
        println!("\nGraceful shutdown in progress...");
        println!("(Press Ctrl+C again for immediate exit)");

        // Set up a second Ctrl+C handler for force exit during shutdown
        let force_exit = Arc::new(AtomicBool::new(false));
        let fe = force_exit.clone();
        if let Ok(()) = ctrlc::set_handler(move || {
            println!("\nForce exit requested. Exiting immediately...");
            fe.store(true, Ordering::SeqCst);
            std::process::exit(0);
        }) {
            println!("Emergency exit handler activated");
        }

        // IMPORTANT: Do NOT perform database operations during shutdown
        // SQLite WAL mode ensures database consistency even when interrupted

        // Now request the main shutdown
        deduper_clone.request_shutdown();
        println!("Waiting for current operations to complete...");
        
        // Add a timeout that will force exit if shutdown takes too long
        let mut timeout_counter = 0;
        let max_wait_seconds = 15; // Shorter timeout because we now have multiple safeguards
        
        while timeout_counter < max_wait_seconds * 2 {
            // Check if force exit requested
            if force_exit.load(Ordering::SeqCst) {
                break;
            }
            
            std::thread::sleep(std::time::Duration::from_millis(500));
            timeout_counter += 1;
            
            // Print a countdown every 2 seconds
            if timeout_counter % 4 == 0 {
                let remaining = max_wait_seconds - (timeout_counter / 2);
                println!("Still waiting... (forcing exit in {} seconds)", remaining);
            }
        }
        
        // If we reached here, force exit
        println!("Graceful shutdown timeout exceeded. Forcing exit...");
        println!("Note: SQLite WAL mode ensures database integrity even on forced exit");
        std::process::exit(0);
    });

    // Always use efficient mode (default behavior)
    // Only process new or modified images, not ones already in the database
    let force_rescan = false;
    
    println!("\nEFFICIENT MODE ENABLED");
    println!("Only new or modified images will be processed.");
    println!("Images already in database will be skipped for efficiency.\n");
    
    // First discover the images to get the total count - this doesn't process them yet
    let found_images = match deduper.discover_images(&directories) {
        Ok(images) => images,
        Err(e) => {
            error!("Error discovering images: {}", e);
            return Err(e.into());
        }
    };
    
    // Store the total count for reporting
    let total_images = found_images.len();
    info!("Found {} total images to scan", total_images);
    println!("Found {} total images to scan", total_images);
    
    // Run and handle any errors
    match deduper.run(&directories, force_rescan) {
        // Explicitly show force_rescan mode in comments
        Ok(processed_images) => {
            // Log to file with more detailed info
            let total_count = processed_images.len();
            info!("Successfully processed all images - got {} results", total_count);
            
            // Print total numbers to console for clarity
            println!("Process completed - successfully processed {} images", total_count);
            
            if total_count < 1000 && total_images > 1000 {
                // Add message celebrating efficiency 
                let skipped = total_images - total_count;
                let skipped_pct = (skipped as f64 / total_images as f64) * 100.0;
                
                println!("\nEFFICIENCY REPORT:");
                println!("✅ Only {} images needed processing out of {} total images", 
                        total_count, total_images);
                println!("✅ Saved processing time for {} images ({:.1}%) already in database", 
                        skipped, skipped_pct);
            }
            
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
