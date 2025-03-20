use crate::error::{Error, Result};
use log::{error, info, LevelFilter, Record};
use std::path::Path;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration; // Required for log4rs's Append trait

use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

// Custom appender for BetterStack
use anyhow;
use log4rs::append::Append;
use log4rs::encode::Encode;
use serde_json::json;
use std::io;

// Constants for BetterStack
***REMOVED***
***REMOVED***

// Channel sender to send logs to background thread
static mut LOG_SENDER: Option<Sender<String>> = None;

/// Custom BetterStack appender
pub struct BetterStackAppender {
    encoder: Box<dyn Encode + Send + Sync>,
    min_level: LevelFilter,
}

// Custom implementation of Debug for BetterStackAppender since Box<dyn Encode> doesn't implement Debug
impl std::fmt::Debug for BetterStackAppender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BetterStackAppender")
            .field("min_level", &self.min_level)
            .field("encoder", &"<dyn Encode>") // Just show a placeholder for the encoder
            .finish()
    }
}

impl BetterStackAppender {
    pub fn new(encoder: Box<dyn Encode + Send + Sync>, min_level: LevelFilter) -> Self {
        // Start the background worker thread when creating the appender
        let (tx, rx) = channel::<String>();

        // Store sender in static variable for direct access if needed
        unsafe {
            LOG_SENDER = Some(tx.clone());
        }

        // Spawn background thread to process log messages
        thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());

            while let Ok(log_message) = rx.recv() {
                // Don't block too long on sending logs
                let _result = client
                    .post(BETTERSTACK_API_URL)
                    .header("Content-Type", "application/json")
                    .header("Authorization", format!("Bearer {}", BETTERSTACK_API_TOKEN))
                    .body(log_message)
                    .send();

                // Sleep briefly to avoid overwhelming the API
                thread::sleep(Duration::from_millis(10));
            }
        });

        Self { encoder, min_level }
    }

    fn format_log(&self, record: &Record) -> Option<String> {
        // Parse the log message to extract structured data
        let message = record.args().to_string();
        
        // Extract operation, path, and error details for structured logs
        // Patterns to match:
        // 1. "Hash computation failed - Path: /path/to/file, Error: I/O error: Timeout"
        // 2. "File operation failed - Operation: read, Path: /path/to/file, Error: Permission denied"
        
        let (operation, path, error_type, error_details) = if message.starts_with("Hash computation failed") {
            // Extract path and error from hash error messages
            let parts: Vec<&str> = message.splitn(2, " - Path: ").collect();
            if parts.len() == 2 {
                let path_error_parts: Vec<&str> = parts[1].splitn(2, ", Error: ").collect();
                if path_error_parts.len() == 2 {
                    let error_parts: Vec<&str> = path_error_parts[1].splitn(2, ": ").collect();
                    if error_parts.len() == 2 {
                        ("hash_computation", path_error_parts[0], error_parts[0], error_parts[1])
                    } else {
                        ("hash_computation", path_error_parts[0], "generic_error", path_error_parts[1])
                    }
                } else {
                    ("hash_computation", parts[1], "unknown", "")
                }
            } else {
                ("hash_computation", "", "unknown", message.as_str())
            }
        } else if message.starts_with("File operation failed") {
            // Extract operation, path, and error from file operation messages
            let operation_parts: Vec<&str> = message.splitn(2, "Operation: ").collect();
            if operation_parts.len() == 2 {
                let op_path_parts: Vec<&str> = operation_parts[1].splitn(2, ", Path: ").collect();
                if op_path_parts.len() == 2 {
                    let path_error_parts: Vec<&str> = op_path_parts[1].splitn(2, ", Error: ").collect();
                    if path_error_parts.len() == 2 {
                        (op_path_parts[0], path_error_parts[0], "file_operation", path_error_parts[1])
                    } else {
                        (op_path_parts[0], op_path_parts[1], "unknown", "")
                    }
                } else {
                    (operation_parts[1], "", "unknown", "")
                }
            } else {
                ("file_operation", "", "unknown", message.as_str())
            }
        } else if message.starts_with("FS CHANGE") {
            // Extract operation and path from filesystem change messages
            let parts: Vec<&str> = message.splitn(2, "Operation: ").collect();
            if parts.len() == 2 {
                let op_path_parts: Vec<&str> = parts[1].splitn(2, ", Path: ").collect();
                if op_path_parts.len() == 2 {
                    let path_details: Vec<&str> = op_path_parts[1].splitn(2, ", Details: ").collect();
                    if path_details.len() == 2 {
                        (op_path_parts[0], path_details[0], "fs_change", path_details[1])
                    } else {
                        (op_path_parts[0], op_path_parts[1], "fs_change", "")
                    }
                } else {
                    (parts[1], "", "fs_change", "")
                }
            } else {
                ("fs_change", "", "unknown", message.as_str())
            }
        } else {
            // For other messages, don't try to parse
            ("other", "", "application", message.as_str())
        };
        
        // Create the current timestamp in UTC
        let now = chrono::Utc::now().format("%Y-%m-%d %T UTC").to_string();
        
        // Construct structured JSON payload for BetterStack
        // Keep the "message" field as BetterStack likely uses this as the primary display field
        let payload = json!({
            "dt": now,
            "message": format!("{} error on {}: {}", error_type, path, error_details),
            "raw_message": message,
            "summary": format!("{} error on {}", error_type, path),
            "level": record.level().to_string(),
            "source_module": record.target(),
            "source_file": record.file(),
            "source_line": record.line(),
            "operation": operation,
            "path": path,
            "error_type": error_type,
            "error_details": error_details
        });
        
        Some(payload.to_string())
    }
}

