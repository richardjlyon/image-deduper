use blake3::Hash as Blake3Hash;
use rayon::prelude::*;
use std::path::PathBuf;

use super::compute_cryptographic;

#[derive(Debug, Clone)]
pub struct ImageHashResult {
    /// Path to the image file
    pub path: PathBuf,
    /// Blake3 cryptographic hash of the file contents
    pub cryptographic: Blake3Hash,
}

/// Process a list of images and compute their hashes
pub fn process_images(images: &[PathBuf]) -> Vec<ImageHashResult> {
    images
        .par_iter()
        .map(|path| {
            let blake3 = compute_cryptographic(path).unwrap();
            ImageHashResult {
                path: path.clone(),
                cryptographic: blake3, // Other hashes...
            }
        })
        .collect()
}
