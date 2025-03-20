/// Example to test the structured logging for BetterStack with accurate source location tracking
use std::path::PathBuf;
use std::io::{Error, ErrorKind};

// Import the logging macros to capture accurate source information
use image_deduper_core::{log_file_error, log_hash_error, log_fs_modification, send_direct_betterstack_log};
use image_deduper_core::logging;
use image_deduper_core::Result;

fn log_test_1() {
    // Test hash error logging using macro - this should record this file and line number
    let test_path = PathBuf::from("/sample/image/photo1.raf");
    log_hash_error!(&test_path, Error::new(ErrorKind::TimedOut, "Perceptual hash timed out after 30 seconds: Timeout"));
}

fn log_test_2() {
    // Test file error logging using macro - this should record this file and line number
    let file_path = PathBuf::from("/sample/image/photo2.raf");
    log_file_error!(&file_path, "read", &Error::new(ErrorKind::TimedOut, "I/O error: Timeout"));
}

fn log_test_3() {
    // Test filesystem change logging using macro - this should record this file and line number
    let fs_path = PathBuf::from("/sample/output/processed");
    log_fs_modification!("create_directory", &fs_path, Some("Creating output directory for processed images"));
}

fn main() -> Result<()> {
    // Initialize BetterStack logging
    logging::init_logger()?;
    println!("Logging initialized - testing BetterStack structured logging with source location tracking");
    
    // Call test functions with logging to demonstrate source location capture
    log_test_1();
    log_test_2();
    log_test_3();
    
    // Show difference between using the functions vs. macros
    println!("\nComparing function vs. macro source location tracking...");
    
    // Using functions (will record location in logging.rs)
    println!("Using functions (location will be in logging.rs):");
    let test_path = PathBuf::from("/sample/image/function-test.raf");
    logging::log_hash_error(&test_path, Error::new(ErrorKind::TimedOut, "Function-based logging"));
    
    // Using macros (will record this file and line number)
    println!("Using macros (location will be in test_logging.rs):");
    let test_path = PathBuf::from("/sample/image/macro-test.raf");
    log_hash_error!(&test_path, Error::new(ErrorKind::TimedOut, "Macro-based logging"));
    
    // Test direct BetterStack logging with macro
    println!("\nTesting direct BetterStack logging with macro...");
    
    // Send an ERROR level log with macro - should record this location
    println!("Sending ERROR level log with macro...");
    match send_direct_betterstack_log!(
        "MACRO ERROR - Should record test_logging.rs location", 
        "ERROR", 
        Some("test_operation"), 
        Some("/test/path/file.jpg"), 
        Some("application_error"), 
        Some("Detailed test error message")
    ) {
        Ok(_) => println!("✅ Direct ERROR log sent successfully"),
        Err(e) => println!("❌ Failed to send direct ERROR log: {}", e),
    }
    
    // Send a WARN level log with function - will record logging.rs location
    println!("Sending WARN level log with function...");
    match logging::send_direct_betterstack_log(
        "FUNCTION WARNING - Will record logging.rs location", 
        "WARN", 
        Some("validation_operation"), 
        Some("/test/path/data.json"), 
        Some("validation_warning"), 
        Some("Data validation warning")
    ) {
        Ok(_) => println!("✅ Direct WARN log sent successfully"),
        Err(e) => println!("❌ Failed to send direct WARN log: {}", e),
    }
    
    // Wait to ensure all logs are sent to BetterStack
    println!("\nWaiting for background logs to be sent...");
    std::thread::sleep(std::time::Duration::from_secs(3));
    
    // Shutdown logger properly
    logging::shutdown_logger();
    
    println!("\nLogging test completed - check BetterStack dashboard for accurate source location information");
    println!("You should see logs with source in 'test_logging.rs' for the macros and 'logging.rs' for the functions");
    
    Ok(())
}
