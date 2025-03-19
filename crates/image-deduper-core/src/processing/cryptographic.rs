/// Functions for processing images to compute hashes and other similarity metrics
use crate::error::Result;
use blake3::Hash as Blake3Hash;

use std::{fs::File, io::Read, path::Path};

/// Compute the cryptographic hash of a file using the Blake3 algorithm
pub fn compute_cryptographic<P: AsRef<Path>>(path: P) -> Result<Blake3Hash> {
    // Open the file with explicit scope to ensure it's closed promptly
    let hash = {
        let mut file = File::open(&path)?;

        // Create a Blake3 hasher
        let mut hasher = blake3::Hasher::new();

        // Read the file in chunks and update the hasher
        let mut buffer = [0; 8192]; // 8KB buffer
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        // File will be automatically closed when this scope ends
        hasher.finalize()
    };

    Ok(hash)
}
