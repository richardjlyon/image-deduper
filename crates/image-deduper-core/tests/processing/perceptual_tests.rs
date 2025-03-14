use std::path::Path;

use image_deduper_core::processing::phash_from_file;

#[test]
fn test_perceptual_hash() {
    let moon_original =
        Path::new("/Users/richardlyon/Desktop/test-images/original_images/IMG_0009.JPG");
    let henry_original =
        Path::new("/Users/richardlyon/Desktop/test-images/original_images/2024-10-05-1.jpg");

    let phash_original = phash_from_file(&moon_original).unwrap();
    let phash_henry_original = phash_from_file(&henry_original).unwrap();

    assert_ne!(phash_original, phash_henry_original);

    // let result = compute_perceptual(&file_path).unwrap();
}
