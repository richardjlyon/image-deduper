// Format-specific modules
pub mod heic;
pub mod jpeg;
pub mod png;
pub mod raw;
pub mod tiff;

// Re-export format-specific functions for external use
pub use heic::process_heic_image;
pub use jpeg::process_jpeg_image;
pub use png::process_png_image;
pub use raw::process_raw_image;
pub use tiff::process_tiff_image;