// Implement the log4rs Append trait for BetterStackAppender
impl Append for BetterStackAppender {
    fn append(&self, record: &Record) -> anyhow::Result<()> {
        // Only process logs at or above the minimum level
        if record.level() <= self.min_level {
            if let Some(formatted) = self.format_log(record) {
                // Send log to the background thread without blocking
                if let Some(sender) = unsafe { LOG_SENDER.as_ref() } {
                    // Just log, don't propagate error - we want this to be non-blocking
                    if sender.send(formatted).is_err() {
                        // Nothing we can really do here, but we shouldn't fail the appender
                        eprintln!("Failed to send log to BetterStack background thread");
                    }
                }
            }
        }
        Ok(())
    }

    fn flush(&self) {
        // No explicit flush needed as logs are sent asynchronously
    }
}

/// Initialize the logger with timestamp, log level, and module path
/// Logs will be sent to BetterStack
pub fn init_logger() -> Result<()> {
    // Get log level from environment or default to info
    let env_filter = std::env::var("DEDUP_LOG").unwrap_or_else(|_| "debug".to_string());
    let level = env_filter
        .parse::<LevelFilter>()
        .unwrap_or(LevelFilter::Info);

    // Create BetterStack appender with appropriate log level
    let betterstack_level = LevelFilter::Warn; // Only send warnings and above by default
    let betterstack_encoder = Box::new(PatternEncoder::new("[{l}] [{M}:{L}] - {m}"));
    let betterstack_appender = BetterStackAppender::new(betterstack_encoder, betterstack_level);

    // Build the logger configuration with only BetterStack appender
    let config = Config::builder()
        .appender(Appender::builder().build("betterstack", Box::new(betterstack_appender)))
        .build(
            Root::builder()
                .appender("betterstack")
                .build(level),
        )
        .map_err(|e| Error::Unknown(format!("Failed to build log config: {}", e)))?;

    // Use the configured logger
    log4rs::init_config(config)
        .map_err(|e| Error::Unknown(format!("Failed to initialize log4rs: {}", e)))?;

    // Set the max level for the log crate as well
    log::set_max_level(level);

    info!("Image deduplication application started");
    info!(
        "Remote logging to BetterStack enabled for level: {} and above",
        betterstack_level
    );
    Ok(())
}

