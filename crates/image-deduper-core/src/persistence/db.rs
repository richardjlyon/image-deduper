use log::info;
use rocksdb::Options as rdbOptions;
use rocksdb::DB;
use sysinfo::{Pid, System};

use crate::{error::Result, get_default_db_path, Config};

/// Get the number of open file descriptors for the current process
fn get_open_file_count() -> Result<usize> {
    let mut sys = System::new_all();
    sys.refresh_all(); // Refresh all information
    if let Some(_process) = sys.process(Pid::from(std::process::id() as usize)) {
        #[cfg(target_os = "linux")]
        {
            Ok(_process.number_of_open_files().unwrap_or(0))
        }
        #[cfg(target_os = "macos")]
        {
            // On macOS, we can't easily get the file descriptor count
            // Return 0 to indicate we can't track this
            Ok(0)
        }
    } else {
        Ok(0)
    }
}

/// Get current memory usage in bytes
fn get_memory_usage() -> Result<u64> {
    let mut sys = System::new_all();
    sys.refresh_all(); // Refresh all information
    if let Some(process) = sys.process(Pid::from(std::process::id() as usize)) {
        // Get both resident and virtual memory
        let rss = process.memory() * 1024; // RSS in bytes
        tracy_client::plot!("RSS Memory (MB)", (rss / 1024 / 1024) as f64);

        #[cfg(target_os = "macos")]
        {
            // On macOS, also track virtual memory
            let vm = process.virtual_memory();
            tracy_client::plot!("Virtual Memory (MB)", (vm / 1024 / 1024) as f64);
        }

        Ok(rss)
    } else {
        Ok(0)
    }
}

