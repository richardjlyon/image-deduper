//! Core functionality for finding and managing duplicate images.
//!
//! This library provides the foundational components for image deduplication:
//! - File discovery and metadata extraction
//! - Image processing and hash generation
//! - Duplicate detection algorithms
//! - Safe file operations

// -- External Dependencies --
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, trace, warn};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rayon::prelude::*;

// -- Standard Library --
use std::{path::Path, sync::{Arc, atomic::{AtomicBool, Ordering}}};

// -- Internal Modules --
mod error;

// -- Public Re-exports --
pub use config::*;
pub use error::{Error, Result};
pub use types::*;

// -- Public Modules --
pub mod action;
pub mod config;
pub mod discovery;
pub mod logging;
pub mod persistence;
pub mod processing;
pub mod safety;
pub mod types;
// pub mod deduplication;

// -- Test Modules --
#[cfg(test)]
pub mod test_utils;

/// Main entry point for the deduplication process
pub struct ImageDeduper {
    config: Config,
    _safety_manager: safety::SafetyManager,
    shutdown_requested: Arc<AtomicBool>,
}

impl ImageDeduper {
    /// Create a new ImageDeduper with the provided configuration
    pub fn new(config: Config) -> Self {
        let _safety_manager = safety::SafetyManager::new(&config);
        Self {
            config,
            _safety_manager,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Signal handler method to request a graceful shutdown
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        info!("Shutdown requested. Finishing current operations...");
        
        // CRITICAL: Do NOT attempt to checkpoint or vacuum during Ctrl+C
        // This is counterintuitive, but when SQLite is interrupted,
        // additional operations are more likely to corrupt the database
        // 
        // Instead, we rely on SQLite's WAL mode to ensure that the
        // database remains consistent, and let it recover on the next
        // application start
        info!("SQLite WAL mode is active - database will auto-recover on next start");
    }
    
    /// Check if shutdown has been requested
    fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    /// Run the full deduplication pipeline
    pub fn run(&self, directories: &[impl AsRef<Path>], force_rescan: bool) -> Result<()> {
        // Discover images
        info!("Discovering images...");
        let images = self.discover_images(directories)?;
        info!("Found {} images", images.len());
        
        // Check if database is valid before starting processing
        self.check_database_health()?;

        // Process and persist images
        info!("Processing images...");
        let processed_images = self.process_images(images, force_rescan)?;
        info!("Processed {} images", processed_images.len());
        
        // Run database integrity check after processing
        self.verify_database_integrity()?;

        // // Find duplicates
        // let duplicate_groups = self.find_duplicates(processed_images)?;

        // Take action
        // if self.config.dry_run {
        //     self.preview_actions(&duplicate_groups)
        // } else {
        //     self.execute_deduplication(&duplicate_groups)
        // }

        Ok(())
    }

    /// Discover all images in the provided directories
    pub fn discover_images(
        &self,
        directories: &[impl AsRef<Path>],
    ) -> Result<Vec<types::ImageFile>> {
        discovery::discover_images(directories, &self.config)
    }

    /// Process images to generate hashes and extract metadata
    fn process_images(
        &self,
        images: Vec<types::ImageFile>,
        force_rescan: bool,
    ) -> Result<Vec<types::ProcessedImage>> {
        let total_images = images.len();
        info!(
            "Starting processing of {} images, force_rescan={}",
            total_images, force_rescan
        );

        // Set up progress bar
        let progress = ProgressBar::new(total_images as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );

        // Create a thread-safe progress bar
        let progress = Arc::new(progress);

        // Get database path from config or use default
        let db_path = if self.config.use_database {
            self.config
                .database_path
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("image_deduper.db"))
        } else {
            std::path::PathBuf::from("image_deduper.db")
        };
        debug!("Using database path: {}", db_path.display());

        // Create a connection pool with better timeout handling
        let manager = SqliteConnectionManager::file(&db_path);
        
        // Higher concurrency while maintaining stability
        // We'll use more connections but with better timeout handling
        let cpu_count = num_cpus::get() as u32;
        // Use a more modest connection pool to prevent SQLite contention
        let max_connections = std::cmp::min(std::cmp::max(8, cpu_count), 24); // Between 8 and 24 connections
        
        // Configure pool with improved settings for performance
        let pool = match Pool::builder()
            .max_size(max_connections)
            .min_idle(Some(8)) // Increased idle connections to reduce waits
            .connection_timeout(std::time::Duration::from_secs(5)) // Longer timeout to allow for busy periods
            .build(manager)
        {
            Ok(pool) => pool,
            Err(e) => {
                // Use our logger helper for db file operation
                logging::log_file_error(&db_path, "create_connection_pool", &e);
                return Err(Error::Unknown(format!(
                    "Failed to create connection pool: {}",
                    e
                )));
            }
        };
        
        // Set WAL journal mode and other optimizations
        if let Ok(conn) = pool.get() {
            let _ = conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA temp_store = MEMORY;
                 PRAGMA cache_size = 20000;
                 PRAGMA busy_timeout = 30000;
                 PRAGMA mmap_size = 30000000000;
                 PRAGMA threads = 4;
                 PRAGMA page_size = 32768;"
            );
            
            // Verify settings for debugging
            if let Ok(busy_timeout) = conn.query_row::<i32, _, _>("PRAGMA busy_timeout", [], |r| r.get(0)) {
                info!("SQLite busy_timeout: {}ms", busy_timeout);
            }
            if let Ok(journal_mode) = conn.query_row::<String, _, _>("PRAGMA journal_mode", [], |r| r.get(0)) {
                info!("SQLite journal_mode: {}", journal_mode);
            }
            if let Ok(sync_mode) = conn.query_row::<i32, _, _>("PRAGMA synchronous", [], |r| r.get(0)) {
                info!("SQLite synchronous: {}", sync_mode);
            }
            if let Ok(cache_size) = conn.query_row::<i32, _, _>("PRAGMA cache_size", [], |r| r.get(0)) {
                info!("SQLite cache_size: {}", cache_size);
            }
            if let Ok(page_size) = conn.query_row::<i32, _, _>("PRAGMA page_size", [], |r| r.get(0)) {
                info!("SQLite page_size: {}", page_size);
            }
            if let Ok(threads) = conn.query_row::<i32, _, _>("PRAGMA threads", [], |r| r.get(0)) {
                info!("SQLite threads: {}", threads);
            }
        }
        