/// Log file operation that failed
pub fn log_file_error(path: &Path, operation: &str, error: &dyn std::error::Error) {
    // Convert the error to string and try to extract error type and details
    let error_string = error.to_string();
    let (error_type, error_details) = parse_error_message(&error_string);
    
    error!(
        "File operation failed - Operation: {}, Path: {}, Error: {}",
        operation,
        path.display(),
        error
    );
}

/// Log hash computation error
pub fn log_hash_error<E: std::fmt::Display>(path: &Path, error: E) {
    // Convert the error to string and try to extract error type and details
    let error_string = error.to_string();
    let (error_type, error_details) = parse_error_message(&error_string);
    
    error!(
        "Hash computation failed - Path: {}, Error: {}",
        path.display(),
        error
    );
}

/// Helper function to parse error messages to extract type and details
fn parse_error_message(error_msg: &str) -> (&str, &str) {
    // Common error patterns
    if error_msg.contains("I/O error:") {
        let parts: Vec<&str> = error_msg.splitn(2, "I/O error:").collect();
        if parts.len() == 2 {
            return ("I/O error", parts[1].trim());
        }
    } else if error_msg.contains("Timeout") || error_msg.contains("timed out") {
        return ("Timeout", error_msg);
    } else if error_msg.contains("Permission denied") {
        return ("Permission denied", error_msg);
    } else if error_msg.contains("Perceptual hash") {
        return ("Perceptual hash", error_msg);
    }
    
    // Default case - keep the whole message
    ("error", error_msg)
}

/// Log file system modification
pub fn log_fs_modification(operation: &str, path: &Path, details: Option<&str>) {
    let details_str = details.unwrap_or("");
    info!(
        "FS CHANGE - Operation: {}, Path: {}{}",
        operation,
        path.display(),
        if details_str.is_empty() {
            "".to_string()
        } else {
            format!(", Details: {}", details_str)
        }
    );
}

/// Send a log directly to BetterStack bypassing the standard logging
/// Useful for critical events or when the logger isn't properly initialized
pub fn send_direct_betterstack_log(
    message: &str, 
    level: &str, 
    operation: Option<&str>, 
    path: Option<&str>, 
    error_type: Option<&str>, 
    details: Option<&str>
) -> Result<()> {
    // Create a timestamp in UTC
    let now = chrono::Utc::now().format("%Y-%m-%d %T UTC").to_string();

    // Use provided values or defaults
    let op = operation.unwrap_or("direct");
    let path_str = path.unwrap_or("");
    let err_type = error_type.unwrap_or("application");
    let err_details = details.unwrap_or(message);

    // Prepare the structured JSON payload
    let payload = json!({
        "dt": now,
        "message": format!("{} event on {}: {}", err_type, path_str, err_details),
        "raw_message": message,
        "summary": format!("{} event{}", 
                          err_type, 
                          if path_str.is_empty() { "".to_string() } else { format!(" on {}", path_str) }),
        "level": level,
        "source_module": "direct",
        "source_file": "manual_log",
        "source_line": 0,
        "operation": op,
        "path": path_str,
        "error_type": err_type,
        "error_details": err_details,
        "direct": true
    });

    // Send the log directly
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(BETTERSTACK_API_URL)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", BETTERSTACK_API_TOKEN))
        .body(payload.to_string())
        .send()
        .map_err(|e| Error::Unknown(format!("Failed to send direct log to BetterStack: {}", e)))?;

    if !response.status().is_success() {
        return Err(Error::Unknown(format!(
            "Failed to send log to BetterStack: HTTP {}",
            response.status()
        )));
    }

    Ok(())
}

/// Shutdown the logger gracefully
/// This should be called before application exit to ensure all logs are sent
pub fn shutdown_logger() {
    // Ensure we flush any pending logs
    log::logger().flush();

    // Give background thread time to send remaining logs
    thread::sleep(Duration::from_millis(200));
}
