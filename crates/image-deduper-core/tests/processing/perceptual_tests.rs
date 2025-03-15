#[test]
fn test_phash_ne() {
    use super::super::common::test_image_registry::TEST_IMAGES;
    use image_deduper_core::processing::phash_from_file;

    let img1 = TEST_IMAGES
        .get_image_path(
            "jpg",           // file_type
            "IMG-2624x3636", // image_name
            "original",      // transformation
            None,            // transformation_parameter         // index
        )
        .unwrap();

    let img2 = TEST_IMAGES
        .get_image_path(
            "jpg",           // file_type
            "IMG-2667x4000", // image_name
            "original",      // transformation
            None,            // transformation_parameter      // index
        )
        .unwrap();

    let phash_img1 = phash_from_file(&img1).unwrap();
    let phash_img2 = phash_from_file(&img2).unwrap();

    assert_ne!(phash_img1, phash_img2);

    // let result = compute_perceptual(&file_path).unwrap();
}

#[test]
fn test_phash_distance() {
    use super::super::common::test_image_registry::TEST_IMAGES;
    use image_deduper_core::processing::phash_from_file;

    let img1 = TEST_IMAGES
        .get_image_path(
            "jpg",           // file_type
            "IMG-2624x3636", // image_name
            "compress",      // transformation
            Some("20"),      // transformation_parameter     // index
        )
        .unwrap();

    let img2 = TEST_IMAGES
        .get_image_path(
            "jpg",           // file_type
            "IMG-2624x3636", // image_name
            "compress",      // transformation
            Some("90"),      // transformation_parameter     // index
        )
        .unwrap();

    let phash_img1 = phash_from_file(&img1).unwrap();
    let phash_img2 = phash_from_file(&img2).unwrap();

    let _distance = phash_img1.distance(&phash_img2);
    println!("phash_img1: {:?}", phash_img1);
    println!("phash_img2: {:?}", phash_img2);
    // assert_eq!(distance, 1);
}
