use image_deduper_core::logging;
use image_deduper_core::logging::shutdown_logger;
use image_deduper_core::Config;
use image_deduper_core::ImageDeduper;
use image_deduper_core::LogLevel;
use image_deduper_core::PriorityRule;
use image_deduper_core::Result;
use log::info;
/// Demonstrates the processing of images in a directory.
use std::path::PathBuf;

fn main() -> Result<()> {
    logging::init_logger()?;

    if let Err(e) = run_app() {
        log::error!("Application error: {}", e);
        println!("Error: Processing failed. See logs for details.");
        std::process::exit(1);
    }

    println!("Process completed.");

    Ok(())
}

fn run_app() -> Result<()> {
    println!("Initializing image deduplication process...");
    println!("Press Ctrl+C to gracefully stop processing");

    let source_directory = PathBuf::from("/Volumes/SamsungT9/Mylio_22c15a");

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
        database_path: Some(PathBuf::from("image_hash_db")),
        batch_size: Some(100),
        log_level: LogLevel::Debug,
        use_gpu_acceleration: false, // Enable GPU acceleration
        excluded_directories: vec![PathBuf::from(
            "/Volumes/SamsungT9/Mylio_22c15a/Generated Images.bundle",
        )],
    };

    let deduper = ImageDeduper::new(config);

    info!("Indexing {} for images...", source_directory.display());
    let mut images = deduper.discover_images(&[source_directory])?;
    info!("Found {} images", images.len());

    // Sort images by size
    images.sort_by_key(|image| image.size);
    info!("Processing images...");

    // Use force_rescan=true to process all test images
    info!("Calling hash_and_persist...");
    let _ = deduper.hash_and_persist(&images, false)?;

    // Display final database statistics
    let (final_pc_count, final_pp_count) = deduper.get_db_stats()?;
    println!("\nFinal database contents:");
    println!("  - Cryptographic hashes: {}", final_pc_count);
    println!("  - Perceptual hashes: {}", final_pp_count);
    println!("  - Total unique images: {}", final_pc_count);

    shutdown_logger();

    Ok(())
}
