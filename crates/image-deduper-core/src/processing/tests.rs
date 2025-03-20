#[allow(clippy::module_inception)]
#[cfg(test)]
mod tests {
    use crate::processing::perceptual_hash::{phash_from_file, PHash};
    use crate::processing::{compute_cryptographic, process_images};
    use crate::test_utils::test_support::test_image_registry::TEST_IMAGES;
    // Use the test image registry from test_support
    // use crate::test_support::test_image_registry::TEST_IMAGES;

    #[test]
    fn test_jpg_cryptographic_hash() {
        let img_path = TEST_IMAGES
            .get_image_path(
                "jpg",           // file_type
                "IMG-2624x3636", // image_name
                "original",      // transformation
                None,            // transformation_parameter
            )
            .unwrap();

        // Compute the hash
        let result = compute_cryptographic(&img_path).unwrap();
        // Computed with the b3sum utility
        let expected_hash = "0adc4958a3bfdb3ab5d3d747aa5982045dae251667e237e8dd8d38f9778d92cc";

        // Verify the hash matches
        assert_eq!(result.to_string(), expected_hash);
    }

    #[test]
    fn test_heic_cryptographic_hash() {
        let img_path = TEST_IMAGES
            .get_image_path(
                "heic",          // file_type
                "IMG-2624x3636", // image_name
                "original",      // transformation
                None,            // transformation_parameter
            )
            .unwrap();

        // Compute the hash
        let result = compute_cryptographic(&img_path).unwrap();
        // Computed with the b3sum utility
        let expected_hash = "96c6c4bba9a39818c2645ba48a2530d71122c446c173eab88c46120e38d769b4";

        // Verify the hash matches
        assert_eq!(result.to_string(), expected_hash);
    }

    #[test]
    fn test_jpg_phash() {
        let img1 = TEST_IMAGES
            .get_image_path(
                "jpg",           // file_type
                "IMG-2624x3636", // image_name
                "original",      // transformation
                None,            // transformation_parameter
            )
            .unwrap();

        let phash_img1 = phash_from_file(&img1).unwrap();
        let expected_hash = 0x70008111c7ffffff;
        match phash_img1 {
            PHash::Standard(hash) => assert_eq!(hash, expected_hash),
            _ => panic!("Expected Standard PHash variant"),
        };
    }

    #[test]
    fn test_heic_phash() {
        let img1 = TEST_IMAGES
            .get_image_path(
                "heic",          // file_type
                "IMG-2624x3636", // image_name
                "original",      // transformation
                None,            // transformation_parameter
            )
            .unwrap();

        let phash_img1 = phash_from_file(&img1).unwrap();
        let expected_hash = 0x70008111c7ffffff;
        match phash_img1 {
            PHash::Standard(hash) => assert_eq!(hash, expected_hash),
            _ => panic!("Expected Standard PHash variant"),
        };
    }

    #[test]
    fn test_jpgphash_ne() {
        let img1 = TEST_IMAGES
            .get_image_path(
                "jpg",           // file_type
                "IMG-2624x3636", // image_name
                "original",      // transformation
                None,            // transformation_parameter
            )
            .unwrap();

        let img2 = TEST_IMAGES
            .get_image_path(
                "jpg",           // file_type
                "IMG-2667x4000", // image_name
                "original",      // transformation
                None,            // transformation_parameter
            )
            .unwrap();

        let phash_img1 = phash_from_file(&img1).unwrap();
        let phash_img2 = phash_from_file(&img2).unwrap();

        assert_ne!(phash_img1, phash_img2);
    }

    #[test]
    fn test_phash_distance() {
        let img1 = TEST_IMAGES
            .get_image_path(
                "jpg",           // file_type
                "IMG-2624x3636", // image_name
                "original",      // transformation
                None,            // transformation_parameter
            )
            .unwrap();

        let img2 = TEST_IMAGES
            .get_image_path(
                "jpg",           // file_type
                "IMG-2624x3636", // image_name
                "rotate",        // transformation
                Some("5"),       // transformation_parameter
            )
            .unwrap();

        let phash_img1 = phash_from_file(&img1).unwrap();
        let phash_img2 = phash_from_file(&img2).unwrap();
        let distance = phash_img1.distance(&phash_img2);

        println!("phash_img1: {:?}", phash_img1);
        println!("phash_img2: {:?}", phash_img2);
        println!("distance: {:?}", distance);
        // This assertion was commented out in original test
        // assert_eq!(distance, 1);
    }

    #[test]
    fn test_process_images() {
        let img1 = TEST_IMAGES
            .get_image_path(
                "jpg",           // file_type
                "IMG-2624x3636", // image_name
                "original",      // transformation
                None,            // transformation_parameter
            )
            .unwrap();

        let img2 = TEST_IMAGES
            .get_image_path(
                "jpg",           // file_type
                "IMG-2667x4000", // image_name
                "original",      // transformation
                None,            // transformation_parameter
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
}
