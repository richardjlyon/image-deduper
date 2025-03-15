#[test]
fn test_process_images() {
    use super::super::common::test_image_registry::TEST_IMAGES;
    use image_deduper_core::processing::process_images;

    let img1 = TEST_IMAGES
        .get_image_path(
            "jpg",           // file_type
            "IMG-2624x3636", // image_name
            "original",      // transformation
            None,            // transformation_parameter        // index
        )
        .unwrap();

    let img2 = TEST_IMAGES
        .get_image_path(
            "jpg",           // file_type
            "IMG-2667x4000", // image_name
            "original",      // transformation
            None,            // transformation_parameter           // index
        )
        .unwrap();

    let images = vec![img1, img2];
    let results = process_images(&images);

    let expected_hash_1 = "0adc4958a3bfdb3ab5d3d747aa5982045dae251667e237e8dd8d38f9778d92cc";
    let expected_hash_2 = "4ffaeacb536fb65fb32bc75b7cc5a230d1879290aa36ddc9a98fae7b1cf37e0c";

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].cryptographic.to_string(), expected_hash_1);
    assert_eq!(results[1].cryptographic.to_string(), expected_hash_2);
}
