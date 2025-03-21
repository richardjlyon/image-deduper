use image::{DynamicImage, GenericImageView};

use super::types::PHash;

/// Core hash calculation algorithms
///

/// Calculate a standard 64-bit perceptual hash for an image (8x8 grid)
#[inline]
pub fn calculate_phash(img: &DynamicImage) -> PHash {
    // Use fastest filter for downscaling
    let small = img.resize_exact(8, 8, image::imageops::FilterType::Nearest);

    // Extract grayscale values directly, avoiding full grayscale conversion
    // Grayscale formula: 0.299*R + 0.587*G + 0.114*B
    let mut pixels = [0.0; 64];

    for y in 0..8 {
        for x in 0..8 {
            let pixel = small.get_pixel(x, y);
            let gray_value =
                0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
            pixels[(y as usize) * 8 + (x as usize)] = gray_value;
        }
    }

    // Use a partial sum approach to calculate the mean
    let mut sum = 0.0;
    for &p in &pixels {
        sum += p;
    }
    let mean = sum / 64.0;

    // Optimized hash calculation using bit manipulation
    let mut hash: u64 = 0;

    // Process 8 comparisons at once in each loop iteration
    for chunk in 0..8 {
        let base = chunk * 8;

        // Build an 8-bit chunk
        let mut byte: u8 = 0;
        if pixels[base] > mean {
            byte |= 1 << 0;
        }
        if pixels[base + 1] > mean {
            byte |= 1 << 1;
        }
        if pixels[base + 2] > mean {
            byte |= 1 << 2;
        }
        if pixels[base + 3] > mean {
            byte |= 1 << 3;
        }
        if pixels[base + 4] > mean {
            byte |= 1 << 4;
        }
        if pixels[base + 5] > mean {
            byte |= 1 << 5;
        }
        if pixels[base + 6] > mean {
            byte |= 1 << 6;
        }
        if pixels[base + 7] > mean {
            byte |= 1 << 7;
        }

        // Place the byte in the appropriate position in the hash
        hash |= (byte as u64) << (chunk * 8);
    }

    PHash::Standard(hash)
}

/// Calculate an enhanced 1024-bit perceptual hash for an image (32x32 grid)
/// For higher quality discrimination and better GPU acceleration potential
#[inline]
pub fn calculate_enhanced_phash(img: &DynamicImage) -> PHash {
    // Use fastest filter for downscaling to 32x32
    let small = img.resize_exact(32, 32, image::imageops::FilterType::Nearest);

    // Extract grayscale values directly, avoiding full grayscale conversion
    // Grayscale formula: 0.299*R + 0.587*G + 0.114*B
    let mut pixels = [0.0; 1024]; // 32x32 = 1024 pixels

    for y in 0..32 {
        for x in 0..32 {
            let pixel = small.get_pixel(x, y);
            let gray_value =
                0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
            pixels[(y as usize) * 32 + (x as usize)] = gray_value;
        }
    }

    // Calculate mean of all pixels
    let mut sum = 0.0;
    for &p in &pixels {
        sum += p;
    }
    let mean = sum / 1024.0;

    // Create an array of 16 u64 values (1024 bits total)
    let mut hash_array = [0u64; 16];

    // Process 64 pixels at a time to fill each u64
    for segment in 0..16 {
        let mut hash: u64 = 0;

        // Each segment processes 64 pixels
        for i in 0..64 {
            let pixel_idx = segment * 64 + i;

            // Set bit if pixel value > mean
            if pixels[pixel_idx] > mean {
                hash |= 1u64 << i;
            }
        }

        hash_array[segment] = hash;
    }

    PHash::Enhanced(hash_array)
}

/// Ultra-fast implementation for when quality can be traded for speed
#[inline]
pub fn ultra_fast_phash(img: &DynamicImage) -> PHash {
    // Work with the original image directly
    let width = img.width();
    let height = img.height();

    // Calculate sampling steps
    let step_x = width.max(8) / 8;
    let step_y = height.max(8) / 8;

    // Sample the image at 64 strategic points
    let mut pixels = [0.0; 64];
    let mut sum = 0.0;

    for y in 0..8 {
        let img_y = (y as u32 * step_y).min(height - 1);
        for x in 0..8 {
            let img_x = (x as u32 * step_x).min(width - 1);

            // Get pixel and convert to grayscale on the fly
            let pixel = img.get_pixel(img_x, img_y);
            let gray = 0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;

            pixels[(y as usize) * 8 + (x as usize)] = gray;
            sum += gray;
        }
    }

    // Calculate mean
    let mean = sum / 64.0;

    // Optimized bit comparisons
    let mut hash: u64 = 0;

    // Unrolled loop for maximum performance
    for (bit_pos, &p) in pixels.iter().enumerate() {
        if p > mean {
            hash |= 1u64 << bit_pos;
        }
    }

    PHash::Standard(hash)
}
