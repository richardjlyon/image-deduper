use image::DynamicImage;

use crate::processing::{calculate_enhanced_phash, calculate_phash};

use super::types::PHash;

/// Calculate a perceptual hash from an image in memory using standard 8x8 hash
pub fn phash_from_img(img: &DynamicImage) -> PHash {
    calculate_phash(img)
}

/// Calculate an enhanced perceptual hash from an image in memory
pub fn enhanced_phash_from_img(img: &DynamicImage) -> PHash {
    calculate_enhanced_phash(img)
}
