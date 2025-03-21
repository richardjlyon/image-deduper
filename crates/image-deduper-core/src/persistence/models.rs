use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::{processing::types::PHash, ImageFile, ImageFormat};

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
    pub fn new(image: &ImageFile, cryptographic_hash: Vec<u8>, perceptual_hash: PHash) -> Self {
        // Extract the u64 value for storage
        let hash_value = match perceptual_hash {
            PHash::Standard(hash) => hash,
            PHash::Enhanced(array) => array[0], // Store only first 64 bits from enhanced hash
        };
        Self {
            id: None,
            path: image.path.clone(),
            size: image.size,
            last_modified: system_time_to_unix_timestamp(&image.last_modified),
            format: image.format.clone(),
            created: image.created.as_ref().map(system_time_to_unix_timestamp),
            cryptographic_hash,
            perceptual_hash: hash_value,
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
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs();
            if secs > i64::MAX as u64 {
                i64::MAX
            } else {
                secs as i64
            }
        }
        Err(_) => 0,
    }
}

// Helper function to convert Unix timestamp to SystemTime
fn unix_timestamp_to_system_time(timestamp: i64) -> SystemTime {
    if timestamp < 0 {
        SystemTime::UNIX_EPOCH
    } else {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64)
    }
}
