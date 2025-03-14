use image_deduper_core::{discovery, Config};
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get directory to scan from command line or use current directory
    let args: Vec<String> = env::args().collect();
    let directory = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        env::current_dir()?
    };

    println!("Scanning directory: {}", directory.display());

    // Create default configuration
    let config = Config::default();

    // Discover images
    let images = discovery::discover_images(&[directory], &config)?;

    // Print results
    println!("Found {} images:", images.len());
    for (i, img) in images.iter().enumerate() {
        println!("{}: {} ({} bytes)", i + 1, img.path.display(), img.size);
    }

    Ok(())
}
