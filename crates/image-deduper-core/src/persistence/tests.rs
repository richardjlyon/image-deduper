#[allow(clippy::module_inception)]
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use tempfile::tempdir;

    use super::super::db::{
        add_image, clear_database, create_database_if_not_exists, get_image_by_path, remove_image,
    };
    use super::super::models::StoredImage;
    use crate::types::{ImageFile, ImageFormat};

    #[test]
    fn test_create_database() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let _db = create_database_if_not_exists(&db_path).unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn test_add_and_get_image() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create a test image file
        let image_path = PathBuf::from("/test/path/image.jpg");
        let now = SystemTime::now();
        let image_file = ImageFile {
            path: image_path.clone(),
            size: 1024,
            last_modified: now,
            format: ImageFormat::Jpeg,
            created: Some(now - Duration::from_secs(3600)),
        };

        // Create test hashes
        let crypto_hash = vec![1, 2, 3, 4, 5];
        let perceptual_hash = 12345678;

        // Create a stored image
        let stored_image = StoredImage::new(&image_file, crypto_hash.clone(), perceptual_hash);

        // Add the image to the database
        let id = add_image(&db_path, &stored_image).unwrap();
        assert!(id > 0);

        // Retrieve the image
        let retrieved = get_image_by_path(&db_path, &image_path).unwrap();

        // Verify properties
        assert_eq!(retrieved.path, image_path);
        assert_eq!(retrieved.size, 1024);
        assert_eq!(retrieved.format, ImageFormat::Jpeg);
        assert_eq!(retrieved.cryptographic_hash, crypto_hash);
        assert_eq!(retrieved.perceptual_hash, perceptual_hash);
    }

    #[test]
    fn test_clear_database() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create a test image file
        let image_path = PathBuf::from("/test/path/image.jpg");
        let now = SystemTime::now();
        let image_file = ImageFile {
            path: image_path.clone(),
            size: 1024,
            last_modified: now,
            format: ImageFormat::Jpeg,
            created: Some(now - Duration::from_secs(3600)),
        };

        // Create a stored image
        let stored_image = StoredImage::new(&image_file, vec![1, 2, 3, 4, 5], 12345678);

        // Add the image to the database
        let _id = add_image(&db_path, &stored_image).unwrap();

        // Clear the database
        let count = clear_database(&db_path).unwrap();
        assert_eq!(count, 1);

        // Try to retrieve the image (should fail)
        let result = get_image_by_path(&db_path, &image_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_image() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create a test image file
        let image_path = PathBuf::from("/test/path/image.jpg");
        let now = SystemTime::now();
        let image_file = ImageFile {
            path: image_path.clone(),
            size: 1024,
            last_modified: now,
            format: ImageFormat::Jpeg,
            created: Some(now - Duration::from_secs(3600)),
        };

        // Create a stored image
        let stored_image = StoredImage::new(&image_file, vec![1, 2, 3, 4, 5], 12345678);

        // Add the image to the database
        let _id = add_image(&db_path, &stored_image).unwrap();

        // Remove the image
        let removed = remove_image(&db_path, &image_path).unwrap();
        assert!(removed);

        // Try to retrieve the image (should fail)
        let result = get_image_by_path(&db_path, &image_path);
        assert!(result.is_err());
    }
}
