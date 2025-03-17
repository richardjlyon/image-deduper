//! Database persistence for image deduplication.
//!
//! This module provides functionality for storing and retrieving
//! image information, including paths, cryptographic hashes, and perceptual hashes.
//!
//! # Features
//!
//! - SQLite database storage with optimized performance settings
//! - Storage for image paths, metadata, cryptographic hashes, and perceptual hashes
//! - Efficient indexing on paths and hashes for fast lookups
//! - Database versioning and schema migration support
//! - Comprehensive error handling
//!
//! # Database Schema
//!
//! The database schema consists of a single `images` table with the following structure:
//!
//! ```sql
//! CREATE TABLE images (
//!     id INTEGER PRIMARY KEY,
//!     path TEXT NOT NULL UNIQUE,
//!     size INTEGER NOT NULL,
//!     last_modified INTEGER NOT NULL,
//!     format TEXT NOT NULL,
//!     created INTEGER,
//!     cryptographic_hash BLOB NOT NULL,
//!     perceptual_hash INTEGER NOT NULL
//! );
//! ```
//!
//! The following indexes are created for efficient lookups:
//! - `idx_images_path`: Unique index on the path column
//! - `idx_images_crypto_hash`: Index on the cryptographic hash column
//! - `idx_images_perceptual_hash`: Index on the perceptual hash column
//!
//! # Usage Examples
//!
//! ## Creating a Database
//!
//! ```rust
//! use image_deduper_core::persistence;
//! use std::path::PathBuf;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new database or open existing one
//! let db_path = PathBuf::from("path/to/database.db");
//! let db = persistence::create_database_if_not_exists(&db_path)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Adding an Image
//!
//! ```rust
//! use image_deduper_core::persistence::{StoredImage, add_image};
//! use image_deduper_core::types::ImageFile;
//! use std::path::PathBuf;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let image_file = ImageFile { path: PathBuf::new(), size: 0, last_modified: std::time::SystemTime::now(), format: image_deduper_core::types::ImageFormat::Jpeg, created: None };
//! # let cryptographic_hash = vec![0u8; 32];
//! # use image_deduper_core::processing::perceptual::PHash;
//! # let perceptual_hash = PHash::Standard(0u64);
//! # let db_path = PathBuf::new();
//! // After processing an image and generating hashes
//! let stored_image = StoredImage::new(&image_file, cryptographic_hash, perceptual_hash);
//! let id = add_image(&db_path, &stored_image)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Retrieving Images
//!
//! ```rust
//! use image_deduper_core::persistence::{get_image_by_path, get_image_by_crypto_hash, get_image_by_perceptual_hash};
//! use std::path::PathBuf;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let db_path = PathBuf::new();
//! # let image_path = PathBuf::new();
//! # let crypto_hash = vec![0u8; 32];
//! # let perceptual_hash = 0u64; // u64 is fine here since DB stores the raw value
//! // Get by path
//! let image = get_image_by_path(&db_path, &image_path)?;
//!
//! // Get by cryptographic hash
//! let image = get_image_by_crypto_hash(&db_path, &crypto_hash)?;
//!
//! // Get by perceptual hash
//! let image = get_image_by_perceptual_hash(&db_path, perceptual_hash)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Removing Images
//!
//! ```rust
//! use image_deduper_core::persistence::remove_image;
//! use std::path::PathBuf;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let db_path = PathBuf::new();
//! # let image_path = PathBuf::new();
//! // Remove an image by path
//! let removed = remove_image(&db_path, &image_path)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Clearing the Database
//!
//! ```rust
//! use image_deduper_core::persistence::clear_database;
//! use std::path::PathBuf;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let db_path = PathBuf::new();
//! // Remove all images from the database
//! let count = clear_database(&db_path)?;
//! println!("Removed {} images", count);
//! # Ok(())
//! # }
//! ```
//!
//! # Error Handling
//!
//! The module provides a custom error type [`PersistenceError`] and result type [`PersistenceResult`]
//! for handling database-specific errors, including:
//!
//! - Database errors (from rusqlite)
//! - Path-related errors
//! - Duplicate entry errors
//! - Not found errors
//! - Initialization errors
//!
//! These errors are properly mapped to the crate's main error type for seamless integration.

mod db;
mod error;
mod models;
#[cfg(test)]
mod tests;

pub use db::{
    add_image, clear_database, create_database_if_not_exists, get_image_by_crypto_hash,
    get_image_by_path, get_image_by_path_with_conn, get_image_by_perceptual_hash,
    initialize_database, open_database, remove_image, save_processed_image_with_conn,
    save_processed_images, Database,
};
pub use error::{PersistenceError, PersistenceResult};
pub use models::StoredImage;
