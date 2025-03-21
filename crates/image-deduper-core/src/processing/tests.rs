#[cfg(test)]
mod tests {
    use std::sync::Once;
    static INIT: Once = Once::new();
    use crate::logging;

    fn setup() {
        INIT.call_once(|| {
            logging::init_logger(false).ok();
        });
    }

    // "happy path" cHash Tests
    mod valid_chash_tests {
        use crate::processing::core::compute_cryptographic;
        use crate::test_utils::get_test_data_path;

        macro_rules! test_image_hash {
            ($test_name:ident, $format:expr, $filename:expr, $expected_hash:expr) => {
                #[test]
                fn $test_name() {
                    let img_path = get_test_data_path(concat!($format, "/valid"), $filename);
                    let result = compute_cryptographic(&img_path).unwrap().to_string();
                    assert_eq!(result, $expected_hash);
                }
            };
        }

        test_image_hash!(
            test_jpeg,
            "jpeg",
            "IMG-2624x3636_original.jpeg",
            "0adc4958a3bfdb3ab5d3d747aa5982045dae251667e237e8dd8d38f9778d92cc"
        );

        test_image_hash!(
            test_png,
            "png",
            "IMG-2624x3636_original.png",
            "9438c3556933007eae38ba1566a3764e506408eab4503e279ba6066d286ae095"
        );

        test_image_hash!(
            test_heic,
            "heic",
            "IMG-2624x3636_original.heic",
            "96c6c4bba9a39818c2645ba48a2530d71122c446c173eab88c46120e38d769b4"
        );

        test_image_hash!(
            test_tiff,
            "tiff",
            "IMG-2624x3636_original.tif",
            "fbc39cce9c3c868b94c923212733fbfae9d84a6ba96f90c32ff3afcfc51feaff"
        );

        test_image_hash!(
            test_raw,
            "raw",
            "2025-01-14-1.raf",
            "e8a6167126c826a6c899cc46e7abf20716d757dcdf102e5f05ce1c36861d11c0"
        );
    }

    // "happy path" pHash Tests
    mod valid_phash_tests {
        use super::*;
        use crate::processing::file_processing::phash_from_file;
        use crate::{processing::types::PHash, test_utils::get_test_data_path};

        macro_rules! test_image_phash {
            ($test_name:ident, $format:expr, $filename:expr, $expected_hash:expr) => {
                #[test]
                fn $test_name() {
                    setup();
                    let img_path = get_test_data_path(concat!($format, "/valid"), $filename);
                    let result = phash_from_file(&img_path).unwrap();
                    match result {
                        PHash::Standard(hash) => assert_eq!(hash, $expected_hash),
                        _ => panic!("Expected Standard PHash variant"),
                    }
                }
            };
        }

        test_image_phash!(
            test_jpeg_phash,
            "jpeg",
            "IMG-2624x3636_original.jpeg",
            0x70000111C7FFFFFF
        );

        test_image_phash!(
            test_png_phash,
            "png",
            "IMG-2624x3636_original.png",
            0x70000111C7FFFFFF
        );

        test_image_phash!(
            test_heic_phash,
            "heic",
            "IMG-2624x3636_original.heic",
            0x70000111C7FFFFFF
        );

        test_image_phash!(
            test_tiff_phash,
            "tiff",
            "IMG-2624x3636_original.tif",
            0x70000111C7FFFFFF
        );

        // test_image_phash!(
        //     test_raw_phash,
        //     "raw",
        //     "2025-01-14-1.raf",
        //     0x70008111c7ffffff
        // );
    }

    // Group 3: pHash distance tests
    mod phash_distance_tests {
        use crate::processing::file_processing::phash_from_file;
        use crate::test_utils::get_test_data_path;

        #[test]
        fn test_compressed() {
            let img1 = get_test_data_path("jpeg/valid", "IMG-2624x3636_original.jpeg");
            let img2 = get_test_data_path("jpeg/valid", "IMG-2624x3636_compress_50.jpeg");
            let img3 = get_test_data_path("jpeg/valid", "IMG-2624x3636_compress_10.jpeg");

            let phash_img1 = phash_from_file(&img1).unwrap();
            let phash_img2 = phash_from_file(&img2).unwrap();
            let phash_img3 = phash_from_file(&img3).unwrap();

            assert_eq!(phash_img1.distance(&phash_img2), 0);
            assert_eq!(phash_img1.distance(&phash_img3), 0);
        }

        #[test]
        fn test_scaled() {
            let img1 = get_test_data_path("jpeg/valid", "IMG-2624x3636_original.jpeg");
            let img2 = get_test_data_path("jpeg/valid", "IMG-2624x3636_resize_866_1200.jpeg");
            let img3 = get_test_data_path("jpeg/valid", "IMG-2624x3636_resize_577_800.jpeg");
            let img4 = get_test_data_path("jpeg/valid", "IMG-2624x3636_resize_289_400.jpeg");
            let img5 = get_test_data_path("jpeg/valid", "IMG-2624x3636_resize_144_200.jpeg");

            let phash_img1 = phash_from_file(&img1).unwrap();
            let phash_img2 = phash_from_file(&img2).unwrap();
            let phash_img3 = phash_from_file(&img3).unwrap();
            let phash_img4 = phash_from_file(&img4).unwrap();
            let phash_img5 = phash_from_file(&img5).unwrap();

            assert_eq!(phash_img1.distance(&phash_img2), 0);
            assert_eq!(phash_img1.distance(&phash_img3), 0);
            assert_eq!(phash_img1.distance(&phash_img4), 1);
            assert_eq!(phash_img1.distance(&phash_img5), 3);
        }

        #[test]
        fn test_rotated() {
            let img1 = get_test_data_path("jpeg/valid", "IMG-2624x3636_original.jpeg");
            let img2 = get_test_data_path("jpeg/valid", "IMG-2624x3636_rotate_5.jpeg");
            let img3 = get_test_data_path("jpeg/valid", "IMG-2624x3636_rotate_10.jpeg");

            let phash_img1 = phash_from_file(&img1).unwrap();
            let phash_img2 = phash_from_file(&img2).unwrap();
            let phash_img3 = phash_from_file(&img3).unwrap();

            let distance1 = phash_img1.distance(&phash_img2);
            let distance2 = phash_img1.distance(&phash_img3);

            assert!(distance2 > distance1);
        }
    }

    mod problematic_handling {

        #[test]
        fn raw_has_jpeg() {}
    }
}
