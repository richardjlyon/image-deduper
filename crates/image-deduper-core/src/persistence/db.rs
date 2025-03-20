use log::{info, warn};
use rocksdb::{Options as RdbOptions, WriteBatch, DB};

use crate::processing::types::ImageHashResult;
use crate::processing::PHash;
use crate::{error::Result, get_default_db_path, Config};
use std::path::PathBuf;

/// Initialize and open the RocksDB database with optimized settings
pub fn rocksdb(config: &Config) -> Result<DB> {
    // Get the absolute path for the database
    let db_path = if let Some(path) = &config.database_path {
        if path.is_absolute() {
            path.clone()
        } else {
            // If relative path is provided, make it absolute relative to current directory
            std::env::current_dir()?.join(path)
        }
    } else {
        // Use the default path in user's home directory
        get_default_db_path()
    };

    // Create parent directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Configure RocksDB options for better concurrent write performance
    let mut options = RdbOptions::default();
    options.create_if_missing(true);
    options.increase_parallelism(num_cpus::get() as i32);
    options.set_max_background_jobs(4);
    options.set_write_buffer_size(64 * 1024 * 1024);
    options.set_max_write_buffer_number(4);

    // Use level-based compaction for better performance
    options.set_level_compaction_dynamic_level_bytes(true);

    // Open the database
    info!("Opening RocksDB database at: {}", db_path.display());
    let db = DB::open(&options, &db_path)?;

    Ok(db)
}

/// Convert a Blake3 hash to a byte vector
fn blake3_to_vec(hash: blake3::Hash) -> Vec<u8> {
    hash.as_bytes().to_vec()
}

