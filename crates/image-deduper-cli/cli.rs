use clap::{Parser, Subcommand};
use std::path::PathBuf;
use log::{info, warn, error};
use image_deduper_core::{Config, ImageDeduper};

#[derive(Parser)]
#[command(name = "image-deduper")]
#[command(about = "Safely deduplicate image files")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan directories for duplicate images
    Scan {
        /// Directories to scan for duplicate images
        #[arg(required = true)]
        directories: Vec<PathBuf>,

        /// Where to store duplicate images (instead of deleting)
        #[arg(long, default_value = "duplicates")]
        duplicates_dir: PathBuf,

        /// Run without making changes
        #[arg(long)]
        dry_run: bool,

        /// Delete duplicates instead of moving them
        #[arg(long)]
        delete: bool,

        /// Create symbolic links to originals instead of keeping duplicates
        #[arg(long)]
        symlinks: bool,

        /// Verbosity level
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,

        /// Path to configuration file
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Generate default configuration file
    GenerateConfig {
        /// Path to save configuration file
        #[arg(default_value = "image-deduper.json")]
        path: PathBuf,
    },
}

fn main() -> Result<(), anyhow::Error> {
    // Initialize logger
    env_logger::init();

    // Parse command line arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            directories,
            duplicates_dir,
            dry_run,
            delete,
            symlinks,
            verbose,
            config,
        } => {
            // Set up configuration
            let mut config = if let Some(config_path) = config {
                // Load config from file
                Config::from_file(&config_path)?
            } else {
                Config::default()
            };

            // Override config with command line arguments
            config.dry_run = dry_run;
            config.duplicates_dir = duplicates_dir;
            config.delete_duplicates = delete;
            config.create_symlinks = symlinks;

            // Set log level based on verbosity
            config.log_level = match verbose {
                0 => image_deduper_core::config::LogLevel::Info,
                1 => image_deduper_core::config::LogLevel::Debug,
                _ => image_deduper_core::config::LogLevel::Trace,
            };

            // Validate configuration
            config.validate()?;

            // Initialize deduplicator
            let deduper = ImageDeduper::new(config);

            // Run the deduplication process
            info!("Starting image deduplication...");
            deduper.run(&directories)?;
            info!("Deduplication complete");

            Ok(())
        },

        Commands::GenerateConfig { path } => {
            let config = Config::default();
            config.save_to_file(&path)?;
            println!("Configuration file generated at: {}", path.display());
            Ok(())
        },
    }
}
