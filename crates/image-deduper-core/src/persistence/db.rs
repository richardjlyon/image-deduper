use log::info;
use rocksdb::Options as rdbOptions;
use rocksdb::DB;
use std::path::PathBuf;

use crate::{
    error::{Error, Result},
    Config,
};

/// Initialize and open the RocksDB database
pub fn rocksdb(config: &Config) -> Result<DB> {
    let project_dir = std::env::current_dir()?;

    let db_path = project_dir.join(
        config
            .database_path
            .clone()
            .unwrap_or(PathBuf::from("image_hash_db")),
    );

    // Configure RocksDB options for better concurrent write performance
    let mut options = rdbOptions::default();
    options.create_if_missing(true);
    options.increase_parallelism(num_cpus::get() as i32); // Use all available CPU cores
    options.set_max_background_jobs(4); // Adjust based on your hardware
    options.set_write_buffer_size(64 * 1024 * 1024); // 64MB write buffer
    options.set_max_write_buffer_number(4); // Allow multiple write buffers

    // Open the database
    let db = DB::open(&options, &db_path)?;

    info!(
        "RocksDB database initialized successfully at {}",
        db_path.display()
    );

    Ok(db)
}

/// Insert cryptographic and perceptual hashes into the RocksDB database
pub fn insert_hashes(
    db: &DB,
    path: &std::path::PathBuf,
    c_hash: &Vec<u8>,
    p_hash: &Vec<u8>,
) -> Result<()> {
    let path_str = path.to_string_lossy().into_owned();

    // Store path->hash mappings
    let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();
    let path_p_key = [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();
    db.put(path_c_key, c_hash)?;
    db.put(path_p_key, p_hash)?;

    // Store hash entries for efficient iteration
    let c_key = [b"c:".to_vec(), c_hash.clone()].concat();
    let p_key = [b"p:".to_vec(), p_hash.clone()].concat();
    db.put(c_key, path_str.as_bytes())?;
    db.put(p_key, path_str.as_bytes())?;

    Ok(())
}

/// Check if hashes exist for a given path
pub fn check_hashes(db: &DB, path: &std::path::PathBuf) -> Result<bool> {
    let path_str = path.to_string_lossy().into_owned();
    let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();
    let path_p_key = [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();

    // Check if both hashes exist for this path
    let c_exists = db.get(&path_c_key)?.is_some();
    let p_exists = db.get(&path_p_key)?.is_some();

    Ok(c_exists && p_exists)
}

/// Diagnose the database for inconsistencies
pub fn diagnose_database(db: &DB) -> Result<()> {
    let mut pc_keys = 0;
    let mut pp_keys = 0;
    let mut other_keys = 0;
    let mut inconsistent_paths = Vec::new();
    let mut path_to_hashes: std::collections::HashMap<String, (bool, bool)> =
        std::collections::HashMap::new();

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
                } else {
                    other_keys += 1;
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

    log::info!("Database diagnosis:");
    log::info!("- Path->Cryptographic keys (pc:): {}", pc_keys);
    log::info!("- Path->Perceptual keys (pp:): {}", pp_keys);
    log::info!("- Other keys: {}", other_keys);
    log::info!("- Total unique paths: {}", path_to_hashes.len());
    log::info!("- Inconsistent paths: {}", inconsistent_paths.len());

    // Print details about inconsistent paths
    if !inconsistent_paths.is_empty() {
        log::info!("Inconsistent paths details:");
        for (path, has_c, has_p) in
            &inconsistent_paths[0..std::cmp::min(5, inconsistent_paths.len())]
        {
            log::info!("  - Path: {}, Has C: {}, Has P: {}", path, has_c, has_p);
        }
        if inconsistent_paths.len() > 5 {
            log::info!("  - ... and {} more", inconsistent_paths.len() - 5);
        }
    }

    // Cleanup inconsistent records if needed
    if !inconsistent_paths.is_empty() {
        log::info!(
            "Cleaning up {} inconsistent records",
            inconsistent_paths.len()
        );
        let mut batch = rocksdb::WriteBatch::default();

        for (path, has_c, has_p) in inconsistent_paths {
            // Remove all related entries
            if has_c {
                batch.delete(format!("pc:{}", path).as_bytes());
                // Also remove the corresponding reverse mapping if it exists
                if let Ok(Some(hash_bytes)) = db.get(format!("pc:{}", path).as_bytes()) {
                    let c_key = [b"c:".to_vec(), hash_bytes.to_vec()].concat();
                    batch.delete(&c_key);
                }
            }
            if has_p {
                batch.delete(format!("pp:{}", path).as_bytes());
                // Also remove the corresponding reverse mapping if it exists
                if let Ok(Some(hash_bytes)) = db.get(format!("pp:{}", path).as_bytes()) {
                    let p_key = [b"p:".to_vec(), hash_bytes.to_vec()].concat();
                    batch.delete(&p_key);
                }
            }
        }

        db.write(batch)?;
        log::info!("Cleanup complete");
    }

    Ok(())
}
