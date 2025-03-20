use super::perceptual_hash::PHash;
use blake3::Hash as Blake3Hash;
use std::path::PathBuf;

/// Result of processing a single image
#[derive(Debug, Clone)]
pub struct ImageHashResult {
    /// Path to the image file
    pub path: PathBuf,
    /// Blake3 cryptographic hash of the file contents
    pub cryptographic: Blake3Hash,
    /// Perceptual hash of the image
    pub perceptual: PHash,
}
