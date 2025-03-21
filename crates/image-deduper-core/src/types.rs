use blake3::Hash;
use log::info;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};
use sysinfo::System;

use crate::processing::types::PHash;

/// Supported image formats
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Tiff,
    Heic,
    Raw, // Added RAW format
    Other(String),
}
impl ImageFormat {
    /// Determine format from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => Self::Jpeg,
            "png" => Self::Png,
            "tif" | "tiff" => Self::Tiff,
            "heic" => Self::Heic,
            // RAW format extensions
            "raw" | "dng" | "cr2" | "nef" | "arw" | "orf" | "rw2" | "nrw" | "raf" | "crw"
            | "pef" | "srw" | "x3f" | "rwl" | "3fr" => Self::Raw,
            other => Self::Other(other.to_string()),
        }
    }

    /// Check if format is supported
    pub fn is_supported(&self) -> bool {
        match self {
            Self::Jpeg | Self::Png | Self::Tiff | Self::Heic => true,
            Self::Raw => true, // Mark RAW as supported
            Self::Other(_) => false,
        }
    }
}

/// Representation of an image file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFile {
    /// Full path to the image file
    pub path: PathBuf,

    /// File size in bytes
    pub size: u64,

    /// Last modified timestamp
    pub last_modified: SystemTime,

    /// Image format
    pub format: ImageFormat,

    /// Optional creation time if available
    pub created: Option<SystemTime>,
}

/// Image with extracted metadata and hash information
#[derive(Debug, Clone)]
pub struct ProcessedImage {
    /// Reference to the original image file
    pub original: Arc<ImageFile>,

    /// Perceptual hash for similarity detection
    pub perceptual_hash: PHash,

    /// Cryptographic hash for exact matching
    pub cryptographic_hash: Hash,
    // Image dimensions (width, height) if available
    // pub dimensions: Option<(u32, u32)>,

    // Small thumbnail for visual comparison if enabled
    // pub thumbnail: Option<Vec<u8>>,
}

/// Types of actions that can be performed on duplicates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    /// Move to duplicates directory
    Move,

    /// Delete the file
    Delete,

    /// Replace with symbolic link to original
    Symlink,
}

/// Result of a deduplication action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Type of action performed
    pub action_type: ActionType,

    /// Path of the duplicate file
    pub duplicate_path: PathBuf,

    /// Path of the original file
    pub original_path: PathBuf,

    /// Whether the action was successful
    pub success: bool,

    /// Optional error message if action failed
    pub error: Option<String>,
}

/// Memory usage tracker
pub struct MemoryTracker {
    system: Mutex<System>,
    start_memory: u64,
    peak_memory: AtomicUsize,
    last_check: Mutex<Instant>,
}

impl MemoryTracker {
    /// Create a new memory tracker
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let total_used = system.used_memory();

        Self {
            system: Mutex::new(system),
            start_memory: total_used,
            peak_memory: AtomicUsize::new(total_used as usize),
            last_check: Mutex::new(Instant::now()),
        }
    }

    /// Update memory usage statistics and log if significant changes detected
    pub fn update(&self) -> (u64, u64) {
        let mut system = self.system.lock().unwrap();
        system.refresh_memory();

        let current_used = system.used_memory();
        let usage_diff = if current_used > self.start_memory {
            current_used - self.start_memory
        } else {
            0
        };

        // Update peak memory
        let peak = self.peak_memory.load(std::sync::atomic::Ordering::Relaxed) as u64;
        if current_used > peak {
            self.peak_memory
                .store(current_used as usize, std::sync::atomic::Ordering::Relaxed);
        }

        // Only log if enough time has passed since last check
        let mut last_check = self.last_check.lock().unwrap();
        if last_check.elapsed().as_secs() >= 5 {
            // Log memory usage in MB
            info!(
                "Memory usage: current={}MB, diff=+{}MB, peak={}MB",
                current_used / 1024 / 1024,
                usage_diff / 1024 / 1024,
                self.peak_memory.load(std::sync::atomic::Ordering::Relaxed) as u64 / 1024 / 1024
            );
            *last_check = Instant::now();
        }

        (current_used, usage_diff)
    }

    /// Get peak memory usage in MB
    pub fn peak_mb(&self) -> u64 {
        self.peak_memory.load(std::sync::atomic::Ordering::Relaxed) as u64 / 1024 / 1024
    }

    /// Get current memory usage diff in MB
    pub fn current_diff_mb(&self) -> i64 {
        let mut system = self.system.lock().unwrap();
        system.refresh_memory();

        let current_used = system.used_memory();
        ((current_used as i64) - (self.start_memory as i64)) / 1024 / 1024
    }
}
