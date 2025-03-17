use std::path::PathBuf;
use image_deduper_core::{SimpleDeduper, logging, Error, Result as DedupeResult};

fn main() -> DedupeResult<()> {
    // Initialize logging
    logging::init_logger("./logs").map_err(|e| Error::Unknown(format!("Failed to initialize logger: {}", e)))?;
    
    println!("Starting simplified image scanning and hashing...");
    
    // Directory to scan
    let scan_directory = PathBuf::from("/Volumes/SamsungT9/Mylio_22c15a");
    
    // Directory to exclude
    let excluded_dir = PathBuf::from("/Volumes/SamsungT9/Mylio_22c15a/Generated Images.bundle");
    println!("Scanning directory: {}", scan_directory.display());
    
    // Create the deduper with custom configuration
    let deduper = SimpleDeduper::new()
        .with_threads(num_cpus::get())
        .with_database("image-deduper.db")
        .with_batch_size(100)
        .with_excluded_directories(vec![excluded_dir]);
    
    // Run the deduplication process
    let result = deduper.run(&[scan_directory]);
    
    match result {
        Ok(processed_images) => {
            println!("Successfully processed {} images", processed_images.len());
            
            // Find duplicates
            let duplicate_groups = deduper.find_duplicates(&processed_images);
            println!("Found {} duplicate groups", duplicate_groups.len());
            
            // Print some statistics about duplicates
            let total_duplicates: usize = duplicate_groups.iter().map(|g| g.len() - 1).sum();
            println!("Total duplicates: {}", total_duplicates);
            
            // Print a few examples
            if !duplicate_groups.is_empty() {
                println!("\nSample duplicate group:");
                for img in &duplicate_groups[0] {
                    println!("  - {}", img.path.display());
                }
            }
            
            Ok(())
        },
        Err(e) => {
            println!("Error processing images: {}", e);
            Err(e)
        }
    }
}