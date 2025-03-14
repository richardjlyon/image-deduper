#!/bin/bash
# Script to create test images for image-deduper-core tests

# Ensure the script is run from the project root directory
if [ ! -d "crates/image-deduper-core" ]; then
    echo "Error: This script must be run from the project root directory"
    exit 1
fi

# Create the tools directory if it doesn't exist
mkdir -p tools

# Run the test image creator
echo "Creating test images..."
cargo run --bin create-test-images

echo "Test images created successfully!"
echo "You can now run the tests that depend on these images."
