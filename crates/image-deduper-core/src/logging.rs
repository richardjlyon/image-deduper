use crate::error::{Error, Result};
use log::{info, LevelFilter, Record};
use log4rs::append::console::{ConsoleAppender, Target};
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

// Constants for BetterStack
***REMOVED***
***REMOVED***

// Channel sender to send logs to background thread
static mut LOG_SENDER: Option<Sender<String>> = None;

/// Custom BetterStack appender
#[allow(dead_code)]
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
        let (operation, path, error_type, error_details) = if message
            .starts_with("Hash computation failed")
        {
            // Extract path and error from hash error messages
            let parts: Vec<&str> = message.splitn(2, " - Path: ").collect();
            if parts.len() == 2 {
                let path_error_parts: Vec<&str> = parts[1].splitn(2, ", Error: ").collect();
                if path_error_parts.len() == 2 {
                    let error_parts: Vec<&str> = path_error_parts[1].splitn(2, ": ").collect();
                    if error_parts.len() == 2 {
                        (
                            "hash_computation",
                            path_error_parts[0],
                            error_parts[0],
                            error_parts[1],
                        )
                    } else {
                        (
                            "hash_computation",
                            path_error_parts[0],
                            "generic_error",
                            path_error_parts[1],
                        )
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
                    let path_error_parts: Vec<&str> =
                        op_path_parts[1].splitn(2, ", Error: ").collect();
                    if path_error_parts.len() == 2 {
                        (
                            op_path_parts[0],
                            path_error_parts[0],
                            "file_operation",
                            path_error_parts[1],
                        )
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
                    let path_details: Vec<&str> =
                        op_path_parts[1].splitn(2, ", Details: ").collect();
                    if path_details.len() == 2 {
                        (
                            op_path_parts[0],
                            path_details[0],
                            "fs_change",
                            path_details[1],
                        )
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

        // Construct structured JSON payload according to BetterStack documentation
        let formatted_message = format!("{} error on {}: {}", error_type, path, error_details);

        // Extract caller module, file, and line information from the log record
        // The target should contain the module path when using our macros
        let module = record.target();

        // File and line should come directly from the log record
        let file = record.file().unwrap_or("unknown");
        let line = record.line().unwrap_or(0);

        // Build the data object with all the structured fields
        let data = json!({
            "raw_message": message,
            "formatted_message": formatted_message,
            "context": {
                "source": {
                    "module": module,
                    "file": file,
                    "line": line
                },
                "operation": operation,
                "path": path,
                "error": {
                    "type": error_type,
                    "details": error_details
                },
                "timestamp": now
            }
        });

        // Construct the outer payload according to BetterStack format
        let payload = json!({
            "level": record.level().to_string(),
            "data": data
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

    // Create a console appender
    let console_encoder = Box::new(PatternEncoder::new("[{l}] [{M}:{L}] - {m}\n"));
    let console_appender = ConsoleAppender::builder()
        .encoder(console_encoder)
        .target(Target::Stdout)
        .build();

    // Build the logger configuration with only BetterStack appender
    let config = Config::builder()
        .appender(Appender::builder().build("betterstack", Box::new(betterstack_appender)))
        .appender(Appender::builder().build("console", Box::new(console_appender)))
        .build(
            Root::builder()
                .appender("betterstack")
                .appender("console")
                .build(level),
        )
        .map_err(|e| Error::Unknown(format!("Failed to build log config: {}", e)))?;

    println!("->> logger config created");

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
///
/// This macro captures the caller's file, line, and module info
#[macro_export]
macro_rules! log_file_error {
    ($path:expr, $operation:expr, $error:expr) => {
        log::error!(
            target: module_path!(),
            "File operation failed - Operation: {}, Path: {}, Error: {}",
            $operation,
            $path.display(),
            $error
        );
    };
}

/// Log hash computation error
///
/// This macro captures the caller's file, line, and module info
#[macro_export]
macro_rules! log_hash_error {
    ($path:expr, $error:expr) => {
        log::error!(
            target: module_path!(),
            "Hash computation failed - Path: {}, Error: {}",
            $path.display(),
            $error
        );
    };
}

/// Log file system modification
///
/// This macro captures the caller's file, line, and module info
#[macro_export]
macro_rules! log_fs_modification {
    ($operation:expr, $path:expr, $details:expr) => {
        log::info!(
            target: module_path!(),
            "FS CHANGE - Operation: {}, Path: {}{}",
            $operation,
            $path.display(),
            if let Some(details_str) = $details {
                format!(", Details: {}", details_str)
            } else {
                String::new()
            }
        );
    };
}

/// Log database operations
///
/// This macro captures the caller's file, line, and module info
#[macro_export]
macro_rules! log_db_operation {
    ($operation:expr, $details:expr) => {
        log::info!(
            target: module_path!(),
            "DB OPERATION - Operation: {}, Details: {}",
            $operation,
            $details
        );
    };
}

/// Send a log directly to BetterStack bypassing the standard logging
/// Useful for critical events or when the logger isn't properly initialized
#[macro_export]
macro_rules! send_direct_betterstack_log {
    ($message:expr, $level:expr, $operation:expr, $path:expr, $error_type:expr, $details:expr) => {
        crate::logging::_send_direct_betterstack_log(
            $message,
            $level,
            $operation,
            $path,
            $error_type,
            $details,
            module_path!(),
            file!(),
            line!(),
        )
    };
}
// Internal implementation function not meant to be called directly
// Users should use the send_direct_betterstack_log macro instead
pub fn _send_direct_betterstack_log(
    message: &str,
    level: &str,
    operation: Option<&str>,
    path: Option<&str>,
    error_type: Option<&str>,
    details: Option<&str>,
    module: &str,
    file: &str,
    line: u32,
) -> Result<()> {
    // Create a timestamp in UTC
    let now = chrono::Utc::now().format("%Y-%m-%d %T UTC").to_string();

    // Use provided values or defaults
    let op = operation.unwrap_or("direct");
    let path_str = path.unwrap_or("");
    let err_type = error_type.unwrap_or("application");
    let err_details = details.unwrap_or(message);

    // Create formatted message
    let formatted_message = format!(
        "{} event on {}: {}",
        err_type,
        if path_str.is_empty() {
            "system"
        } else {
            path_str
        },
        err_details
    );

    // Build the data object with all structured fields
    let data = json!({
        "raw_message": message,
        "formatted_message": formatted_message,
        "context": {
            "source": {
                "module": module,
                "file": file,
                "line": line
            },
            "operation": op,
            "path": path_str,
            "error": {
                "type": err_type,
                "details": err_details
            },
            "timestamp": now,
            "direct": true
        }
    });

    // Prepare the structured JSON payload according to BetterStack format
    let payload = json!({
        "level": level,
        "data": data
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

// Backward compatibility function for direct logging
// This will capture the source location of the caller directly
pub fn send_direct_betterstack_log(
    message: &str,
    level: &str,
    operation: Option<&str>,
    path: Option<&str>,
    error_type: Option<&str>,
    details: Option<&str>,
) -> Result<()> {
    // Call the macro which will capture file, line, and module information from the call site
    send_direct_betterstack_log!(message, level, operation, path, error_type, details)
}

/// Shutdown the logger gracefully
/// This should be called before application exit to ensure all logs are sent
pub fn shutdown_logger() {
    // Ensure we flush any pending logs
    log::logger().flush();

    // Give background thread time to send remaining logs
    thread::sleep(Duration::from_millis(200));
}
