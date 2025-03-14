// Import common/image_variants.rs directly
#[path = "./common/image_variants.rs"]
mod _image_variants;
use _image_variants::ImageVariant;

#[cfg(test)]
mod test_image_variants {
    use super::ImageVariant;
    use std::path::PathBuf;

    #[test]
    fn test_constructor() {
        let base_image_path = PathBuf::from(
            "/Users/richardlyon/Code/image-deduper/crates/image-deduper-core/tests/andromeda.jpg",
        );
        let output_dir = PathBuf::from(
            "/Users/richardlyon/Code/image-deduper/crates/image-deduper-core/tests/output",
        );

        // With the new implementation, we can pass PathBuf directly
        let _variant = ImageVariant::new(base_image_path, output_dir);

        // Now you can use variant to perform operations on the image
    }
}
