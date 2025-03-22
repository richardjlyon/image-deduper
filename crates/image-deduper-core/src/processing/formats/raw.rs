use std::path::Path;

use log::info;

use crate::processing::{calculate_phash, types::PHash};

/// Process a RAW file
pub fn process_raw_image<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    info!("Processing RAW image");

    // Try to directly open the TIFF file
    let path_ref = path.as_ref();
    match image::open(path_ref) {
        Ok(img) => {
            // Standard processing
            Ok(calculate_phash(&img))
        }
        Err(e) => Err(e),
    }
}
