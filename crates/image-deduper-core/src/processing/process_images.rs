use blake3::Hash as Blake3Hash;
use log::info;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::collections::HashSet;
use std::sync::Mutex;
use once_cell::sync::Lazy;

use super::compute_cryptographic;
use super::perceptual::{phash_from_file, process_tiff_directly, PHash};

// Global skip list for problematic files that consistently cause timeouts
// This list persists across multiple function calls to prevent repeated timeouts
static PROBLEMATIC_FILES: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

#[derive(Debug, Clone)]
pub struct ImageHashResult {
    /// Path to the image file
    pub path: PathBuf,
    /// Blake3 cryptographic hash of the file contents
    pub cryptographic: Blake3Hash,
    /// Perceptual hash of the image
    pub perceptual: PHash,
}

/// Process a batch of images and compute their hashes with error handling
/// Returns a tuple of (successful results, error count)
pub fn process_image_batch(
    paths: &[PathBuf],
    progress_counter: Option<&Arc<AtomicUsize>>,
) -> (Vec<ImageHashResult>, usize) {
    use rayon::prelude::*; // Using rayon for parallel processing
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Instant;
    use sysinfo::System;

    info!("Processing batch of {} images...", paths.len());

    // Don't print anything to console for this function

    // Track memory usage for debugging
    let mut system = System::new_all();
    system.refresh_all();
    let start_mem = system.used_memory() / 1024 / 1024; // Convert to MB
    info!("Memory usage at start: {}MB", start_mem);

    let batch_start = Instant::now();

    // Use atomic counter for thread safety
    let error_counter = Arc::new(AtomicUsize::new(0));
    let processed_counter = Arc::new(AtomicUsize::new(0));

    // Set a thread limit to prevent resource exhaustion
    // This creates a thread pool with a reasonable number of threads
    let thread_limit = std::cmp::min(num_cpus::get(), 8);
    info!("Using {} threads for image processing", thread_limit);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(thread_limit)
        .build()
        .unwrap();

    // Process images in parallel using a controlled thread pool
    let results: Vec<_> = pool.install(|| {
        paths
            .par_iter()
            .map(|path| {
                let start = Instant::now();
                let path_display = path.display().to_string();

                // Process each hash computation with a timeout
                // Only log file processing at debug level
                info!("Starting to process: '{}'", path_display);

                // First check if file exists and is accessible
                if !path.exists() {
                    // Log the error using your file operation logging
                    crate::logging::log_file_error(path, "check_exists", &std::io::Error::new(
                        std::io::ErrorKind::NotFound, "File does not exist"
                    ));
                    println!("File does not exist: '{}'", path_display);

                    // Increment counters
                    error_counter.fetch_add(1, Ordering::Relaxed);
                    if let Some(counter) = progress_counter {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                    return None;
                }

                // Check if file is zero-sized or too large
                match std::fs::metadata(path) {
                    Ok(metadata) => {
                        let file_size = metadata.len();
                        if file_size == 0 {
                            // Log zero-sized file
                            crate::logging::log_file_error(path, "check_size", &std::io::Error::new(
                                std::io::ErrorKind::InvalidData, "Zero-sized file"
                            ));
                            println!("Skipping zero-sized file: '{}'", path_display);

                            // Increment counters
                            error_counter.fetch_add(1, Ordering::Relaxed);
                            if let Some(counter) = progress_counter {
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            return None;
                        }

                        // We used to skip files larger than 200MB, but now we use image resizing 
                        // for perceptual hash calculation, so we can handle large files efficiently
                        if file_size > 200_000_000 { // 200MB limit - log but don't skip
                            // Just log large file for monitoring
                            log::info!(
                                "Processing large file ({}MB) with resize optimization: '{}'",
                                file_size / 1_000_000, path_display
                            );
                        }
                        
                        // Check for TIFF files for specialized handling 
                        if let Some(ext) = path.extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if ext_str == "tif" || ext_str == "tiff" {
                                // Log TIFF processing
                                if file_size > 100_000_000 {
                                    // Very large TIFF gets more aggressive handling
                                    log::info!(
                                        "Processing large TIFF file ({}MB) with aggressive optimization: '{}'",
                                        file_size / 1_000_000, path_display
                                    );
                                } else {
                                    log::info!(
                                        "Processing TIFF file ({}MB) with specialized handler: '{}'",
                                        file_size / 1_000_000, path_display
                                    );
                                }
                            }
                        }
                    },
                    Err(e) => {
                        // Log metadata error
                        crate::logging::log_file_error(path, "metadata", &e);
                        println!("Cannot read file metadata for '{}': {}", path_display, e);

                        // Increment counters
                        error_counter.fetch_add(1, Ordering::Relaxed);
                        if let Some(counter) = progress_counter {
                            counter.fetch_add(1, Ordering::Relaxed);
                        }
                        return None;
                    }
                }

                // Process cryptographic hash with timeout
                info!("Computing crypto hash for: '{}'", path_display);
                let crypto_result = std::panic::catch_unwind(|| {
                    use std::thread;
                    use std::sync::atomic::{AtomicBool, Ordering};
                    use std::sync::Arc;

                    // Create a cancellation token
                    let cancel_token = Arc::new(AtomicBool::new(false));
                    let cancel_token_clone = cancel_token.clone();

                    // Get file extension for timeout configuration before moving path
                    let file_ext = path.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();

                    // Spawn a thread to compute the hash with a timeout
                    let path_clone = path.clone();
                    let (tx, rx) = std::sync::mpsc::channel();

                    // Compute in a separate thread so we can timeout
                    let handle = thread::spawn(move || {
                        // Check if we've been asked to cancel before starting
                        if cancel_token_clone.load(Ordering::SeqCst) {
                            return;
                        }
                        
                        let result = compute_cryptographic(&path_clone);
                        
                        // Only send if we haven't been cancelled
                        if !cancel_token_clone.load(Ordering::SeqCst) {
                            let _ = tx.send(result);
                        }
                    });

                    // Determine timeout based on file extension
                    let timeout_duration = {
                        if ["raw", "raf", "dng", "cr2", "nef", "arw", "orf", "rw2", "nrw", "crw", "pef", "srw", "x3f", "rwl", "3fr"].contains(&file_ext.as_str()) {
                            // Use longer timeout for RAW formats
                            std::time::Duration::from_secs(15) // 15 seconds for RAW
                        } else if ["tif", "tiff"].contains(&file_ext.as_str()) {
                            // Longer timeout for TIFF files too
                            std::time::Duration::from_secs(10) // 10 seconds for TIFF formats
                        } else {
                            std::time::Duration::from_secs(5) // 5 seconds for regular images
                        }
                    };
                    
                    // Wait with the appropriate timeout
                    match rx.recv_timeout(timeout_duration) {
                        Ok(result) => {
                            // Thread completed within timeout - ensure it's joined
                            let _ = handle.join();
                            result
                        },
                        Err(e) => {
                            // Timeout occurred, thread is still running - signal cancellation
                            cancel_token.store(true, Ordering::SeqCst);
                            
                            // Log timeout with format-specific information
                            let timeout_seconds = timeout_duration.as_secs();
                            info!("TIMEOUT: Crypto hash took too long for '{}'", path_display);

                            // Log the timeout error properly
                            let timeout_err = std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                format!("Crypto hash computation timed out after {} seconds: {:?}", timeout_seconds, e)
                            );
                            crate::logging::log_hash_error(path, &timeout_err);

                            // Abort the thread to prevent resource leaks
                            // This is a last resort but better than leaking the thread
                            let _ = handle.thread().unpark(); // Wake thread if it's parked

                            // Try to abort the thread if the OS supports it
                            #[cfg(target_os = "macos")]
                            {
                                // Try to send an abort signal
                                std::thread::yield_now(); // Give thread a chance to exit
                            }

                            // Create a cleanup thread with a name for better debugging
                            let _cleanup_thread = std::thread::Builder::new()
                                .name("crypto-cleanup".to_string())
                                .spawn(move || {
                                    // Try to join with a short timeout in a background thread
                                    let _ = handle.join();
                                });
                                
                            // Just let the cleanup thread run in the background
                            // No need to wait for it to complete

                            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout").into())
                        }
                    }
                }).unwrap_or_else(|panic_err| {
                    // Extract panic information if possible
                    let panic_msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                        format!("Panic with message: {}", s)
                    } else if let Some(s) = panic_err.downcast_ref::<String>() {
                        format!("Panic with message: {}", s)
                    } else {
                        "Unknown panic occurred".to_string()
                    };

                    info!("PANIC during crypto hash for '{}': {}", path_display, panic_msg);

                    // Log the panic properly
                    let err = std::io::Error::new(std::io::ErrorKind::Other, panic_msg);
                    crate::logging::log_hash_error(path, &err);

                    Err(err.into())
                });

                // Only compute perceptual hash if crypto hash succeeded
                let phash_result = if crypto_result.is_ok() {
                    info!("Computing perceptual hash for: '{}'", path_display);
                    std::panic::catch_unwind(|| {
                        use std::thread;
                        use std::sync::atomic::{AtomicBool, Ordering};
                        use std::sync::Arc;

                        // Create a cancellation token
                        let cancel_token = Arc::new(AtomicBool::new(false));
                        let cancel_token_clone = cancel_token.clone();

                        // Get file extension for timeout configuration before moving path
                        let file_ext = path.extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();

                        // Spawn a thread to compute the hash with a timeout
                        let path_clone = path.clone();
                        let (tx, rx) = std::sync::mpsc::channel();

                        // Compute in a separate thread so we can timeout
                        let handle = thread::spawn(move || {
                            // Check if we've been asked to cancel before starting
                            if cancel_token_clone.load(Ordering::SeqCst) {
                                return;
                            }
                            
                            // Check if this file is in the skip list from previous timeouts
                            let skip_this_file = {
                                let path_str = path_clone.to_string_lossy().to_string();
                                if let Ok(skip_list) = PROBLEMATIC_FILES.lock() {
                                    skip_list.contains(&path_str)
                                } else {
                                    false
                                }
                            };
                            
                            if skip_this_file {
                                // Create a filename-based hash instead for previously problematic files
                                log::info!("Skipping known problematic file: {}", path_clone.display());
                                
                                // Fast-path hash generation
                                let filename = path_clone.file_name().unwrap_or_default().to_string_lossy();
                                use std::hash::{Hash, Hasher};
                                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                                filename.hash(&mut hasher);
                                
                                // Add other metadata for uniqueness
                                if let Ok(metadata) = std::fs::metadata(&path_clone) {
                                    metadata.len().hash(&mut hasher);
                                    if let Ok(modified) = metadata.modified() {
                                        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                                            duration.as_secs().hash(&mut hasher);
                                        }
                                    }
                                }
                                
                                let fallback_hash = hasher.finish();
                                let _ = tx.send(Ok(PHash::Standard(fallback_hash)));
                            } else {
                                // Check if this is a TIFF file to use specialized handling
                                if let Some(ext) = path_clone.extension() {
                                    let ext_str = ext.to_string_lossy().to_lowercase();
                                    if ext_str == "tif" || ext_str == "tiff" {
                                        // Use specialized TIFF handling directly
                                        log::info!("Using specialized TIFF handler for: {}", path_clone.display());
                                        let result = process_tiff_directly(&path_clone);
                                        
                                        // Only send if we haven't been cancelled
                                        if !cancel_token_clone.load(Ordering::SeqCst) {
                                            let _ = tx.send(result);
                                        }
                                        return;
                                    }
                                }
                                
                                // Normal processing for regular files
                                let result = phash_from_file(&path_clone);
                            
                                // Only send if we haven't been cancelled
                                if !cancel_token_clone.load(Ordering::SeqCst) {
                                    let _ = tx.send(result);
                                }
                            }
                        });

                        // Determine timeout based on file extension
                        let timeout_duration = {
                            if ["raw", "raf", "dng", "cr2", "nef", "arw", "orf", "rw2", "nrw", "crw", "pef", "srw", "x3f", "rwl", "3fr"].contains(&file_ext.as_str()) {
                                // Use longer timeout for RAW formats
                                std::time::Duration::from_secs(30) // 30 seconds for RAW
                            } else if ["tif", "tiff"].contains(&file_ext.as_str()) {
                                // Longer timeout for TIFF files too
                                std::time::Duration::from_secs(20) // 20 seconds for TIFF formats
                            } else {
                                std::time::Duration::from_secs(10) // 10 seconds for regular images
                            }
                        };
                        
                        // Wait with the appropriate timeout
                        match rx.recv_timeout(timeout_duration) {
                            Ok(result) => {
                                // Thread completed within timeout - ensure it's joined
                                let _ = handle.join();
                                result
                            },
                            Err(e) => {
                                // Timeout occurred, thread is still running - signal cancellation
                                cancel_token.store(true, Ordering::SeqCst);
                                
                                // Log timeout with format-specific information
                                let timeout_seconds = timeout_duration.as_secs();
                                info!("TIMEOUT: Perceptual hash took too long for '{}'", path_display);

                                // Add this file to the global skip list for future processing
                                let path_str = path.to_string_lossy().to_string();
                                if let Ok(mut skip_list) = PROBLEMATIC_FILES.lock() {
                                    skip_list.insert(path_str);
                                    info!("Added {} to problematic files skip list (now {} entries)", 
                                          path_display, skip_list.len());
                                }

                                // Log the timeout error properly
                                let timeout_err = std::io::Error::new(
                                    std::io::ErrorKind::TimedOut,
                                    format!("Perceptual hash computation timed out after {} seconds: {:?}", timeout_seconds, e)
                                );
                                crate::logging::log_hash_error(path, &timeout_err);

                                // Abort the thread to prevent resource leaks
                                // This is a last resort but better than leaking the thread
                                let _ = handle.thread().unpark(); // Wake thread if it's parked

                                // Try to abort the thread if the OS supports it
                                #[cfg(target_os = "macos")]
                                {
                                    // Try to send an abort signal
                                    std::thread::yield_now(); // Give thread a chance to exit
                                }

                                // Create a cleanup thread with a name for better debugging
                                let _cleanup_thread = std::thread::Builder::new()
                                    .name("phash-cleanup".to_string())
                                    .spawn(move || {
                                        // Try to join with a short timeout in a background thread
                                        let _ = handle.join();
                                    });
                                    
                                // Just let the cleanup thread run in the background
                                // No need to wait for it to complete

                                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout").into())
                            }
                        }
                    }).unwrap_or_else(|panic_err| {
                        // Extract panic information if possible
                        let panic_msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                            format!("Panic with message: {}", s)
                        } else if let Some(s) = panic_err.downcast_ref::<String>() {
                            format!("Panic with message: {}", s)
                        } else {
                            "Unknown panic occurred".to_string()
                        };

                        info!("PANIC during perceptual hash for '{}': {}", path_display, panic_msg);

                        // Log the panic properly
                        let err = std::io::Error::new(std::io::ErrorKind::Other, panic_msg);
                        crate::logging::log_hash_error(path, &err);

                        Err(err.into())
                    })
                } else {
                    // Skip perceptual hash if crypto hash failed
                    info!("Skipping perceptual hash due to crypto hash failure for '{}'", path_display);
                    Err(std::io::Error::new(std::io::ErrorKind::Other, "Skipped").into())
                };

                // Process results
                let result = match (crypto_result, phash_result) {
                    (Ok(blake3), Ok(phash)) => {
                        // Increment progress if counter provided
                        if let Some(counter) = progress_counter {
                            counter.fetch_add(1, Ordering::Relaxed);
                        }

                        processed_counter.fetch_add(1, Ordering::Relaxed);
                        let processed = processed_counter.load(Ordering::Relaxed);
                        let elapsed = start.elapsed();

                        // Log progress (only for longer operations or periodically)
                        if processed % 20 == 0 || elapsed > std::time::Duration::from_secs(3) {
                            info!("Processed: {}/{} - '{}' in {:.2?}",
                                 processed, paths.len(), path_display, elapsed);
                        }

                        Some(ImageHashResult {
                            path: path.clone(),
                            cryptographic: blake3,
                            perceptual: phash,
                        })
                    }
                    (crypto_result, phash_result) => {
                        // Count error and increment progress
                        error_counter.fetch_add(1, Ordering::Relaxed);
                        if let Some(counter) = progress_counter {
                            counter.fetch_add(1, Ordering::Relaxed);
                        }

                        // Use dedicated logging functions for hash errors
                        if let Err(e) = &crypto_result {
                            // Use your custom logging function for hash errors
                            crate::logging::log_hash_error(path, &format!("{}", e));
                            info!("Crypto hash failed for '{}'", path_display);
                        }

                        if let Err(e) = &phash_result {
                            // Use your custom logging function for hash errors
                            crate::logging::log_hash_error(path, &format!("{}", e));
                            info!("Perceptual hash failed for '{}'", path_display);
                        }

                        // Log a summary of the failure
                        info!("Failed to process: {}", path_display);

                        None
                    }
                };

                result
            })
            .filter_map(|r| r)
            .collect()
    });

    let batch_duration = batch_start.elapsed();

    // Track memory usage after processing
    system.refresh_all();
    let end_mem = system.used_memory() / 1024 / 1024; // Convert to MB
    let mem_diff = if end_mem > start_mem {
        end_mem - start_mem
    } else {
        0
    };

    // Log results to file
    info!(
        "Batch completed: {} successful, {} errors in {:.2?}",
        results.len(),
        error_counter.load(Ordering::Relaxed),
        batch_duration
    );

    // Log more detailed info to log file
    info!(
        "Memory usage: before={}MB, after={}MB, diff=+{}MB",
        start_mem, end_mem, mem_diff
    );

    // Check results size
    let result_estimate = results.len() * std::mem::size_of::<ImageHashResult>();
    info!("Approximate result size: ~{}KB", result_estimate / 1024);

    (results, error_counter.load(Ordering::Relaxed))
}

