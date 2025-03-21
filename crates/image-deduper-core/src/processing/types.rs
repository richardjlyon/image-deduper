/// PHash enum and core methods
///
use blake3::Hash as Blake3Hash;
use std::path::PathBuf;

/// A perceptual hash that can be either a 64-bit value (8x8) or a 1024-bit value (32x32)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PHash {
    /// Standard 64-bit perceptual hash (8x8 grid)
    Standard(u64),

    /// Enhanced 1024-bit perceptual hash (32x32 grid) for GPU acceleration
    /// Stored as 16 u64 values (16 * 64 = 1024 bits)
    Enhanced([u64; 16]),
}

impl PHash {
    /// Calculate the Hamming distance between two perceptual hashes
    pub fn distance(&self, other: &PHash) -> u32 {
        match (self, other) {
            // Both standard 64-bit hashes
            (PHash::Standard(a), PHash::Standard(b)) => (a ^ b).count_ones(),

            // Both enhanced 1024-bit hashes
            (PHash::Enhanced(a), PHash::Enhanced(b)) => {
                let mut distance = 0;
                for i in 0..16 {
                    distance += (a[i] ^ b[i]).count_ones();
                }
                distance
            }

            // Mixed types - downgrade enhanced to standard for compatibility
            (PHash::Standard(a), PHash::Enhanced(b)) => {
                // Use only the first 64 bits of the enhanced hash
                (a ^ b[0]).count_ones()
            }

            (PHash::Enhanced(a), PHash::Standard(b)) => {
                // Use only the first 64 bits of the enhanced hash
                (a[0] ^ b).count_ones()
            }
        }
    }

    /// Check if two images are perceptually similar based on a threshold
    pub fn is_similar(&self, other: &PHash, threshold: u32) -> bool {
        let distance = self.distance(other);

        // Adjust threshold based on hash type (enhanced hashes need higher thresholds)
        let adjusted_threshold = match (self, other) {
            (PHash::Standard(_), PHash::Standard(_)) => threshold,
            (PHash::Enhanced(_), PHash::Enhanced(_)) => threshold * 16, // Scale by hash size ratio
            _ => threshold, // Mixed types use standard threshold
        };

        distance <= adjusted_threshold
    }

    /// Convert to a standard 64-bit hash if enhanced
    pub fn to_standard(&self) -> PHash {
        match self {
            PHash::Standard(hash) => PHash::Standard(*hash),
            PHash::Enhanced(hash_array) => PHash::Standard(hash_array[0]),
        }
    }

    /// Get the underlying 64-bit hash value (for compatibility)
    pub fn as_u64(&self) -> u64 {
        match self {
            PHash::Standard(hash) => *hash,
            PHash::Enhanced(hash_array) => hash_array[0],
        }
    }
}

/// For cached image loading and processing
pub struct ImageCache {
    buffer_size: usize,
    cache: std::collections::HashMap<String, PHash>,
}

impl ImageCache {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            cache: std::collections::HashMap::with_capacity(buffer_size),
        }
    }

    pub fn get_hash<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
        hash_fn: impl Fn(&P) -> Result<PHash, image::ImageError>,
    ) -> Result<PHash, image::ImageError> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        if let Some(hash) = self.cache.get(&path_str) {
            return Ok(*hash);
        }

        // Use the provided hash function
        let hash = hash_fn(&path)?;

        // Simple LRU-like behavior: clear cache if it's too big
        if self.cache.len() >= self.buffer_size {
            self.cache.clear();
        }

        self.cache.insert(path_str, hash);
        Ok(hash)
    }
}

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
