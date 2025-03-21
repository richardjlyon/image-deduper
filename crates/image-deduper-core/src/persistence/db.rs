use std::path::PathBuf;

use blake3::Hash as Blake3Hash;
use directories::ProjectDirs;
use log::{info, warn};
use rocksdb::{IteratorMode, Options as RdbOptions, WriteBatch, DB};

use crate::error::Result;
use crate::processing::perceptual_hash::PHash;
use crate::processing::types::ImageHashResult;
use crate::Config;

#[derive(Clone, Debug)]
pub struct DBImageData {
    pub path: PathBuf,
    pub crypto_hash: Option<Blake3Hash>,
    pub perceptual_hash: Option<PHash>,
}

pub struct ImageHashDB {
    db: DB,
}

impl ImageHashDB {
    /// Create a new ImageGashDB
    pub fn new(config: &Config) -> Self {
        // Configure RocksDB options for better concurrent write performance
        let mut options = RdbOptions::default();
        options.create_if_missing(true);
        options.increase_parallelism(num_cpus::get() as i32);
        options.set_max_background_jobs(4);
        options.set_write_buffer_size(64 * 1024 * 1024);
        options.set_max_write_buffer_number(4);
        // Use level-based compaction for better performance
        options.set_level_compaction_dynamic_level_bytes(true);

        // Create the db in tghe system's  config dir
        let mut store_path = ProjectDirs::from("com", "lyonef", "image_deduper")
            .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
            .expect("Failed to get config directory");

        store_path.push(PathBuf::from(
            config
                .database_name
                .as_ref()
                .unwrap_or(&String::from("image_hash_db")),
        ));

        // Delete the data base if config.reinitialise_database is true
        if config.reinitialise_database {
            std::fs::remove_dir_all(&store_path).unwrap_or_default();
            info!("Deleted existing database at: {}", store_path.display());
        }

        info!("Opening RocksDB database at: {}", store_path.display());

        return Self {
            db: DB::open(&options, &store_path).expect("failed to open store"),
        };
    }

    /// Insert multiple hash results efficiently in a single batch operation
    pub fn batch_insert_hashes(&self, results: &[ImageHashResult]) -> Result<()> {
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
        self.db.write(batch)?;

        info!("Inserted {} hash records into database", results.len());
        Ok(())
    }

    pub fn get_all_hashes(&self) -> Result<Vec<DBImageData>> {
        let mut images = Vec::new();

        // Iterate over all keys in the database
        let iter = self.db.iterator(IteratorMode::Start);
        for result in iter {
            match result {
                Ok((key, value)) => {
                    let key_str = String::from_utf8(key.to_vec()).expect("Invalid UTF-8 sequence");
                    if key_str.starts_with("pc:") {
                        let path_str = &key_str[3..];
                        let path = PathBuf::from(path_str);

                        // Retrieve the perceptual hash
                        let path_p_key = [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();
                        let p_hash_bytes = self.db.get(path_p_key)?;

                        // Convert byte vectors back to hashes
                        let c_hash = vec_to_blake3(&value);
                        let p_hash = p_hash_bytes.map(|bytes| vec_to_phash(&bytes));

                        images.push(DBImageData {
                            path,
                            crypto_hash: Some(c_hash),
                            perceptual_hash: p_hash,
                        });
                    }
                }
                Err(e) => {
                    warn!("Error iterating over database: {}", e);
                }
            }
        }

        Ok(images)
    }

    /// Find images that are not already in the database
    pub fn find_new_images(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        use rayon::prelude::*;
        use std::time::Instant;

        info!("Starting to filter {} paths for new images...", paths.len());
        let start_time = Instant::now();
        let mut new_paths = Vec::new();

        // Process in chunks to show progress
        // Use smaller chunks for better feedback
        const CHUNK_SIZE: usize = 1000;
        for (_chunk_idx, chunk) in paths.chunks(CHUNK_SIZE).enumerate() {
            // let chunk_start = Instant::now();
            let chunk_new_paths: Vec<PathBuf> = chunk
                .par_iter()
                .filter_map(|path| match self.check_hashes(path) {
                    Ok(exists) if !exists => Some(path.clone()),
                    _ => None,
                })
                .collect();

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

    /// Check if hashes exist for a given path
    fn check_hashes(&self, path: &PathBuf) -> Result<bool> {
        let path_str = path.to_string_lossy().into_owned();

        // Check only the cryptographic hash for faster lookups
        // We know both hashes are inserted together
        let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();

        // One database read is faster than two
        let exists = self.db.get(&path_c_key)?.is_some();

        Ok(exists)
    }

    /// Flush memtable to disk
    pub fn flush(&self) -> Result<()> {
        Ok(self.db.flush()?)
    }

    /// Compact range
    pub fn compact_range(&self) {
        self.db.compact_range::<&[u8], &[u8]>(None, None)
    }

    /// Get statistics about the database contents
    pub fn get_db_stats(&self) -> Result<(usize, usize)> {
        let mut pc_count = 0;
        let mut pp_count = 0;

        // Count entries with pc: and pp: prefixes
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
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

    /// Diagnose the database for inconsistencies
    pub fn diagnose_database(&self) -> Result<()> {
        info!("Scanning database for inconsistencies...");

        let mut pc_keys = 0;
        let mut pp_keys = 0;
        let mut inconsistent_paths = Vec::new();
        let mut path_to_hashes = std::collections::HashMap::new();

        // Count all types of keys and collect paths
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
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

// Helper function to convert byte vector to blake3::Hash
fn vec_to_blake3(bytes: &[u8]) -> Blake3Hash {
    let mut hash_bytes = [0u8; 32];
    hash_bytes.copy_from_slice(bytes);
    Blake3Hash::from(hash_bytes)
}

// Helper function to convert byte vector to PHash
fn vec_to_phash(bytes: &[u8]) -> PHash {
    match bytes.len() {
        8 => {
            // Deserialize as Standard PHash (64-bit)
            let mut array = [0u8; 8];
            array.copy_from_slice(bytes);
            let value = u64::from_be_bytes(array);
            PHash::Standard(value)
        }
        128 => {
            // Deserialize as Enhanced PHash (1024-bit)
            let mut array = [0u64; 16];
            for (i, chunk) in bytes.chunks_exact(8).enumerate() {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(chunk);
                array[i] = u64::from_be_bytes(buf);
            }
            PHash::Enhanced(array)
        }
        _ => panic!("Invalid byte length for PHash"),
    }
}
