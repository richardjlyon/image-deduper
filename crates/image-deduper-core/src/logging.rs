use crate::error::{Error, Result};
use log::{error, info, LevelFilter};
use std::path::Path;
// For file-based logging with rotation
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

/// Initialize the logger with timestamp, log level, and module path
/// Logs will be written to file only to avoid interfering with progress bars
pub fn init_logger(log_dir: &str) -> Result<()> {
    // Create log directory if it doesn't exist
    std::fs::create_dir_all(log_dir)?;

    let log_file_path = format!("{}/dedup.log", log_dir);
    let archived_logs_pattern = format!("{}/dedup.{{}}.log", log_dir);

    // Set up the rotating file appender - rotate at 10MB
    let file_trigger = SizeTrigger::new(10 * 1024 * 1024); // 10MB

    // Keep 5 archived log files
    let file_roller = FixedWindowRoller::builder()
        .build(&archived_logs_pattern, 5)
        .map_err(|e| Error::Unknown(format!("Failed to create log roller: {}", e)))?;

    // Combine trigger and roller into a compound policy
    let compound_policy = CompoundPolicy::new(Box::new(file_trigger), Box::new(file_roller));

    // Create the rolling file appender
    let rolling_file = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} [{l}] [{M}:{L}] - {m}{n}",
        )))
        .build(log_file_path.clone(), Box::new(compound_policy))
        .map_err(|e| Error::Unknown(format!("Failed to create log appender: {}", e)))?;

    // Get log level from environment or default to info
    let env_filter = std::env::var("DEDUP_LOG").unwrap_or_else(|_| "debug".to_string());
    let level = env_filter
        .parse::<LevelFilter>()
        .unwrap_or(LevelFilter::Info);

    // Build the logger configuration - file only, no console output
    let config = Config::builder()
        .appender(Appender::builder().build("file", Box::new(rolling_file)))
        .build(Root::builder().appender("file").build(level))
        .map_err(|e| Error::Unknown(format!("Failed to build log config: {}", e)))?;

    // Use the configured logger
    log4rs::init_config(config)
        .map_err(|e| Error::Unknown(format!("Failed to initialize log4rs: {}", e)))?;

    // Set the max level for the log crate as well
    log::set_max_level(level);

    info!("Image deduplication application started");
    info!("Logging to file: {} with level: {}", log_file_path, level);
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

    // Create a direct file log entry as well to ensure something is recorded
    let log_msg = format!(
        "{} - FS CHANGE - Operation: {}, Path: {}{}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        operation,
        path.display(),
        if details_str.is_empty() {
            "".to_string()
        } else {
            format!(", Details: {}", details_str)
        }
    );

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("./direct_logs/backup_log.txt")
    {
        use std::io::Write;
        let _ = file.write_all(log_msg.as_bytes());
    }
}