/// Initialize and open the RocksDB database
pub fn rocksdb(config: &Config) -> Result<DB> {
    let _span = tracy_client::span!("init_rocksdb");

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

    // Track initial state
    let initial_files = get_open_file_count()?;
    let initial_memory = get_memory_usage()?;

    // Plot memory usage in Tracy with a descriptive name
    tracy_client::plot!("Memory Usage (MB)", (initial_memory / 1024 / 1024) as f64);
    tracy_client::plot!("Open Files", initial_files as f64);

    // Configure RocksDB options for better concurrent write performance
    let mut options = rdbOptions::default();
    options.create_if_missing(true);
    options.increase_parallelism(num_cpus::get() as i32);
    options.set_max_background_jobs(4);
    options.set_write_buffer_size(64 * 1024 * 1024);
    options.set_max_write_buffer_number(4);

    // Open the database
    info!("Opening RocksDB database at: {}", db_path.display());
    let db = DB::open(&options, &db_path)?;

    // Track final state and plot changes
    let final_files = get_open_file_count()?;
    let final_memory = get_memory_usage()?;

    // Plot final state
    tracy_client::plot!("Memory Usage (MB)", (final_memory / 1024 / 1024) as f64);
    tracy_client::plot!("Open Files", final_files as f64);

    // Log using standard logging
    info!(
        "RocksDB stats: files: {} -> {}, memory: {:.2} -> {:.2} MB",
        initial_files,
        final_files,
        initial_memory as f64 / 1024.0 / 1024.0,
        final_memory as f64 / 1024.0 / 1024.0
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
    let _span = tracy_client::span!("insert_hashes");
    let path_str = path.to_string_lossy().into_owned();

    // Track memory before operation
    let initial_memory = get_memory_usage()?;
    let initial_files = get_open_file_count()?;

    // Plot current state
    tracy_client::plot!("Memory Usage (MB)", (initial_memory / 1024 / 1024) as f64);
    tracy_client::plot!("Open Files", initial_files as f64);

    // Store path->hash mappings with individualized spans
    {
        let _span = tracy_client::span!("store_path_hash_mappings");
        let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();
        let path_p_key = [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();
        db.put(path_c_key, c_hash)?;
        db.put(path_p_key, p_hash)?;
    }

    // Store hash entries for efficient iteration
    {
        let _span = tracy_client::span!("store_hash_entries");
        let c_key = [b"c:".to_vec(), c_hash.clone()].concat();
        let p_key = [b"p:".to_vec(), p_hash.clone()].concat();
        db.put(c_key, path_str.as_bytes())?;
        db.put(p_key, path_str.as_bytes())?;
    }

    // Track final state
    let final_memory = get_memory_usage()?;
    let final_files = get_open_file_count()?;

    // Plot final state
    tracy_client::plot!("Memory Usage (MB)", (final_memory / 1024 / 1024) as f64);
    tracy_client::plot!("Open Files", final_files as f64);

    // Calculate deltas using saturating arithmetic to prevent overflow
    let files_delta = if final_files >= initial_files {
        final_files - initial_files
    } else {
        0
    };
    let memory_delta = if final_memory >= initial_memory {
        final_memory - initial_memory
    } else {
        0
    };

    info!(
        "Hash insertion complete (files: +{}, memory: +{:.2} MB)",
        files_delta,
        memory_delta as f64 / 1024.0 / 1024.0
    );

    Ok(())
}

/// Check if hashes exist for a given path
pub fn check_hashes(db: &DB, path: &std::path::PathBuf) -> Result<bool> {
    let _span = tracy_client::span!("check_hashes");
    let path_str = path.to_string_lossy().into_owned();
    let path_c_key = [b"pc:".to_vec(), path_str.as_bytes().to_vec()].concat();
    let path_p_key = [b"pp:".to_vec(), path_str.as_bytes().to_vec()].concat();

    // Track memory before operation
    let initial_memory = get_memory_usage()?;
    let initial_files = get_open_file_count()?;

    // Check if both hashes exist for this path
    let c_exists = {
        let _span = tracy_client::span!("get_cryptographic_hash");
        db.get(&path_c_key)?.is_some()
    };

    let p_exists = {
        let _span = tracy_client::span!("get_perceptual_hash");
        db.get(&path_p_key)?.is_some()
    };

    // Track final state
    let final_memory = get_memory_usage()?;
    let final_files = get_open_file_count()?;
    let _span = tracy_client::span!("check_complete");

    // Calculate deltas using saturating arithmetic to prevent overflow
    let files_delta = if final_files >= initial_files {
        final_files - initial_files
    } else {
        0
    };
    let memory_delta = if final_memory >= initial_memory {
        final_memory - initial_memory
    } else {
        0
    };

    info!(
        "Hash check complete (files: +{}, memory: +{} bytes)",
        files_delta, memory_delta
    );

    Ok(c_exists && p_exists)
}

/// Diagnose the database for inconsistencies
pub fn diagnose_database(db: &DB) -> Result<()> {
    let _span = tracy_client::span!("diagnose_database");
    let mut pc_keys = 0;
    let mut pp_keys = 0;
    let mut other_keys = 0;
    let mut inconsistent_paths = Vec::new();
    let mut path_to_hashes: std::collections::HashMap<String, (bool, bool)> =
        std::collections::HashMap::new();

    // Track initial state
    let initial_memory = get_memory_usage()?;
    let initial_files = get_open_file_count()?;

    // Count all types of keys and collect paths
    {
        let _span = tracy_client::span!("scan_database");
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
    }

    // Find inconsistent paths
    {
        let _span = tracy_client::span!("find_inconsistencies");
        for (path, (has_c, has_p)) in &path_to_hashes {
            if *has_c != *has_p {
                inconsistent_paths.push((path.clone(), *has_c, *has_p));
            }
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
        let _span = tracy_client::span!("cleanup_inconsistencies");
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

    // Track final state
    let final_memory = get_memory_usage()?;
    let final_files = get_open_file_count()?;
    let _span = tracy_client::span!("diagnose_complete");

    // Calculate deltas using saturating arithmetic to prevent overflow
    let files_delta = if final_files >= initial_files {
        final_files - initial_files
    } else {
        0
    };
    let memory_delta = if final_memory >= initial_memory {
        final_memory - initial_memory
    } else {
        0
    };

    info!(
        "Database diagnosis complete (files: +{}, memory: +{} bytes)",
        files_delta, memory_delta
    );

    Ok(())
}

/// Get statistics about the database contents
pub fn get_db_stats(db: &DB) -> Result<(usize, usize)> {
    let _span = tracy_client::span!("get_db_stats");

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
