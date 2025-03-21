//! Computes the cryptographic hash of a file using the Blake3 algorithm.
//!
//! This function reads the contents of the specified file in chunks and processes
//! it using the Blake3 hashing algorithm to produce a cryptographic hash.
//!
//! # Arguments
//!
//! * `path` - A reference to a path that specifies the location of the file to be hashed.
//!
//! # Returns
//!
//! This function returns a `Result` containing the Blake3 hash of the file. If an error
//! occurs during file reading or hashing, the error is returned.
//!
//! # Errors
//!
//! This function will return an error if the file cannot be opened or read.
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
