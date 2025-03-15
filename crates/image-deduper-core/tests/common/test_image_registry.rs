use image::DynamicImage;
use once_cell::sync::Lazy;
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a test image with parsed metadata
#[derive(Debug, Clone)]
pub struct TestImageInfo {
    /// Full path to the image file
    pub path: PathBuf,
    /// Base name of the image (e.g., "IMG-2624x3636")
    pub image_name: String,
    /// Type of transformation applied (e.g., "resize", "blur", "original")
    pub transformation: String,
    /// Parameter of the transformation (e.g., "800x600", "1.5")
    pub transformation_parameter: Option<String>,
    /// Index number of the image
    pub file_type: String,
}

/// Registry for managing and retrieving test images
#[derive(Default)]
pub struct TestImageRegistry {
    /// Collection of all test images organized by their properties
    images: Vec<TestImageInfo>,
}

/// Global test image registry that's initialized once
#[allow(dead_code)]
pub static TEST_IMAGES: Lazy<TestImageRegistry> = Lazy::new(TestImageRegistry::new);

impl TestImageRegistry {
    /// Create a new registry by scanning the test images directory
    pub fn new() -> Self {
        let base_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_images");
        let mut images = Vec::new();

        if base_dir.exists() {
            Self::scan_directory(&base_dir, &mut images)
                .expect("Failed to scan test images directory");
        } else {
            eprintln!(
                "Warning: Test images directory not found at: {}",
                base_dir.display()
            );
        }

        Self { images }
    }

