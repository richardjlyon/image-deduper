use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use std::io::Read;
use std::hash::Hasher;
use std::collections::hash_map::DefaultHasher;

use blake3::{Hasher as Blake3Hasher, Hash as Blake3Hash};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, warn};
use rayon::prelude::*;
use rusqlite::{Connection, params};
use walkdir::WalkDir;

use crate::processing::perceptual::{PHash, phash_from_file};
use crate::types::ImageFormat;
use crate::{Result as DedupeResult, Error};

// Type alias for Blake3Hash for clarity
type Hash = Blake3Hash;

/// Simple image deduper that focuses on the core task: 
/// Scan directories, compute hashes, and store them in a database.
pub struct SimpleDeduper {
    threads: usize,
    db_path: PathBuf,
    batch_size: usize,
    excluded_directories: Vec<PathBuf>,
}

/// Represents a processed image with its path and hashes
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessedImage {
    pub path: PathBuf,
    pub size: u64,
    pub last_modified: i64,
    pub format: ImageFormat,
    pub cryptographic_hash: Hash,
    pub perceptual_hash: PHash,
}

impl Eq for ProcessedImage {}

impl SimpleDeduper {
    /// Create a new SimpleDeduper with default configuration
    pub fn new() -> Self {
        Self {
            threads: num_cpus::get(),
            db_path: PathBuf::from("image-deduper.db"),
            batch_size: 100,
            excluded_directories: Vec::new(),
        }
    }
    
    /// Configure directories to exclude from scanning
    pub fn with_excluded_directories(mut self, dirs: Vec<PathBuf>) -> Self {
        self.excluded_directories = dirs;
        self
    }

    /// Configure the number of threads to use
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    /// Configure the database path
    pub fn with_database(mut self, db_path: impl Into<PathBuf>) -> Self {
        self.db_path = db_path.into();
        self
    }

    /// Configure the batch size for database operations
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Run the deduplication process
    pub fn run(&self, directories: &[impl AsRef<Path>]) -> DedupeResult<Vec<ProcessedImage>> {
        // 1. Initialize the database
        info!("Initializing database at {}", self.db_path.display());
        self.init_database()?;

        // 2. Discover images
        info!("Discovering images...");
        let image_paths = self.discover_images(directories)?;
        info!("Found {} images", image_paths.len());

        // 3. Process images with batched DB operations
        info!("Processing images with {} threads", self.threads);
        let processed_images = self.process_images(image_paths)?;

        Ok(processed_images)
    }

    /// Initialize the database with schema
    fn init_database(&self) -> DedupeResult<()> {
        // Open the database - convert any SQLite errors to our own error type
        let conn = Connection::open(&self.db_path)
            .map_err(|e| Error::Unknown(format!("Failed to open database: {}", e)))?;
        
        // Set pragmas for performance
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = 10000;
             PRAGMA busy_timeout = 10000;"
        ).map_err(|e| Error::Unknown(format!("Failed to set database pragmas: {}", e)))?;
        
        // Create schema if needed
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS images (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                size INTEGER NOT NULL,
                last_modified INTEGER NOT NULL,
                format TEXT NOT NULL,
                cryptographic_hash BLOB NOT NULL,
                perceptual_hash TEXT NOT NULL
            );
            
