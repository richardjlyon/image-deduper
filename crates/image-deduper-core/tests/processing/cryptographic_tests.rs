#[test]
fn test_cryptographic_hash() {
    use crate::common::TestImageRegistry;
    use image_deduper_core::processing::compute_cryptographic;

    let registry = TestImageRegistry::new();

    let img_path = registry
        .get_image_path(
            "jpg",           // file_type
            "IMG-2624x3636", // image_name
            "original",      // transformation
            None,            // transformation_parameter
            None,            // index
        )
        .unwrap();

    // Compute the hash
    let result = compute_cryptographic(&img_path).unwrap();
    // Computed with the b3sum utility
    let expected_hash = "0adc4958a3bfdb3ab5d3d747aa5982045dae251667e237e8dd8d38f9778d92cc";

    // Verify the hash matches
    assert_eq!(result.to_string(), expected_hash);
}
