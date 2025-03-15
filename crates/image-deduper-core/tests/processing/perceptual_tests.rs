use image_deduper_core::processing::phash_from_file;

#[test]
fn test_perceptual_hash_ne() {
    use crate::common::TestImageRegistry;

    let registry = TestImageRegistry::new();

    let img1 = registry
        .get_image_path(
            "jpg",           // file_type
            "IMG-2624x3636", // image_name
            "original",      // transformation
            None,            // transformation_parameter
            None,            // index
        )
        .unwrap();

    let img2 = registry
        .get_image_path(
            "jpg",           // file_type
            "IMG-2667x4000", // image_name
            "original",      // transformation
            None,            // transformation_parameter
            None,            // index
        )
        .unwrap();

    let phash_img1 = phash_from_file(&img1).unwrap();
    let phash_img2 = phash_from_file(&img2).unwrap();

    assert_ne!(phash_img1, phash_img2);

    // let result = compute_perceptual(&file_path).unwrap();
}
