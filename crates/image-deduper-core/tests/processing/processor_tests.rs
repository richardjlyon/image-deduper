use std::path::PathBuf;

use image_deduper_core::processing::process_images;

#[test]
fn test_process_images() {
    let images = vec![
        PathBuf::from("/Users/richardlyon/Code/image-deduper/crates/image-deduper-core/tests/test_images/andromeda.jpg"),
        PathBuf::from("/Users/richardlyon/Code/image-deduper/crates/image-deduper-core/tests/test_images/andromeda.jpg"),
    ];
    let results = process_images(&images);
    assert_eq!(results.len(), 2);

    let expected_hash = "63dd202f81edc4f5d1929361dc84c19aa5a5cb1f80fd4eb2c654e9a335e7db88";
    assert_eq!(results[0].cryptographic.to_string(), expected_hash);
    assert_eq!(results[1].cryptographic.to_string(), expected_hash);
}
