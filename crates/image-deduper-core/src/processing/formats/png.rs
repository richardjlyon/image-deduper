use std::path::Path;

use log::{info, warn};

use crate::processing::{calculate_phash, types::PHash};

/// Process a JPEG file with corruption recovery
pub fn process_png_image<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    info!("Processing PNG image");

    let path_ref = path.as_ref();
    match image::open(path_ref) {
        Ok(img) => {
            // Standard processing
            Ok(calculate_phash(&img))
        }
        Err(e) => {
            warn!("Failed to open {} ({})", path_ref.display(), e);
            Err(e)
        }
    }
}
