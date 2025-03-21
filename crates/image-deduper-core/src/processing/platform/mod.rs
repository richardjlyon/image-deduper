// Import platform-specific modules conditionally
#[cfg(target_os = "macos")]
pub mod macos;

// Import common platform module
pub mod common;

// Re-export based on platform
#[cfg(not(target_os = "macos"))]
pub use self::common::*;
#[cfg(target_os = "macos")]
pub use self::macos::*;
