/// Example to test the new structured logging for BetterStack
use std::path::PathBuf;
use std::io::{Error, ErrorKind};

use image_deduper_core::logging;
use image_deduper_core::Result;

fn main() -> Result<()> {
    // Initialize BetterStack logging
    logging::init_logger()?;
    println!("Logging initialized - testing different error types");
    
    // Test standard log macros at different levels
    log::error!("TEST ERROR MESSAGE - This is a test error message");
    log::warn!("TEST WARNING MESSAGE - This is a test warning message");
    log::info!("TEST INFO MESSAGE - This is a test info message");
    
    // Test hash error logging
    let test_path = PathBuf::from("/sample/image/photo.raf");
    logging::log_hash_error(&test_path, Error::new(ErrorKind::TimedOut, "Perceptual hash timed out after 30 seconds: Timeout"));
    
    // Test file error logging
    let file_path = PathBuf::from("/sample/image/another-photo.raf");
    logging::log_file_error(&file_path, "read", &Error::new(ErrorKind::TimedOut, "I/O error: Timeout"));
    
    // Test filesystem change logging
    let fs_path = PathBuf::from("/sample/output/processed");
    logging::log_fs_modification("create_directory", &fs_path, Some("Creating output directory for processed images"));
    
    // Test direct logging with different parameters for each log level
    println!("Sending direct logs to BetterStack at different levels...");
    
    // ERROR level
    match logging::send_direct_betterstack_log(
        "TEST DIRECT ERROR - Manual test log entry with structured data", 
        "ERROR", 
        Some("test_error_operation"), 
        Some("/test/error/path"), 
        Some("test_error"), 
        Some("Detailed error information for testing")
    ) {
        Ok(_) => println!("✅ Direct ERROR log sent successfully"),
        Err(e) => println!("❌ Failed to send direct ERROR log: {}", e),
    }
    
    // WARN level
    match logging::send_direct_betterstack_log(
        "TEST DIRECT WARNING - This is a warning level direct log", 
        "WARN", 
        Some("test_warn_operation"), 
        Some("/test/warn/path"), 
        Some("test_warning"), 
        Some("Warning details here")
    ) {
        Ok(_) => println!("✅ Direct WARN log sent successfully"),
        Err(e) => println!("❌ Failed to send direct WARN log: {}", e),
    }
    
    // INFO level
    match logging::send_direct_betterstack_log(
        "TEST DIRECT INFO - This is an info level direct log", 
        "INFO", 
        Some("test_info_operation"), 
        Some("/test/info/path"), 
        Some("test_info"), 
        Some("Info details here")
    ) {
        Ok(_) => println!("✅ Direct INFO log sent successfully"),
        Err(e) => println!("❌ Failed to send direct INFO log: {}", e),
    }
    
    // Wait to ensure logs are sent
    println!("Waiting for background logs to be sent...");
    std::thread::sleep(std::time::Duration::from_secs(3));
    
    // Shutdown logger properly
    logging::shutdown_logger();
    
    println!("Logging test completed - check BetterStack dashboard");
    Ok(())
}
