# Test Image Registry

A utility for managing test images in a structured way, parsing filenames to extract metadata about the images.

## Features

- Automatic recursive scanning of the test images directory
- Structured parsing of image filenames to extract metadata:
  - Image name (e.g., "IMG-2624x3636")
  - Transformation type (e.g., "resize", "blur", "original")
  - Transformation parameter (e.g., "800x600", "1.5")
  - Index (numerical identifier)
- Comprehensive search capabilities to find test images by their properties
- Methods to get the path to a test image or load it directly

## Usage

### File Naming Convention

The registry expects test images to follow this naming pattern:

- `[ImageName]_[Transformation]_[Parameter]_[Index].[ext]` (with parameter)
- `[ImageName]_[Transformation]_[Index].[ext]` (without parameter)
- `[ImageName]_original.[ext]` (for original images)

Examples:
- `IMG-2624x3636_resize_800x600_1.jpg`
- `IMG-2624x3636_crop_9.jpg`
- `IMG-2624x3636_original.jpg`

### Basic Usage

```rust
// In test files
use crate::common::{TestImageRegistry, TEST_IMAGES};

// Initialize the registry (scans the test_images directory)
let registry = TestImageRegistry::new();

// Find a specific image
let img_path = registry.get_image_path(
    "jpg",                // file_type
    "IMG-2624x3636",      // image_name
    "resize",             // transformation
    Some("800x600"),      // transformation_parameter
    Some(1)               // index
);

// Load a test image directly
let img = registry.load_image(
    "jpg",                // file_type
    "IMG-2624x3636",      // image_name
    "resize",             // transformation
    Some("800x600"),      // transformation_parameter
    Some(1)               // index
);

// Find all images with a specific transformation
let blur_images = registry.find_images(
    None,                 // file_type (any)
    None,                 // image_name (any)
    Some("blur"),         // transformation
    None,                 // transformation_parameter (any)
);
```

### Global Registry

For convenience, there's a global lazy-loaded registry:

```rust
use crate::common::TEST_IMAGES;

// The registry will be initialized on first use
let img = TEST_IMAGES.load_image(
    "jpg", "IMG-2624x3636", "resize", Some("800x600"), Some(1)
);
```

## Adding New Test Images

When adding new test images, follow the naming convention. The registry will automatically detect and parse them on initialization.

The recommended structure is:
- Organize images by file type: `tests/test_images/jpg/`, `tests/test_images/png/`, etc.
- Group related images in subdirectories: `tests/test_images/jpg/IMG-2624x3636/`
- Name files according to the convention: `IMG-2624x3636_resize_800x600_1.jpg`
