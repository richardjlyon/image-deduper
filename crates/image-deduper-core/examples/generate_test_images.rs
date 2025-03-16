use anyhow::Result;
use image::{ImageBuffer, ImageFormat};
use log::{error, info};
use reqwest::blocking;
use std::fs::{self, create_dir_all};
use std::path::{Path, PathBuf};

const TEST_DATA_DIR: &str = "crates/image-deduper-core/examples/test_data";
const BLUR_AMOUNT: f32 = 3.0;

fn download_image(width: u32, height: u32, id: u32) -> Result<Vec<u8>> {
    let url = format!("https://picsum.photos/seed/{}/{}/{}", id, width, height);
    let response = blocking::get(&url)?;
    Ok(response.bytes()?.to_vec())
}

fn save_image(data: &[u8], filename: &str) -> Result<()> {
    let path = Path::new(TEST_DATA_DIR).join(filename);
    fs::write(&path, data)?;
    info!("Saved image to: {}", path.display());
    Ok(())
}

fn create_blurred_variant(data: &[u8], filename: &str) -> Result<()> {
    let img = image::load_from_memory(data)?;
    let blurred = img.blur(BLUR_AMOUNT);
    let path = Path::new(TEST_DATA_DIR).join(filename);
    blurred.save(&path)?;
    info!("Saved blurred variant to: {}", path.display());
    Ok(())
}

fn generate_test_images() -> Result<()> {
    // Create test data directory if it doesn't exist
    create_dir_all(TEST_DATA_DIR)?;

    // Generate 4 images of 200x300
    for i in 0..4 {
        let data = download_image(200, 300, i)?;
        save_image(&data, &format!("image_200x300_{}.jpg", i))?;
        create_blurred_variant(&data, &format!("image_200x300_{}_blurred.jpg", i))?;
    }

    // Generate 4 images of 300x200
    for i in 4..8 {
        let data = download_image(300, 200, i)?;
        save_image(&data, &format!("image_300x200_{}.jpg", i))?;
        create_blurred_variant(&data, &format!("image_300x200_{}_blurred.jpg", i))?;
    }

    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    info!("Starting test image generation...");

    match generate_test_images() {
        Ok(_) => info!("Successfully generated test images"),
        Err(e) => error!("Error generating test images: {}", e),
    }

    Ok(())
}