    /// Recursively scan a directory for image files
    fn scan_directory(dir: &Path, images: &mut Vec<TestImageInfo>) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    Self::scan_directory(&path, images)?;
                } else if Self::is_image_file(&path) {
                    if let Some(image_info) = Self::parse_image_file(&path) {
                        images.push(image_info);
                    }
                }
            }
        }
        Ok(())
    }

    /// Check if a file is an image based on its extension
    fn is_image_file(path: &Path) -> bool {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => matches!(
                ext.to_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "tiff" | "tif" | "heic" | "webp"
            ),
            None => false,
        }
    }

    /// Parse the filename into structured metadata
    fn parse_image_file(path: &Path) -> Option<TestImageInfo> {
        let file_stem = path.file_stem()?.to_str()?;
        let file_type = path.extension()?.to_str()?.to_lowercase();

        // Split the filename by underscores
        let parts: Vec<&str> = file_stem.split('_').collect();

        if parts.is_empty() {
            return None;
        }

        let image_name = parts[0].to_string();

        // Handle different filename patterns
        if parts.len() == 2 && parts[1] == "original" {
            // Pattern: ImageName_original.ext
            return Some(TestImageInfo {
                path: path.to_path_buf(),
                image_name,
                transformation: "original".to_string(),
                transformation_parameter: None,
                file_type,
            });
        } else if parts.len() >= 3 {
            let transformation = parts[1].to_string();

            // Check if there's a transformation parameter
            if parts.len() == 3 {
                // Pattern: ImageName_Transformation_Index.ext

                return Some(TestImageInfo {
                    path: path.to_path_buf(),
                    image_name,
                    transformation,
                    transformation_parameter: None,
                    file_type,
                });
            } else if parts.len() >= 4 {
                // Pattern: ImageName_Transformation_Parameter_Index.ext
                let transformation_parameter = Some(parts[2].to_string());

                return Some(TestImageInfo {
                    path: path.to_path_buf(),
                    image_name,
                    transformation,
                    transformation_parameter,
                    file_type,
                });
            }
        }

        None
    }

    /// Get all images in the registry
    pub fn _get_all_images(&self) -> &[TestImageInfo] {
        &self.images
    }

    /// Find an image by its properties
    pub fn find_image(
        &self,
        file_type: &str,
        image_name: &str,
        transformation: &str,
        transformation_parameter: Option<&str>,
    ) -> Option<&TestImageInfo> {
        self.images.iter().find(|img| {
            img.file_type == file_type
                && img.image_name == image_name
                && img.transformation == transformation
                && match (transformation_parameter, &img.transformation_parameter) {
                    (Some(p1), Some(p2)) => p1 == p2,
                    (None, None) => true,
                    _ => false,
                }
        })
    }

    /// Find all images matching partial criteria
    pub fn find_images(
        &self,
        file_type: Option<&str>,
        image_name: Option<&str>,
        transformation: Option<&str>,
        transformation_parameter: Option<&str>,
    ) -> Vec<&TestImageInfo> {
        self.images
            .iter()
            .filter(|img| {
                file_type.map_or(true, |ft| img.file_type == ft)
                    && image_name.map_or(true, |name| img.image_name == name)
                    && transformation.map_or(true, |trans| img.transformation == trans)
                    && transformation_parameter.map_or(true, |param| {
                        img.transformation_parameter
                            .as_ref()
                            .is_some_and(|p| p == param)
                    })
            })
            .collect()
    }

    /// Get the path to an image by its properties
    pub fn get_image_path(
        &self,
        file_type: &str,
        image_name: &str,
        transformation: &str,
        transformation_parameter: Option<&str>,
    ) -> Option<PathBuf> {
        self.find_image(
            file_type,
            image_name,
            transformation,
            transformation_parameter,
        )
        .map(|info| info.path.clone())
    }

    /// Load an image by its properties
    pub fn load_image(
        &self,
        file_type: &str,
        image_name: &str,
        transformation: &str,
        transformation_parameter: Option<&str>,
    ) -> Option<DynamicImage> {
        self.get_image_path(
            file_type,
            image_name,
            transformation,
            transformation_parameter,
        )
        .and_then(|path| image::open(&path).ok())
    }

    /// Get all available image names
    pub fn get_image_names(&self) -> Vec<String> {
        self.images
            .iter()
            .map(|img| img.image_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Get all available transformations
    pub fn get_transformations(&self) -> Vec<String> {
        self.images
            .iter()
            .map(|img| img.transformation.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Debug: Print all registered images
    pub fn print_registry(&self) {
        println!("TestImageRegistry contains {} images:", self.images.len());
        for (i, img) in self.images.iter().enumerate() {
            println!("{}. {} ({}):", i + 1, img.path.display(), img.file_type);
            println!("   - Image Name: {}", img.image_name);
            println!("   - Transformation: {}", img.transformation);
            if let Some(param) = &img.transformation_parameter {
                println!("   - Parameter: {}", param);
            }

            println!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_image_file() {
        // Test original image format
        let path = PathBuf::from("/path/to/IMG-2624x3636_original.jpg");
        let info = TestImageRegistry::parse_image_file(&path).unwrap();
        assert_eq!(info.image_name, "IMG-2624x3636");
        assert_eq!(info.transformation, "original");
        assert_eq!(info.transformation_parameter, None);
        assert_eq!(info.file_type, "jpg");

        // Test transformation with parameter and index
        let path = PathBuf::from("/path/to/IMG-2624x3636_resize_800x600_1.jpg");
        let info = TestImageRegistry::parse_image_file(&path).unwrap();
        assert_eq!(info.image_name, "IMG-2624x3636");
        assert_eq!(info.transformation, "resize");
        assert_eq!(info.transformation_parameter, Some("800x600".to_string()));
        assert_eq!(info.file_type, "jpg");

        // Test transformation without parameter
        let path = PathBuf::from("/path/to/IMG-2624x3636_crop_9.jpg");
        let info = TestImageRegistry::parse_image_file(&path).unwrap();
        assert_eq!(info.image_name, "IMG-2624x3636");
        assert_eq!(info.transformation, "crop");
        assert_eq!(info.transformation_parameter, None);
        assert_eq!(info.file_type, "jpg");
    }
}
