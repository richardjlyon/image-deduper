use crate::error::{Error, Result};
use crate::processing::calculate_phash;
use crate::processing::types::PHash;
use log::{info, warn};
use std::path::Path;

/// Public function for direct TIFF processing from external modules
/// This provides access to the optimized TIFF handling
pub fn process_tiff_image<P: AsRef<Path>>(path: P) -> Result<PHash> {
    info!("Processing TIFF image");

    // Try to directly open the JPEG file
    let path_ref = path.as_ref();
    match image::open(path_ref) {
        Ok(img) => {
            // Standard processing
            Ok(calculate_phash(&img))
        }
        Err(e) => {
            warn!("Failed to open {} ({})", path_ref.display(), e);
            Err(Error::Image(e))
        }
    }
}