/// Convert a PHash to a byte vector
fn phash_to_vec(phash: &PHash) -> Vec<u8> {
    match phash {
        PHash::Standard(hash_value) => {
            // Convert u64 to 8 bytes
            hash_value.to_be_bytes().to_vec()
        }
        PHash::Enhanced(hash_array) => {
            // Convert [u64; 16] to 128 bytes
            let mut bytes = Vec::with_capacity(128);
            for &value in hash_array.iter() {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
            bytes
        }
    }
}

/// Insert cryptographic and perceptual hashes into the RocksDB database
pub fn insert_hashes(db: &DB, path: &PathBuf, c_hash: &blake3::Hash, p_hash: &PHash) -> Result<()> {
    let path_str = path.to_string_lossy().into_owned();

    // Convert hashes to byte vectors
    let c_hash_bytes = blake3_to_vec(*c_hash);
    let p_hash_bytes = phash_to_vec(p_hash);

    // Store path->hash mappings
    let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();
    let path_p_key = [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();

    db.put(path_c_key, &c_hash_bytes)?;
    db.put(path_p_key, &p_hash_bytes)?;

    Ok(())
}

/// Insert multiple hash results efficiently in a single batch operation
pub fn batch_insert_hashes(db: &DB, results: &[ImageHashResult]) -> Result<()> {
    if results.is_empty() {
        return Ok(());
    }

    // Create a batch operation
    let mut batch = WriteBatch::default();

    // Add all items to batch
    for result in results {
        let path_str = result.path.to_string_lossy().into_owned();
        let c_hash_bytes = blake3_to_vec(result.cryptographic);
        let p_hash_bytes = phash_to_vec(&result.perceptual);

        // Create keys for path->hash mappings
        let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();
        let path_p_key = [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();

        // Add to batch
        batch.put(&path_c_key, &c_hash_bytes);
        batch.put(&path_p_key, &p_hash_bytes);
    }

    // Write batch to database
    db.write(batch)?;

    info!("Inserted {} hash records into database", results.len());
    Ok(())
}

/// Check if hashes exist for a given path
pub fn check_hashes(db: &DB, path: &PathBuf) -> Result<bool> {
    let path_str = path.to_string_lossy().into_owned();

    // Check only the cryptographic hash for faster lookups
    // We know both hashes are inserted together
    let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();

    // One database read is faster than two
    let exists = db.get(&path_c_key)?.is_some();

    Ok(exists)
}

/// Filter a list of paths to only include those not already in the database
pub fn filter_new_images(db: &DB, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    use rayon::prelude::*;
    use std::time::Instant;

    info!("Starting to filter {} paths for new images...", paths.len());
    let start_time = Instant::now();

    // Use smaller chunks for better feedback
    const CHUNK_SIZE: usize = 1000;
    let mut new_paths = Vec::new();

    // Process in chunks to show progress
    for (_chunk_idx, chunk) in paths.chunks(CHUNK_SIZE).enumerate() {
        // println!(
        //     "Checking chunk {}/{} ({} paths)...",
        //     chunk_idx + 1,
        //     (paths.len() + CHUNK_SIZE - 1) / CHUNK_SIZE,
        //     chunk.len()
        // );

        // let chunk_start = Instant::now();
        let chunk_new_paths: Vec<PathBuf> = chunk
            .par_iter()
            .filter_map(|path| match check_hashes(db, path) {
                Ok(exists) if !exists => Some(path.clone()),
                _ => None,
            })
            .collect();

        // let chunk_duration = chunk_start.elapsed();
        // println!(
        //     "Found {} new images in chunk {} (took {:.2}s)",
        //     chunk_new_paths.len(),
        //     chunk_idx + 1,
        //     chunk_duration.as_secs_f64()
        // );

        new_paths.extend(chunk_new_paths);
    }

    let duration = start_time.elapsed();
    info!("Filtering completed in {:.2}s", duration.as_secs_f64());
    info!(
        "Found {} new images out of {} total",
        new_paths.len(),
        paths.len()
    );
    Ok(new_paths)
}

/// Get statistics about the database contents
pub fn get_db_stats(db: &DB) -> Result<(usize, usize)> {
    let mut pc_count = 0;
    let mut pp_count = 0;

    // Count entries with pc: and pp: prefixes
    let iter = db.iterator(rocksdb::IteratorMode::Start);
    for result in iter {
        if let Ok((key, _)) = result {
            if let Ok(key_str) = std::str::from_utf8(&key) {
                if key_str.starts_with("pc:") {
                    pc_count += 1;
                } else if key_str.starts_with("pp:") {
                    pp_count += 1;
                }
            }
        }
    }

    Ok((pc_count, pp_count))
}

/// Perform database maintenance operations
pub fn maintain_database(db: &DB) -> Result<()> {
    info!("Starting database maintenance...");

    // Flush all write buffers to disk
    db.flush()?;

    // Trigger compaction on the entire database
    db.compact_range::<&[u8], &[u8]>(None, None);

    info!("Database maintenance complete");
    Ok(())
}

/// Diagnose the database for inconsistencies
pub fn diagnose_database(db: &DB) -> Result<()> {
    info!("Scanning database for inconsistencies...");

    let mut pc_keys = 0;
    let mut pp_keys = 0;
    let mut inconsistent_paths = Vec::new();
    let mut path_to_hashes = std::collections::HashMap::new();

    // Count all types of keys and collect paths
    let iter = db.iterator(rocksdb::IteratorMode::Start);
    for result in iter {
        if let Ok((key, _value)) = result {
            if let Ok(key_str) = std::str::from_utf8(&key) {
                if key_str.starts_with("pc:") {
                    pc_keys += 1;
                    let path = key_str[3..].to_string();
                    path_to_hashes
                        .entry(path.clone())
                        .and_modify(|(c, _)| *c = true)
                        .or_insert((true, false));
                } else if key_str.starts_with("pp:") {
                    pp_keys += 1;
                    let path = key_str[3..].to_string();
                    path_to_hashes
                        .entry(path.clone())
                        .and_modify(|(_, p)| *p = true)
                        .or_insert((false, true));
                }
            }
        }
    }

    // Find inconsistent paths
    for (path, (has_c, has_p)) in &path_to_hashes {
        if *has_c != *has_p {
            inconsistent_paths.push((path.clone(), *has_c, *has_p));
        }
    }

    info!("Database diagnosis:");
    info!("- Path->Cryptographic keys (pc:): {}", pc_keys);
    info!("- Path->Perceptual keys (pp:): {}", pp_keys);
    info!("- Total unique paths: {}", path_to_hashes.len());
    info!("- Inconsistent paths: {}", inconsistent_paths.len());

    // Print details about inconsistent paths
    if !inconsistent_paths.is_empty() {
        info!("Inconsistent paths details:");
        for (path, has_c, has_p) in
            &inconsistent_paths[0..std::cmp::min(5, inconsistent_paths.len())]
        {
            info!("  - Path: {}, Has C: {}, Has P: {}", path, has_c, has_p);
        }
        if inconsistent_paths.len() > 5 {
            info!("  - ... and {} more", inconsistent_paths.len() - 5);
        }

        // Warn user about inconsistencies but don't fix automatically
        warn!(
            "Found {} inconsistent records in database",
            inconsistent_paths.len()
        );
    }

    Ok(())
}
