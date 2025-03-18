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