            CREATE UNIQUE INDEX IF NOT EXISTS idx_images_path ON images(path);
            CREATE INDEX IF NOT EXISTS idx_images_crypto_hash ON images(cryptographic_hash);
            CREATE INDEX IF NOT EXISTS idx_images_perceptual_hash ON images(perceptual_hash);"
        ).map_err(|e| Error::Unknown(format!("Failed to create database schema: {}", e)))?;
        
        Ok(())
    }
    
    /// Compute cryptographic hash for a file using blake3
    fn compute_hash_file(&self, path: &Path) -> DedupeResult<Hash> {
        // Open the file
        let mut file = std::fs::File::open(path)
            .map_err(|e| Error::Unknown(format!("Failed to open file for hashing: {}", e)))?;
            
        // Create a hasher
        let mut hasher = Blake3Hasher::new();
        
        // Read the file in chunks and update the hasher
        let mut buffer = [0; 8192]; // 8KB buffer
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    hasher.update(&buffer[..n]);
                },
                Err(e) => return Err(Error::Unknown(format!("Failed to read file for hashing: {}", e))),
            }
        }
        
        // Finalize the hash
        Ok(hasher.finalize())
    }

    /// Discover image files in the provided directories
    fn discover_images(&self, directories: &[impl AsRef<Path>]) -> DedupeResult<Vec<PathBuf>> {
        let mut image_paths = Vec::new();
        
        info!("Excluded directories: {:?}", self.excluded_directories);
        
        for dir in directories {
            for entry in WalkDir::new(dir.as_ref())
                          .follow_links(true)
                          .into_iter()
                          .filter_map(|e| e.ok()) {
                
                let path = entry.path();
                
                // Check if path is in an excluded directory
                let should_skip = self.excluded_directories.iter().any(|excluded| {
                    path.starts_with(excluded)
                });
                
                if should_skip {
                    if path.is_dir() {
                        info!("Skipping excluded directory: {}", path.display());
                    }
                    continue;
                }
                
                // Skip directories
                if path.is_dir() {
                    continue;
                }
                
                // Check file extension for common image formats
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_lowercase();
                    if ["jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "heic"].contains(&ext.as_str()) {
                        image_paths.push(path.to_path_buf());
                    }
                }
            }
        }
        
        Ok(image_paths)
    }

    /// Process images in parallel, storing results in the database
    fn process_images(&self, image_paths: Vec<PathBuf>) -> DedupeResult<Vec<ProcessedImage>> {
        // Set up thread pool with configured number of threads
        rayon::ThreadPoolBuilder::new()
            .num_threads(self.threads)
            .build_global()
            .map_err(|e| Error::Unknown(format!("Failed to set up thread pool: {}", e)))?;
        
        // Set up progress bar
        let progress = ProgressBar::new(image_paths.len() as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        let progress = Arc::new(progress);
        
        // Track metrics
        let start_time = Instant::now();
        let new_images_counter = std::sync::atomic::AtomicUsize::new(0);
        let db_hit_counter = std::sync::atomic::AtomicUsize::new(0);
        
        // Process images in chunks
        let chunk_size = self.batch_size;
        let mut processed_images = Vec::with_capacity(image_paths.len());
        
        // Open database connection for checking
        let db_conn = Connection::open(&self.db_path)
            .map_err(|e| Error::Unknown(format!("Failed to open database: {}", e)))?;

        // Process in chunks to avoid memory issues with large collections
        for (chunk_idx, chunk) in image_paths.chunks(chunk_size).enumerate() {
            progress.set_message(format!("Processing chunk {}/{}", 
                chunk_idx + 1, 
                (image_paths.len() + chunk_size - 1) / chunk_size
            ));
            
            // Process chunk in parallel
            let chunk_results: Vec<DedupeResult<ProcessedImage>> = chunk.par_iter()
                .map(|path| {
                    let progress = Arc::clone(&progress);
                    
                    // Create a new database connection for each thread
                    let db_conn_thread = match Connection::open(&self.db_path) {
                        Ok(conn) => conn,
                        Err(e) => {
                            return Err(Error::Unknown(format!("Failed to open database connection: {}", e)));
                        }
                    };
                    
                    // First check if image is already in database
                    let path_str = path.to_string_lossy().to_string();
                    let in_db = db_conn_thread.query_row(
                        "SELECT 1 FROM images WHERE path = ?1", 
                        params![path_str], 
                        |_| Ok(true)
                    ).unwrap_or(false);
                    
                    let result = if in_db {
                        // Image already in database, retrieve it
                        db_hit_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        
                        match db_conn_thread.query_row(
                            "SELECT id, path, size, last_modified, format, cryptographic_hash, perceptual_hash 
                             FROM images WHERE path = ?1",
                            params![path_str],
                            |row| {
                                let path: String = row.get(1)?;
                                let size: u64 = row.get(2)?;
                                let last_modified: i64 = row.get(3)?;
                                let format_str: String = row.get(4)?;
                                let cryptographic_hash: Vec<u8> = row.get(5)?;
                                let perceptual_hash_str: String = row.get(6)?;
                                
                                // Convert format string to enum
                                let format = match format_str.as_str() {
                                    "jpeg" => ImageFormat::Jpeg,
                                    "png" => ImageFormat::Png,
                                    "tiff" => ImageFormat::Tiff,
                                    "heic" => ImageFormat::Heic,
                                    "raw" => ImageFormat::Raw,
                                    other => ImageFormat::Other(other.to_string()),
                                };
                                
                                // Convert hashes - extract the value from the debug string format
                                let phash_value = if perceptual_hash_str.starts_with("Standard(") {
                                    // Extract the number from Standard(12345)
                                    let num_str = perceptual_hash_str
                                        .trim_start_matches("Standard(")
                                        .trim_end_matches(")")
                                        .trim();
                                    num_str.parse::<u64>().unwrap_or(0)
                                } else {
                                    // Default if we can't parse it
                                    0
                                };
                                let phash = PHash::Standard(phash_value);
                                
                                // Convert cryptographic hash - we expect a 32-byte array for Blake3
                                let crypto_hash = if cryptographic_hash.len() == 32 {
                                    // Convert Vec<u8> to [u8; 32]
                                    let mut arr = [0u8; 32];
                                    arr.copy_from_slice(&cryptographic_hash);
                                    Hash::from(arr)
                                } else {
                                    // If hash conversion fails, compute a new one (fallback)
                                    match self.compute_hash_file(&PathBuf::from(&path)) {
                                        Ok(hash) => hash,
                                        Err(e) => {
                                            error!("Error computing hash for {}: {}", path, e);
                                            Hash::from([0u8; 32]) // Empty hash as fallback
                                        }
                                    }
                                };
                                
                                Ok(ProcessedImage {
                                    path: PathBuf::from(path),
                                    size,
                                    last_modified,
                                    format,
                                    cryptographic_hash: crypto_hash,
                                    perceptual_hash: phash,
                                })
                            }
                        ) {
                            Ok(img) => Ok(img),
                            Err(e) => {
                                // Log error and recompute image
                                warn!("Error retrieving image from DB ({}), recomputing: {}", path.display(), e);
                                self.process_single_image(path)
                            }
                        }
                    } else {
                        // Image not in database, process it
                        new_images_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        self.process_single_image(path)
                    };
                    
                    // Update progress bar
                    progress.inc(1);
                    
                    // Return the result
                    result
                })
                .collect();
            
            // Save successful results
            for result in chunk_results {
                match result {
                    Ok(img) => {
                        // Save to database if it's a new image
                        let path_str = img.path.to_string_lossy().to_string();
                        let in_db = db_conn.query_row(
                            "SELECT 1 FROM images WHERE path = ?1", 
                            params![path_str], 
                            |_| Ok(true)
                        ).unwrap_or(false);
                        
                        if !in_db {
                            // Format string
                            let format_str = match &img.format {
                                ImageFormat::Jpeg => "jpeg",
                                ImageFormat::Png => "png",
                                ImageFormat::Tiff => "tiff",
                                ImageFormat::Heic => "heic",
                                ImageFormat::Raw => "raw",
                                ImageFormat::Other(s) => &s,
                            };
                            
                            // Insert into database
                            let _ = db_conn.execute(
                                "INSERT INTO images (path, size, last_modified, format, cryptographic_hash, perceptual_hash)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                params![
                                    path_str,
                                    img.size,
                                    img.last_modified,
                                    format_str,
                                    img.cryptographic_hash.as_bytes(),
                                    format!("{:?}", img.perceptual_hash),
                                ],
                            );
                        }
                        
                        processed_images.push(img);
                    }
                    Err(e) => {
                        error!("Error processing image: {}", e);
                    }
                }
            }
        }
        
        // Print statistics
        let total_time = start_time.elapsed();
        let new_images = new_images_counter.load(std::sync::atomic::Ordering::Relaxed);
        let db_hits = db_hit_counter.load(std::sync::atomic::Ordering::Relaxed);
        
        progress.finish_with_message(format!(
            "Completed in {:?}. Processed {} images ({} new, {} from database)",
            total_time, processed_images.len(), new_images, db_hits
        ));
        
        info!(
            "Processing completed in {:?}. {} total images processed ({} new, {} from database)",
            total_time, processed_images.len(), new_images, db_hits
        );
        
        Ok(processed_images)
    }

    /// Process a single image
    fn process_single_image(&self, path: &Path) -> DedupeResult<ProcessedImage> {
        debug!("Processing image: {}", path.display());
        
        // Get file metadata
        let metadata = std::fs::metadata(path)
            .map_err(|e| Error::Unknown(format!("Failed to read metadata for {}: {}", path.display(), e)))?;
            
        let size = metadata.len();
        let last_modified = metadata.modified()
            .map(|time| time.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
            .unwrap_or(0);
        
        // Determine format from extension
        let format = if let Some(ext) = path.extension() {
            match ext.to_string_lossy().to_lowercase().as_str() {
                "jpg" | "jpeg" => ImageFormat::Jpeg,
                "png" => ImageFormat::Png,
                "tif" | "tiff" => ImageFormat::Tiff,
                "heic" => ImageFormat::Heic,
                _ => ImageFormat::Other(ext.to_string_lossy().to_string()),
            }
        } else {
            ImageFormat::Other("unknown".to_string())
        };
        
        // Compute cryptographic hash
        let cryptographic_hash = self.compute_hash_file(path)?;
        
        // Compute perceptual hash
        let perceptual_hash = match phash_from_file(path) {
            Ok(hash) => hash,
            Err(e) => {
                warn!("Error computing perceptual hash for {}: {}", path.display(), e);
                // Fallback to a hash based on path and file size if image can't be processed
                let mut hasher = DefaultHasher::new();
                std::hash::Hash::hash(&path.to_string_lossy(), &mut hasher);
                std::hash::Hash::hash(&size, &mut hasher);
                PHash::Standard(hasher.finish())
            }
        };
        
        Ok(ProcessedImage {
            path: path.to_path_buf(),
            size,
            last_modified,
            format,
            cryptographic_hash,
            perceptual_hash,
        })
    }

    /// Find duplicate images based on cryptographic and perceptual hashes
    pub fn find_duplicates<'a>(&self, images: &'a [ProcessedImage]) -> Vec<Vec<&'a ProcessedImage>> {
        // Group by cryptographic hash (exact duplicates)
        let mut hash_groups: std::collections::HashMap<[u8; 32], Vec<&'a ProcessedImage>> = std::collections::HashMap::new();
        
        for img in images {
            hash_groups.entry(*img.cryptographic_hash.as_bytes())
                .or_default()
                .push(img);
        }
        
        // Collect groups with more than one image
        let mut duplicate_groups: Vec<Vec<&'a ProcessedImage>> = hash_groups
            .into_iter()
            .filter(|(_, group)| group.len() > 1)
            .map(|(_, group)| group)
            .collect();
        
        // Further group by perceptual similarity
        let perceptual_threshold = 10; // Max hamming distance to consider similar
        
        // First, find images not yet in any group
        let mut ungrouped: Vec<&'a ProcessedImage> = images
            .iter()
            .filter(|img| !duplicate_groups.iter().any(|group| group.contains(img)))
            .collect();
        
        // Then find perceptually similar images
        let mut perceptual_groups: Vec<Vec<&'a ProcessedImage>> = Vec::new();
        
        while !ungrouped.is_empty() {
            let img = ungrouped.remove(0);
            let mut similar = vec![img];
            
            // Find all similar images
            ungrouped.retain(|other| {
                let is_similar = img.perceptual_hash.distance(&other.perceptual_hash) <= perceptual_threshold;
                if is_similar {
                    similar.push(*other);
                }
                !is_similar
            });
            
            if similar.len() > 1 {
                perceptual_groups.push(similar);
            }
        }
        
        // Combine all duplicate groups
        duplicate_groups.extend(perceptual_groups);
        duplicate_groups
    }
}

impl Default for SimpleDeduper {
    fn default() -> Self {
        Self::new()
    }
}