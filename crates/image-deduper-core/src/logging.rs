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
        // Create a simple writer that implements both std::io::Write and log4rs::encode::Write
        struct SimpleWriter(Vec<u8>);

        impl io::Write for SimpleWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.extend_from_slice(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        // Implement log4rs::encode::Write trait by delegating to std::io::Write
        impl log4rs::encode::Write for SimpleWriter {}

        // Create our writer
        let mut writer = SimpleWriter(Vec::new());

        // Try to encode the log message
        if let Err(_) = self.encoder.encode(&mut writer, record) {
            return None;
        }

        // Convert buffer to string
        match String::from_utf8(writer.0) {
            Ok(log_text) => {
                // Create the current timestamp in UTC
                let now = chrono::Utc::now().format("%Y-%m-%d %T UTC").to_string();

                // Construct JSON payload for BetterStack
                let payload = json!({
                    "dt": now,
                    "message": log_text.trim(),
                    "level": record.level().to_string(),
                    "target": record.target(),
                    "file": record.file(),
                    "line": record.line(),
                });

                Some(payload.to_string())
            }
            Err(_) => None,
        }
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
    error!(
        "File operation failed - Operation: {}, Path: {}, Error: {}",
        operation,
        path.display(),
        error
    );
}

/// Log hash computation error
pub fn log_hash_error<E: std::fmt::Display>(path: &Path, error: E) {
    error!(
        "Hash computation failed - Path: {}, Error: {}",
        path.display(),
        error
    );
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
pub fn send_direct_betterstack_log(message: &str, level: &str) -> Result<()> {
    // Create a timestamp in UTC
    let now = chrono::Utc::now().format("%Y-%m-%d %T UTC").to_string();

    // Prepare the JSON payload
    let payload = json!({
        "dt": now,
        "message": message,
        "level": level,
        "direct": true,
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
