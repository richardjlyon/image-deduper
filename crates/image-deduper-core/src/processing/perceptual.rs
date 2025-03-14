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

/// Calculate a 64-bit perceptual hash for an image
fn calculate_phash(img: &DynamicImage) -> PHash {
    // Step 1: Resize the image to 32x32
    let small = img.resize_exact(32, 32, imageops::FilterType::Lanczos3);

    // Step 2: Convert to grayscale
    let gray = small.grayscale();

    // Step 3: Prepare data for DCT
    let mut image_data = Array2::zeros((32, 32));
    for (x, y, pixel) in gray.pixels() {
        image_data[[y as usize, x as usize]] = pixel.to_luma()[0] as f32;
    }

    // Step 4: Apply DCT
    let mut dct_data = Array2::zeros((32, 32));
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct2(32);

    // Apply DCT to rows
    for i in 0..32 {
        let mut row = image_data.row(i).to_vec();
        dct.process_dct2(&mut row);
        for j in 0..32 {
            dct_data[[i, j]] = row[j];
        }
    }

    // Apply DCT to columns
    for j in 0..32 {
        let mut col: Vec<f32> = (0..32).map(|i| dct_data[[i, j]]).collect();
        dct.process_dct2(&mut col);
        for i in 0..32 {
            dct_data[[i, j]] = col[i];
        }
    }

    // Step 5: Extract the low-frequency 8x8 components
    let mut low_freq = Vec::with_capacity(64);
    for i in 0..8 {
        for j in 0..8 {
            low_freq.push(dct_data[[i, j]]);
        }
    }

    // Step 6: Compute the median of the low frequencies
    let mut sorted_freqs = low_freq.clone();
    sorted_freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted_freqs[32]; // Middle value of 64 elements

    // Step 7: Set bits based on whether each frequency is above the median
    let mut hash = 0u64;
    for (i, &val) in low_freq.iter().enumerate() {
        if val > median {
            hash |= 1 << i;
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