/// Simplified batch processor that handles chunking for memory efficiency
pub fn process_images_in_batches(
    images: &[PathBuf],
    batch_size: usize,
    progress_counter: Option<&Arc<AtomicUsize>>,
) -> Vec<ImageHashResult> {
    // This function processes batches sequentially
    use sysinfo::System;

    // Initialize memory tracking
    let mut system = System::new_all();
    system.refresh_memory();
    let start_mem = system.used_memory() / 1024 / 1024; // Convert to MB
    println!("Initial memory usage for batch processing: {}MB", start_mem);

    let total_images = images.len();
    let mut results = Vec::new(); // Don't pre-allocate to avoid excess memory usage
    let mut total_errors = 0;
    let batch_start = std::time::Instant::now();

    // Process images in sequential batches to control memory usage
    for (i, chunk) in images.chunks(batch_size).enumerate() {
        // Check memory before this batch
        system.refresh_memory();
        let before_batch_mem = system.used_memory() / 1024 / 1024;
        println!("Memory before batch {}: {}MB", i + 1, before_batch_mem);

        // Create a smaller slice to work with
        let chunk_slice = &chunk[0..chunk.len()];

        // Process this batch
        let (batch_results, errors) = process_image_batch(chunk_slice, progress_counter);

        // Track errors
        total_errors += errors;

        // Check memory after batch processing but before adding to results
        system.refresh_memory();
        let after_proc_mem = system.used_memory() / 1024 / 1024;
        // Log to file only, not console
        info!(
            "Memory after batch {} processing: {}MB ({}MB change)",
            i + 1,
            after_proc_mem,
            if after_proc_mem > before_batch_mem {
                after_proc_mem - before_batch_mem
            } else {
                0
            }
        );

        // Store results but limit memory usage
        let results_to_keep = std::cmp::min(batch_results.len(), 1000);
        let should_store = results.len() < 1000;

        // Check memory before extending results
        system.refresh_memory();
        let before_extend_mem = system.used_memory() / 1024 / 1024;

        // Only keep up to 1000 results to avoid memory bloat
        if should_store {
            results.extend(batch_results.into_iter().take(results_to_keep));
        } else {
            // Drop batch_results explicitly when not storing
            drop(batch_results);
        }

        // Check memory after adding results
        system.refresh_memory();
        let after_extend_mem = system.used_memory() / 1024 / 1024;
        info!(
            "Memory after adding to results: {}MB ({}MB change)",
            after_extend_mem,
            if after_extend_mem > before_extend_mem {
                after_extend_mem - before_extend_mem
            } else {
                0
            }
        );

        // Log progress (only to file, not console)
        info!(
            "Processed batch {}/{} ({} images, {} errors)",
            i + 1,
            (total_images + batch_size - 1) / batch_size,
            chunk_slice.len(),
            errors
        );

        // Update progress counter if provided
        if let Some(counter) = progress_counter {
            counter.fetch_add(chunk_slice.len(), std::sync::atomic::Ordering::Relaxed);
        }

        // Force memory cleanup of remaining objects
        #[allow(dropping_references)]
        drop(chunk_slice);
        let _ = chunk_slice;

        // Check memory after cleanup
        system.refresh_memory();
        let after_cleanup_mem = system.used_memory() / 1024 / 1024;
        info!(
            "Memory after cleanup: {}MB ({}MB change)",
            after_cleanup_mem,
            if after_cleanup_mem > after_extend_mem {
                format!("+{}", after_cleanup_mem - after_extend_mem)
            } else {
                format!("-{}", after_extend_mem - after_cleanup_mem)
            }
        );

        // Memory cleanup (would use jemalloc if available)
        if i % 5 == 0 {
            // If we had jemalloc as a feature, we could force cleanup
            // For now just suggest to the OS that now is a good time for GC
            std::thread::sleep(std::time::Duration::from_millis(100));
            info!("Suggested memory cleanup after batch {}", i + 1);
        }

        // Explicit pause to let system recover and release resources
        if i % 2 == 0 {
            // Do this more frequently
            std::thread::sleep(std::time::Duration::from_millis(500)); // Longer pause
        }

        // Periodic full cleanup
        if i % 10 == 0 && i > 0 {
            // Release memory pressure by clearing and shrinking results
            if !results.is_empty() {
                results.clear();
                results.shrink_to_fit();
            }

            // Explicitly request OS to reclaim memory
            #[cfg(target_os = "linux")]
            {
                unsafe {
                    libc::malloc_trim(0);
                }
            }

            // Force a longer pause for system to recover
            std::thread::sleep(std::time::Duration::from_secs(2));
            info!("Performed full memory cleanup after batch {}", i + 1);
        }
    }

    // Final memory check
    system.refresh_memory();
    let end_mem = system.used_memory() / 1024 / 1024; // Convert to MB
    let mem_diff = if end_mem > start_mem {
        end_mem - start_mem
    } else {
        0
    };
    let batch_duration = batch_start.elapsed();

    info!(
        "Processing complete: {} successful, {} errors",
        results.len(),
        total_errors
    );
    info!("Total processing time: {:.2?}", batch_duration);
    info!(
        "Final memory usage: before={}MB, after={}MB, diff=+{}MB",
        start_mem, end_mem, mem_diff
    );
    info!(
        "Memory per successful result: ~{}KB",
        if results.len() > 0 {
            (mem_diff * 1024) / results.len() as u64
        } else {
            0
        }
    );

    results
}

/// Simple wrapper for backward compatibility
pub fn process_images(images: &[PathBuf]) -> Vec<ImageHashResult> {
    // Use a reasonable batch size to limit memory usage
    const DEFAULT_BATCH_SIZE: usize = 50;

    process_images_in_batches(images, DEFAULT_BATCH_SIZE, None)
}
