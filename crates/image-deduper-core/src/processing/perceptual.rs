use image::{imageops, DynamicImage, GenericImageView, Pixel};
use ndarray::Array2;
use rustdct::DctPlanner;

/// Functions for processing images to compute perceptual hashes
use crate::error::Result;
use std::path::Path;

/// A perceptual hash represented as a 64-bit value
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PHash(pub u64);

impl PHash {
    /// Calculate the Hamming distance between two perceptual hashes
    ///
    /// This represents the number of bit positions that differ between the two hashes,
    /// which serves as a metric of perceptual difference between the images.
    pub fn distance(&self, other: &PHash) -> u32 {
        (self.0 ^ other.0).count_ones()
    }

    /// Check if two images are perceptually similar based on a threshold
    pub fn is_similar(&self, other: &PHash, threshold: u32) -> bool {
        self.distance(other) <= threshold
    }
}

/// Calculate a 64-bit perceptual hash for an image - optimized version
pub fn calculate_phash(img: &DynamicImage) -> PHash {
    // Direct resize to 8x8 grayscale - much faster approach
    // This skips the 32x32 intermediate step and eliminates the need for a full DCT
    let small = img.resize_exact(8, 8, image::imageops::FilterType::Triangle);
    let gray = small.grayscale();

    // Extract pixel values to a flat array
    let mut pixels = [0.0; 64];
    let mut idx = 0;

    for y in 0..8 {
        for x in 0..8 {
            let pixel = gray.get_pixel(x, y);
            pixels[idx] = pixel.0[0] as f32;
            idx += 1;
        }
    }

    // Calculate mean (faster than median and nearly as effective)
    let sum: f32 = pixels.iter().sum();
    let mean = sum / 64.0;

    // Create hash based on comparison to mean
    let mut hash = 0u64;
    for (i, &p) in pixels.iter().enumerate() {
        if p > mean {
            hash |= 1u64 << i;
        }
    }

    PHash(hash)
}

/// A faster implementation using direct grayscale reduction
pub fn fast_phash(img: &DynamicImage) -> PHash {
    // Create a scaled-down grayscale version without any intermediate conversions
    let gray_img = img.to_luma8();

    // Reduction factor calculation
    let width = gray_img.width();
    let height = gray_img.height();
    let scale_w = width.max(1) / 8;
    let scale_h = height.max(1) / 8;

    // Sampling using integral image for speed
    let mut block_values = [0.0; 64];
    let mut idx = 0;

    for y in 0..8 {
        for x in 0..8 {
            // Calculate the average over each 8x8 block using sampling
            let mut sum = 0u32;
            let mut count = 0;

            let start_x = x * scale_w;
            let end_x = (x + 1) * scale_w;
            let start_y = y * scale_h;
            let end_y = (y + 1) * scale_h;

            // Sample the block (taking every nth pixel for speed)
            for sy in (start_y..end_y).step_by(scale_h.max(1) as usize / 2) {
                for sx in (start_x..end_x).step_by(scale_w.max(1) as usize / 2) {
                    if sx < width && sy < height {
                        sum += gray_img.get_pixel(sx, sy).0[0] as u32;
                        count += 1;
                    }
                }
            }

            block_values[idx] = if count > 0 {
                sum as f32 / count as f32
            } else {
                0.0
            };
            idx += 1;
        }
    }

    // Find median using quick select algorithm - O(n) instead of O(n log n)
    // But for simplicity, we'll use mean which is also effective
    let sum: f32 = block_values.iter().sum();
    let mean = sum / 64.0;

    // Create hash based on comparison to mean
    let mut hash = 0u64;
    for (i, &p) in block_values.iter().enumerate() {
        if p > mean {
            hash |= 1u64 << i;
        }
    }

    PHash(hash)
}

/// Calculate a perceptual hash from an image file
pub fn phash_from_file<P: AsRef<Path>>(path: P) -> Result<PHash> {
    // FIXME handle erro
    let img = image::open(path).unwrap();
    Ok(calculate_phash(&img))
}

/// Calculate a perceptual hash from an image file
pub fn phash_from_img(img: &DynamicImage) -> Result<PHash> {
    // FIXME handle erro

    Ok(calculate_phash(&img))
}
