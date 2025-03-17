use blake3::Hash;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::processing::perceptual::PHash;

/// Supported image formats
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Tiff,
    Heic,
    Raw,  // Added RAW format
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
            "raw" | "dng" | "cr2" | "nef" | "arw" | "orf" |
            "rw2" | "nrw" | "raf" | "crw" | "pef" | "srw" |
            "x3f" | "rwl" | "3fr" => Self::Raw,
            other => Self::Other(other.to_string()),
        }
    }

    /// Check if format is supported
    pub fn is_supported(&self) -> bool {
        match self {
            Self::Jpeg | Self::Png | Self::Tiff | Self::Heic => true,
            Self::Raw => true,  // Mark RAW as supported
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

// Group of duplicate images
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct DuplicateGroup {
//     /// The image to keep (chosen by prioritization rules)
//     pub original: Arc<ProcessedImage>,

//     /// The duplicate images
//     pub duplicates: Vec<Arc<ProcessedImage>>,

//     /// Similarity score between images (1.0 = exact match)
//     pub similarity_score: f64,
// }

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
