use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::types::{ImageFile, ImageFormat};

/// Representation of a stored image with its hashes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredImage {
    /// ID in the database
    pub id: Option<i64>,

    /// Full path to the image file
    pub path: PathBuf,

    /// File size in bytes
    pub size: u64,

    /// Last modified timestamp (stored as unix timestamp)
    pub last_modified: i64,

    /// Image format
    pub format: ImageFormat,

    /// Creation timestamp if available (stored as unix timestamp)
    pub created: Option<i64>,

    /// Cryptographic hash for exact matching
    pub cryptographic_hash: Vec<u8>,

    /// Perceptual hash for similarity detection
    pub perceptual_hash: u64,
}

impl StoredImage {
    /// Create a new stored image from an image file and its hashes
    pub fn new(image: &ImageFile, cryptographic_hash: Vec<u8>, perceptual_hash: u64) -> Self {
        // Convert SystemTime to Unix timestamp (seconds since epoch)
        let last_modified = system_time_to_unix_timestamp(&image.last_modified);
        let created = image.created.as_ref().map(system_time_to_unix_timestamp);

        Self {
            id: None,
            path: image.path.clone(),
            size: image.size,
            last_modified,
            format: image.format.clone(),
            created,
            cryptographic_hash,
            perceptual_hash,
        }
    }

    /// Convert to an ImageFile
    pub fn to_image_file(&self) -> ImageFile {
        ImageFile {
            path: self.path.clone(),
            size: self.size,
            last_modified: unix_timestamp_to_system_time(self.last_modified),
            format: self.format.clone(),
            created: self.created.map(unix_timestamp_to_system_time),
        }
    }

    /// Check if an image file path exists in the filesystem
    pub fn file_exists(&self) -> bool {
        self.path.exists()
    }
}

// Helper function to convert SystemTime to Unix timestamp
fn system_time_to_unix_timestamp(time: &SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// Helper function to convert Unix timestamp to SystemTime
fn unix_timestamp_to_system_time(timestamp: i64) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp.max(0) as u64)
}
