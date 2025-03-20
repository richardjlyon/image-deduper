use crate::log_hash_error;
use log::info;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Types of hash operations that can be performed
pub enum HashOperation {
    Cryptographic,
    Perceptual,
}

/// Extract panic info from panic value
pub fn extract_panic_info(panic_err: Box<dyn std::any::Any + Send>) -> String {
    // Extract panic information if possible
    if let Some(s) = panic_err.downcast_ref::<&str>() {
        format!("Panic with message: {}", s)
    } else if let Some(s) = panic_err.downcast_ref::<String>() {
        format!("Panic with message: {}", s)
    } else {
        "Unknown panic occurred".to_string()
    }
}

/// Get the appropriate timeout duration based on file extension and operation type
pub fn get_timeout_duration(file_ext: &str, operation: HashOperation) -> Duration {
    match operation {
        HashOperation::Cryptographic => {
            if [
                "raw", "raf", "dng", "cr2", "nef", "arw", "orf", "rw2", "nrw", "crw", "pef", "srw",
                "x3f", "rwl", "3fr",
            ]
            .contains(&file_ext)
            {
                Duration::from_secs(15) // 15 seconds for RAW
            } else if ["tif", "tiff"].contains(&file_ext) {
                Duration::from_secs(10) // 10 seconds for TIFF
            } else {
                Duration::from_secs(5) // 5 seconds for regular images
            }
        }
        HashOperation::Perceptual => {
            if [
                "raw", "raf", "dng", "cr2", "nef", "arw", "orf", "rw2", "nrw", "crw", "pef", "srw",
                "x3f", "rwl", "3fr",
            ]
            .contains(&file_ext)
            {
                Duration::from_secs(30) // 30 seconds for RAW
            } else if ["tif", "tiff"].contains(&file_ext) {
                Duration::from_secs(20) // 20 seconds for TIFF formats
            } else {
                Duration::from_secs(10) // 10 seconds for regular images
            }
        }
    }
}

/// Execute a function with a timeout
/// Returns Ok(T) if the function completes within the timeout
/// Returns Err(std::io::Error) if the function times out or panics
pub fn execute_with_timeout<T, F>(
    path: &Path,
    operation_name: &str,
    timeout: Duration,
    task: F,
) -> Result<T, std::io::Error>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    use std::sync::mpsc;
    use std::thread;

    let path_display = path.display().to_string();

    // Create a cancellation token
    let cancel_token = Arc::new(AtomicBool::new(false));
    let cancel_token_clone = cancel_token.clone();

    // Spawn a thread to compute the hash with a timeout
    // Clone path for thread safety (but unused in simple implementation)
    let _path_clone = path.to_path_buf();
    let (tx, rx) = mpsc::channel();

    // Compute in a separate thread so we can timeout
    let handle = thread::spawn(move || {
        // Check if we've been asked to cancel before starting
        if cancel_token_clone.load(Ordering::SeqCst) {
            return;
        }

        let result = task();

        // Only send if we haven't been cancelled
        if !cancel_token_clone.load(Ordering::SeqCst) {
            let _ = tx.send(result);
        }
    });

    // Wait with the timeout
    match rx.recv_timeout(timeout) {
        Ok(result) => {
            // Thread completed within timeout - ensure it's joined
            let _ = handle.join();
            Ok(result)
        }
        Err(e) => {
            // Timeout occurred, thread is still running - signal cancellation
            cancel_token.store(true, Ordering::SeqCst);

            // Log timeout with information
            let timeout_seconds = timeout.as_secs();
            info!(
                "TIMEOUT: {} took too long for '{}'",
                operation_name, path_display
            );

            // Log the timeout error properly
            let timeout_err = std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "{} timed out after {} seconds: {:?}",
                    operation_name, timeout_seconds, e
                ),
            );
            log_hash_error!(path, &timeout_err);

            // Abort the thread to prevent resource leaks
            let _ = handle.thread().unpark(); // Wake thread if it's parked

            // Try to abort the thread if the OS supports it
            #[cfg(target_os = "macos")]
            {
                // Try to send an abort signal
                std::thread::yield_now(); // Give thread a chance to exit
            }

            // Create a cleanup thread with a name for better debugging
            let thread_name = format!("{}-cleanup", operation_name.to_lowercase());
            let _cleanup_thread = std::thread::Builder::new()
                .name(thread_name)
                .spawn(move || {
                    // Try to join with a short timeout in a background thread
                    let _ = handle.join();
                });

            // Return error
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout"))
        }
    }
}