        info!("HIGH PERFORMANCE: Database connection pool created with {} max connections", max_connections);

        // Initialize the database schema
        let conn = match pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to get database connection: {}", e);
                return Err(Error::Unknown(format!(
                    "Failed to get database connection: {}",
                    e
                )));
            }
        };

        if let Err(e) = persistence::initialize_database(&conn) {
            error!("Failed to initialize database: {}", e);
            return Err(e.into());
        }
        debug!("Database initialized successfully");

        // Process images in smaller chunks for better parallelism
        // Smaller chunks allow more even distribution of work
        let chunk_size = 100; // Reduced chunk size to minimize database contention
        let mut processed = Vec::with_capacity(total_images);
        let mut failed_images = Vec::new();
        
        // Performance metrics tracking
        let start_time = std::time::Instant::now();
        let newly_processed_count = std::sync::atomic::AtomicUsize::new(0);
        
        // Only configure the thread pool if we haven't already
        // This avoids the "already initialized" warning when running multiple times
        static THREAD_POOL_INITIALIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        
        if !THREAD_POOL_INITIALIZED.load(std::sync::atomic::Ordering::SeqCst) {
            // Set Rayon global thread pool with higher thread count
            // This is separate from the DB connection pool
            if let Ok(_) = rayon::ThreadPoolBuilder::new()
                .num_threads(std::cmp::min(num_cpus::get() + 2, 16)) // More conservative thread count
                .build_global() 
            {
                THREAD_POOL_INITIALIZED.store(true, std::sync::atomic::Ordering::SeqCst);
                info!("Rayon thread pool initialized with {} threads", std::cmp::min(num_cpus::get() + 2, 16));
            } else {
                warn!("Thread pool already initialized or configuration failed");
            }
        }

        'chunks: for (chunk_index, chunk) in images.chunks(chunk_size).enumerate() {
            // Check for shutdown signal before starting each chunk
            if self.is_shutdown_requested() {
                info!("Shutdown requested. Stopping image processing.");
                progress.finish_with_message("Processing interrupted - shutdown requested");
                break 'chunks;
            }
            
            let chunk_message = format!("Processing chunk {}", chunk_index + 1);
            progress.set_message(chunk_message.clone());
            debug!("{} ({} images)", chunk_message, chunk.len());

            let chunk_results: Vec<_> = chunk
                .par_iter()
                .map(|img| {
                    let progress = Arc::clone(&progress);
                    let pool = Arc::new(pool.clone());
                    let shutdown_requested = Arc::clone(&self.shutdown_requested);
                    let newly_processed_counter = &newly_processed_count;

                    // Check for shutdown signal before processing each image
                    if shutdown_requested.load(Ordering::SeqCst) {
                        progress.inc(1);
                        return Err(Error::Unknown("Shutdown requested".to_string()));
                    }
                    
                    let result = if !force_rescan {
                        // Get a connection from the pool with retry logic
                        let conn = match pool.get() {
                            Ok(conn) => conn,
                            Err(_e) => {
                                // First failure - log warning and retry after a short delay
                                warn!(
                                    "Database connection timeout while processing {}, retrying...",
                                    img.path.display()
                                );
                                
                                // Variable backoff based on system load
                                let backoff = if max_connections > 16 {
                                    50 // Less backoff for larger connection pools
                                } else {
                                    200 // More backoff for smaller pools
                                };
                                std::thread::sleep(std::time::Duration::from_millis(backoff));
                                
                                // Retry once
                                match pool.get() {
                                    Ok(conn) => conn,
                                    Err(e) => {
                                        // Second failure - log error and skip this file
                                        progress.inc(1);
                                        error!(
                                            "Failed to get database connection while processing {}: {}",
                                            img.path.display(),
                                            e
                                        );
                                        
                                        // Instead of failing, skip this file with a warning and continue
                                        warn!("Skipping database lookup for {}, continuing with direct processing", img.path.display());
                                        
                                        // Longer sleep on second failure to prevent thundering herd reconnection
                                        let extended_backoff = if max_connections > 16 {
                                            100 // Less backoff for larger connection pools
                                        } else {
                                            500 // More backoff for smaller pools
                                        };
                                        std::thread::sleep(std::time::Duration::from_millis(extended_backoff));
                                        
                                        // Process the file directly instead of database lookup
                                        return self.process_image_without_db(img);
                                    }
                                }
                            }
                        };

                        // Try to get from database first
                        match persistence::get_image_by_path_with_conn(&conn, &img.path) {
                            Ok(stored_image) => {
                                trace!(
                                    "Found image {} in database, using cached data",
                                    img.path.display()
                                );

                                // Convert Vec<u8> to [u8; 32] for Blake3 hash
                                let chash: [u8; 32] =
                                    match stored_image.cryptographic_hash.try_into() {
                                        Ok(hash) => hash,
                                        Err(_) => {
                                            progress.inc(1);
                                            logging::log_hash_error(
                                                &img.path,
                                                &std::io::Error::new(
                                                    std::io::ErrorKind::InvalidData,
                                                    "Invalid cryptographic hash length",
                                                ),
                                            );
                                            return Err(Error::Unknown(format!(
                                                "Invalid cryptographic hash length for {}",
                                                img.path.display()
                                            )));
                                        }
                                    };

                                Ok(types::ProcessedImage {
                                    original: Arc::new(img.clone()),
                                    cryptographic_hash: chash.into(),
                                    perceptual_hash: processing::PHash(
                                        stored_image.perceptual_hash,
                                    ),
                                })
                            }
                            Err(e) => {
                                debug!(
                                    "Image {} not found in database: {}, computing hashes",
                                    img.path.display(),
                                    e
                                );
                                // Not in database, process it
                                // Track images that need processing (weren't in DB)
                                newly_processed_counter.fetch_add(1, Ordering::Relaxed);
                                
                                let start = std::time::Instant::now();
                                match self.process_single_image(img, &conn) {
                                    Ok(processed_image) => {
                                        trace!("Processed new image {} in {:?}", img.path.display(), start.elapsed());
                                        Ok(processed_image)
                                    },
                                    Err(e) => {
                                        logging::log_file_error(&img.path, "process_image", &e);
                                        Err(e)
                                    }
                                }
                            }
                        }
                    } else {
                        // Force rescan, always process
                        debug!("Force rescanning image {}", img.path.display());
                        
                        // Get a connection with retry logic
                        let conn = match pool.get() {
                            Ok(conn) => conn,
                            Err(_e) => {
                                // First failure - log warning and retry
                                warn!(
                                    "Database connection timeout for {}, retrying...",
                                    img.path.display()
                                );
                                
                                // Variable backoff based on system load
                                let backoff = if max_connections > 16 {
                                    50 // Less backoff for larger connection pools
                                } else {
                                    200 // More backoff for smaller pools
                                };
                                std::thread::sleep(std::time::Duration::from_millis(backoff));
                                
                                // Retry once
                                match pool.get() {
                                    Ok(conn) => conn,
                                    Err(e2) => {
                                        // Second failure - log warning and process without DB
                                        warn!(
                                            "Failed to get database connection for {}, processing without DB: {}",
                                            img.path.display(),
                                            e2
                                        );
                                        
                                        // Process image directly without database
                                        return self.process_image_without_db(img);
                                    }
                                }
                            }
                        };

                        // For force rescan, always count as newly processed
                        newly_processed_counter.fetch_add(1, Ordering::Relaxed);
                        
                        let start = std::time::Instant::now();
                        match self.process_single_image(img, &conn) {
                            Ok(processed_image) => {
                                trace!("Force-processed image {} in {:?}", img.path.display(), start.elapsed());
                                Ok(processed_image)
                            },
                            Err(e) => {
                                logging::log_file_error(&img.path, "process_image", &e);
                                Err(e)
                            }
                        }
                    };

                    progress.inc(1);
                    result
                })
                .collect();

            // Separate successful and failed images
            for (img, result) in chunk.iter().zip(chunk_results.into_iter()) {
                match result {
                    Ok(processed_image) => processed.push(processed_image),
                    Err(e) => failed_images.push((img.path.clone(), e)),
                }
            }
        }

        // Calculate processing statistics
        let total_elapsed = start_time.elapsed();
        let newly_processed = newly_processed_count.load(Ordering::Relaxed);
        
        let stats_message = if newly_processed > 0 {
            let avg_time_ms = (total_elapsed.as_millis() as f64) / (newly_processed as f64);
            format!(
                "Processing complete - {} succeeded, {} failed. {} new images processed in {:?} (avg {:.2}ms per image)",
                processed.len(),
                failed_images.len(),
                newly_processed,
                total_elapsed,
                avg_time_ms
            )
        } else {
            format!(
                "Processing complete - {} succeeded, {} failed. All images were already in database.",
                processed.len(),
                failed_images.len()
            )
        };
        
        progress.finish_with_message(stats_message.clone());
        info!("{}", stats_message);

        // Log failed images
        if !failed_images.is_empty() {
            warn!("Failed to process {} images:", failed_images.len());
            for (path, error) in &failed_images {
                logging::log_file_error(path, "process_image", error);
            }
        }

        Ok(processed)
    }

    /// Helper function to process a single image
    fn process_single_image(
        &self,
        img: &types::ImageFile,
        conn: &r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
    ) -> Result<types::ProcessedImage> {
        // Process the image
        let chash = processing::compute_cryptographic(&img.path)?;
        // Use GPU acceleration based on config
        let phash = processing::gpu_phash_from_file(&self.config, &img.path)?;

        let processed_image = types::ProcessedImage {
            original: Arc::new(img.clone()),
            cryptographic_hash: chash,
            perceptual_hash: phash,
        };

        // Save to database
        if let Err(e) = persistence::save_processed_image_with_conn(conn, &processed_image) {
            info!(
                "Failed to save image {} to database: {}",
                img.path.display(),
                e
            );
            // Continue even if database save fails
        }

        Ok(processed_image)
    }
    
    /// Process image without using the database - use for fallback when DB access fails
    fn process_image_without_db(&self, img: &types::ImageFile) -> Result<types::ProcessedImage> {
        // Log that we're processing without DB
        debug!("Processing {} without database access", img.path.display());
        
        // Process the image directly - with timing
        let start = std::time::Instant::now();
        
        let chash = processing::compute_cryptographic(&img.path)?;
        // Use GPU acceleration based on config
        let phash = processing::gpu_phash_from_file(&self.config, &img.path)?;

        let processed_image = types::ProcessedImage {
            original: Arc::new(img.clone()),
            cryptographic_hash: chash,
            perceptual_hash: phash,
        };
        
        trace!("Processed image without DB {} in {:?}", img.path.display(), start.elapsed());
        Ok(processed_image)
    }
    
    /// Check database health before starting processing
    fn check_database_health(&self) -> Result<()> {
        if !self.config.use_database {
            return Ok(());
        }
        
        let db_path = self.config
            .database_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("image_deduper.db"));
            
        info!("Verifying database integrity at {}", db_path.display());
        
        // Create direct connection to the database
        if let Ok(conn) = rusqlite::Connection::open(&db_path) {
            // Run integrity check on the database
            match conn.query_row("PRAGMA integrity_check", [], |row| {
                let result: String = row.get(0)?;
                Ok(result)
            }) {
                Ok(result) => {
                    if result == "ok" {
                        info!("Database integrity check passed");
                    } else {
                        warn!("Database integrity check returned: {}", result);
                        // Try to repair the database
                        info!("Attempting to restore database consistency");
                        let _ = conn.execute_batch("VACUUM");
                    }
                },
                Err(e) => {
                    warn!("Failed to run integrity check: {}", e);
                    return Err(Error::Unknown(format!("Database integrity check failed: {}", e)));
                }
            }
            
            // Check for WAL files without a matching database
            let wal_path = db_path.with_extension("db-wal");
            let shm_path = db_path.with_extension("db-shm");
            
            if (wal_path.exists() || shm_path.exists()) && !db_path.exists() {
                warn!("Found WAL/SHM files without a matching database - possible corruption");
                return Err(Error::Unknown("Database appears to be corrupted - WAL files exist without main database".to_string()));
            }
        } else {
            warn!("Could not connect to database to verify integrity");
        }
        
        Ok(())
    }
    
    /// Verify database integrity after processing
    fn verify_database_integrity(&self) -> Result<()> {
        // Skip database operations if shutdown has been requested
        if self.is_shutdown_requested() {
            info!("Skipping database integrity checks due to shutdown request");
            return Ok(());
        }
    
        if !self.config.use_database {
            return Ok(());
        }
        
        let db_path = self.config
            .database_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("image_deduper.db"));
            
        info!("Running final database integrity check");
        
        // Create direct connection to the database
        if let Ok(conn) = rusqlite::Connection::open(&db_path) {
            // Check for shutdown again before any operations
            if self.is_shutdown_requested() {
                return Ok(());
            }
            
            // Run PRAGMA wal_checkpoint to ensure all WAL changes are in the main database
            match conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |_| { Ok(()) }) {
                Ok(_) => info!("WAL checkpoint completed successfully"),
                Err(e) => warn!("Failed to checkpoint WAL: {}", e)
            }
            
            // Check for shutdown again before next operation
            if self.is_shutdown_requested() {
                return Ok(());
            }
            
            // Run integrity check
            match conn.query_row("PRAGMA integrity_check", [], |row| {
                let result: String = row.get(0)?;
                Ok(result)
            }) {
                Ok(result) => {
                    if result == "ok" {
                        info!("Final database integrity check passed");
                    } else {
                        warn!("Final database integrity check returned: {}", result);
                    }
                },
                Err(e) => {
                    warn!("Failed to run final integrity check: {}", e);
                }
            }
            
            // Skip VACUUM if shutdown requested - it's a dangerous operation during shutdown
            if !self.is_shutdown_requested() {
                match conn.execute_batch("VACUUM") {
                    Ok(_) => info!("Database vacuumed successfully"),
                    Err(e) => warn!("Failed to vacuum database: {}", e)
                }
            } else {
                info!("Skipping database VACUUM due to shutdown request");
            }
        } else {
            warn!("Could not connect to database for final integrity check");
        }
        
        Ok(())
    }
}
