use image_deduper_core::logging;
use image_deduper_core::logging::shutdown_logger;
use image_deduper_core::Config;
use image_deduper_core::ImageDeduper;
use image_deduper_core::LogLevel;
use image_deduper_core::Result;
use log::info;
/// Demonstrates the processing of images in a directory.
use std::path::PathBuf;

fn main() -> Result<()> {
    logging::init_logger(false)?;

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

    // let source_directory = PathBuf::from("/Volumes/SamsungT9/Mylio_22c15a");
    let source_directory = PathBuf::from(
        "/Users/richardlyon/dev/mine/rust/image-deduper/crates/image-deduper-core/tests/data",
    );

    let config = Config {
        database_name: Some(String::from("test_image_hash_db")),
        reinitialise_database: true,
        threads: num_cpus::get(), // Use all available CPUs
        batch_size: Some(10),
        log_level: LogLevel::Debug,
        ..Default::default()
    };

    let deduper = ImageDeduper::new(&config);

    info!("Indexing {} for images...", source_directory.display());
    let images = deduper.discover_images(&[source_directory])?;
    info!("Found {} images", images.len());

    // Use force_rescan=true to process all test images
    info!("Calling hash_and_persist...");
    let (final_pc_count, final_pp_count) = deduper.hash_and_persist(&images, &config)?;

    // Display final database statistics
    println!("\nFinal database contents:");
    println!("  - Cryptographic hashes: {}", final_pc_count);
    println!("  - Perceptual hashes: {}", final_pp_count);
    println!("  - Total unique images: {}", final_pc_count);

    shutdown_logger();

    Ok(())
}
