# Image Deduper

A safe and efficient tool for finding and managing duplicate images, with special support for HEIC files.

## Features

- **Safe Operations**: Multiple verification steps ensure no data loss
- **Format Support**: Works with JPEG, PNG, TIFF, and HEIC formats
- **Smart Detection**: Finds both exact duplicates and visually similar images
- **Flexible Actions**: Move, delete, or replace duplicates with symlinks
- **Performance**: Parallel processing for faster operation on large collections

## Installation

### From Source

```bash
git clone https://github.com/yourusername/image-deduper.git
cd image-deduper
cargo install --path .
```

## Usage

### Basic Usage

```bash
# Find duplicates in a directory (dry run by default)
image-deduper scan ~/Pictures

# Find duplicates across multiple directories
image-deduper scan ~/Pictures ~/Downloads/Images

# Actually perform deduplication (move duplicates to ./duplicates)
image-deduper scan ~/Pictures --no-dry-run

# Delete duplicates instead of moving them
image-deduper scan ~/Pictures --no-dry-run --delete

# Replace duplicates with symbolic links to originals
image-deduper scan ~/Pictures --no-dry-run --symlinks
```

### Advanced Options

```bash
# Generate default configuration file
image-deduper generate-config

# Use custom configuration file
image-deduper scan ~/Pictures --config my-config.json
```

## Configuration

The tool can be configured using a JSON configuration file. Generate a default one with:

```bash
image-deduper generate-config
```

### Configuration Options

```json
{
  "dry_run": true,
  "duplicates_dir": "duplicates",
  "delete_duplicates": false,
  "create_symlinks": false,
  "phash_threshold": 90,
  "generate_thumbnails": true,
  "backup_dir": "backup",
  "max_depth": null,
  "threads": 0,
  "prioritization": [
    "HighestResolution",
    "LargestFileSize",
    "OldestCreationDate"
  ],
  "use_database": true,
  "database_path": "image-deduper.db",
  "log_level": "Info"
}
```

## For Developers

### Library Usage

This tool is also available as a library:

```rust
use image_deduper_core::{Config, ImageDeduper};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the deduplicator
    let config = Config::default();

    // Create deduplicator instance
    let deduper = ImageDeduper::new(config);

    // Scan directories
    let directories = vec!["~/Pictures".into()];
    deduper.run(&directories, false)?;  // false = don't force rescan

    Ok(())
}
```

## License

This project is licensed under either:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
