// This example demonstrates how to use the TestImageRegistry from the tests/common directory
// It requires adding the tests/ directory to the module path using the following command:
// RUSTFLAGS="--cfg test" cargo run --example test_image_registry_demo

// First, we need to make the tests directory accessible
#[path = "../tests/mod.rs"]
mod tests;

use tests::common::TestImageRegistry;

fn main() {
    // Initialize the registry
    let registry = TestImageRegistry::new();

    // Print all registered images for debugging
    println!("\n===== All Registered Images =====");
    registry.print_registry();

    // Get all unique image names
    println!("\n===== Available Image Names =====");
    for name in registry.get_image_names() {
        println!("- {}", name);
    }

    // Get all transformation types
    println!("\n===== Available Transformations =====");
    for transform in registry.get_transformations() {
        println!("- {}", transform);
    }

    // Find all jpg images with resize transformation
    println!("\n===== All JPG Images with Resize Transformation =====");
    let resize_images = registry.find_images(
        Some("jpg"),    // file_type
        None,           // image_name (any)
        Some("resize"), // transformation
        None,           // transformation_parameter (any)
    );

    for img in resize_images {
        println!(
            "- {} (Parameter: {}) [Index: {}]",
            img.path.display(),
            img.transformation_parameter.as_deref().unwrap_or("N/A"),
            img.index.map_or("N/A".to_string(), |i| i.to_string())
        );
    }

    // Find a specific image
    println!("\n===== Finding Specific Image =====");
    let specific_image = registry.find_image(
        "jpg",           // file_type
        "IMG-2624x3636", // image_name
        "resize",        // transformation
        Some("800x600"), // transformation_parameter
        Some(1),         // index
    );

    if let Some(img) = specific_image {
        println!("Found image: {}", img.path.display());

        // Try to load the image
        match registry.load_image(
            "jpg",           // file_type
            "IMG-2624x3636", // image_name
            "resize",        // transformation
            Some("800x600"), // transformation_parameter
            Some(1),         // index
        ) {
            Some(img) => println!(
                "Successfully loaded image: {}x{}",
                img.width(),
                img.height()
            ),
            None => println!("Failed to load image"),
        }
    } else {
        println!("Image not found!");
    }
}
