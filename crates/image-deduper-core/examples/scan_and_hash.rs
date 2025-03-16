use image_deduper_core::{config::LogLevel, Config, ImageDeduper};
use log::{error, info};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create a basic configuration
    let mut config = Config::default();
    config.log_level = LogLevel::Debug;
    config.max_depth = Some(5); // Limit directory depth
    config.process_unsupported_formats = false;
    config.threads = num_cpus::get(); // Use all available CPUs

    // Create the deduper instance
    let deduper = ImageDeduper::new(config);

    // Specify directories to scan (relative to the crate directory)
    let directories = vec![PathBuf::from("test_data/real_images")];

    // Run and handle any errors
    match deduper.run(&directories, false) {
        // false = don't force rescan
        Ok(_) => {
            info!("Successfully processed all images");
            Ok(())
        }
        Err(e) => {
            error!("Error processing images: {}", e);
            Err(e.into())
        }
    }
}
