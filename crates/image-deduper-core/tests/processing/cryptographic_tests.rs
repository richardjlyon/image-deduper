use std::path::Path;

use image_deduper_core::processing::compute_cryptographic;

#[test]
fn test_cryptographic_hash() {
    let file_path = Path::new("/Users/richardlyon/Code/image-deduper/crates/image-deduper-core/tests/test_images/andromeda.jpg");

    // Compute the hash
    let result = compute_cryptographic(&file_path).unwrap();
    // Computed with the b3sum utility
    let expected_hash = "63dd202f81edc4f5d1929361dc84c19aa5a5cb1f80fd4eb2c654e9a335e7db88";

    // Verify the hash matches
    assert_eq!(result.to_string(), expected_hash);
}
