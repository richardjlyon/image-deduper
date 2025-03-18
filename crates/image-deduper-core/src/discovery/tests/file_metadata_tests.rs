use std::path::PathBuf;

use crate::discovery::discover_images;
use crate::discovery::tests::test_utils::get_real_images_dir;
use crate::types::ImageFormat;
use crate::Config;

/// Test the file metadata extraction function
#[test]
fn test_get_file_metadata() {
    let image_files =
        discover_images(&[PathBuf::from(get_real_images_dir())], &Config::default()).unwrap();
    let image = image_files[0].clone();
    assert_eq!(image.size, 8407);
    assert_eq!(image.format, ImageFormat::Jpeg);
}
