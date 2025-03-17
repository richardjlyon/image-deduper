use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};

use super::error::{PersistenceError, PersistenceResult};
use super::models::StoredImage;
use crate::types::{ImageFormat, ProcessedImage};

/// Database version for schema migrations
const DB_VERSION: i32 = 1;

/// Database connection wrapper
pub struct Database {
    /// The SQLite connection
    conn: Connection,
}

impl Database {
    /// Create a new database with the given connection
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// Initialize the database schema if it doesn't exist
    pub fn initialize(&self) -> PersistenceResult<()> {
        self.create_schema()?;
        self.set_pragmas()?;
        Ok(())
    }

    /// Create the database schema
    fn create_schema(&self) -> PersistenceResult<()> {
        // First check if the tables already exist
        let table_count: i32 = self
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='images'",
                params![],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if table_count > 0 {
            // Schema exists, check version
            let version: i32 = self
                .conn
                .query_row("PRAGMA user_version", params![], |row| row.get(0))?;

            if version < DB_VERSION {
                // Update schema if needed
                self.migrate_schema(version)?;
            }

            return Ok(());
        }

        // Create tables
        self.conn.execute_batch(
            "BEGIN;
            CREATE TABLE IF NOT EXISTS images (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                size INTEGER NOT NULL,
                last_modified INTEGER NOT NULL,
                format TEXT NOT NULL,
                created INTEGER,
                cryptographic_hash BLOB NOT NULL,
                perceptual_hash TEXT NOT NULL
            );

            -- Create indexes for efficient lookups
            CREATE UNIQUE INDEX IF NOT EXISTS idx_images_path ON images(path);
            CREATE INDEX IF NOT EXISTS idx_images_crypto_hash ON images(cryptographic_hash);
            CREATE INDEX IF NOT EXISTS idx_images_perceptual_hash ON images(perceptual_hash);

            PRAGMA user_version = 1;
            COMMIT;",
        )?;

        Ok(())
    }

    /// Migrate schema from an older version
    fn migrate_schema(&self, _current_version: i32) -> PersistenceResult<()> {
        // Implement migrations as needed
        // For now, we don't have any migrations since this is v1

        // Update version
        self.conn
            .execute("PRAGMA user_version = ?1", params![DB_VERSION])?;

        Ok(())
    }

    /// Set SQLite performance pragmas
    fn set_pragmas(&self) -> PersistenceResult<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 30000000000;
             PRAGMA auto_vacuum = INCREMENTAL;
             PRAGMA integrity_check;
             PRAGMA locking_mode = NORMAL;
             PRAGMA busy_timeout = 30000;",
        )?;

        Ok(())
    }

    /// Add an image to the database if it doesn't exist.
    ///
    /// This function checks if the image already exists by path or by cryptographic hash
    /// before inserting it into the database to prevent duplicates.
    ///
    /// # Arguments
    ///
    /// * `image` - The image data to store, including file metadata and hashes
    ///
    /// # Returns
    ///
    /// * `Ok(i64)` - The ID of the inserted image if successful
    /// * `Err(PersistenceError::Duplicate)` - If the image already exists
    /// * `Err(PersistenceError::Database)` - If there was a database error
    ///
    /// # Examples
    ///
    /// ```
    /// use image_deduper_core::persistence::{Database, StoredImage};
    /// use image_deduper_core::types::ImageFile;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Database::new(rusqlite::Connection::open_in_memory()?);
    /// # let image_file = ImageFile {
    /// #    path: PathBuf::from("/path/to/image.jpg"),
    /// #    size: 1024,
    /// #    last_modified: std::time::SystemTime::now(),
    /// #    format: image_deduper_core::types::ImageFormat::Jpeg,
    /// #    created: None
    /// # };
    /// # let cryptographic_hash = vec![1, 2, 3, 4, 5];
    /// # let perceptual_hash = 12345678;
    ///
    /// let stored_image = StoredImage::new(&image_file, cryptographic_hash, perceptual_hash);
    /// match db.add_image(&stored_image) {
    ///     Ok(id) => println!("Added image with ID: {}", id),
    ///     Err(e) => println!("Failed to add image: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_image(&self, image: &StoredImage) -> PersistenceResult<i64> {
        // Check if image already exists by path
        if let Ok(_existing) = self.get_image_by_path(&image.path) {
            return Err(PersistenceError::Duplicate(format!(
                "Image already exists with path: {}",
                image.path.display()
            )));
        }

        // Check if image with same cryptographic hash exists
        if let Ok(existing) = self.get_image_by_crypto_hash(&image.cryptographic_hash) {
            return Err(PersistenceError::Duplicate(format!(
                "Image with identical hash already exists: {}",
                existing.path.display()
            )));
        }

        // Path string
        let path_str = image.path.to_string_lossy().to_string();

        // Format string
        let format_str = match &image.format {
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Png => "png",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Heic => "heic",
            ImageFormat::Raw => "raw",
            ImageFormat::Other(s) => s,
        };

        // Insert the image
        let id = self
            .conn
            .execute(
                "INSERT INTO images
            (path, size, last_modified, format, created, cryptographic_hash, perceptual_hash)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    path_str,
                    image.size,
                    image.last_modified,
                    format_str,
                    image.created,
                    image.cryptographic_hash,
                    image.perceptual_hash.to_string(),
                ],
            )
            .map(|_| self.conn.last_insert_rowid())?;

        Ok(id)
    }

    /// Remove an image from the database by path
    pub fn remove_image(&self, path: &Path) -> PersistenceResult<bool> {
        let path_str = path.to_string_lossy().to_string();

        let rows_affected = self
            .conn
            .execute("DELETE FROM images WHERE path = ?1", params![path_str])?;

        Ok(rows_affected > 0)
    }

    /// Remove an image from the database by ID
    pub fn remove_image_by_id(&self, id: i64) -> PersistenceResult<bool> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM images WHERE id = ?1", params![id])?;

        Ok(rows_affected > 0)
    }

    /// Get an image by path
    pub fn get_image_by_path(&self, path: &Path) -> PersistenceResult<StoredImage> {
        let path_str = path.to_string_lossy().to_string();

        let image = self.conn.query_row(
            "SELECT id, path, size, last_modified, format, created, cryptographic_hash, perceptual_hash
             FROM images WHERE path = ?1",
            params![path_str],
            |row| Ok(row_to_stored_image(row)),
        )?;

        Ok(image)
    }

    /// Get an image by cryptographic hash
    pub fn get_image_by_crypto_hash(&self, hash: &[u8]) -> PersistenceResult<StoredImage> {
        let image = self.conn.query_row(
            "SELECT id, path, size, last_modified, format, created, cryptographic_hash, perceptual_hash
             FROM images WHERE cryptographic_hash = ?1",
            params![hash],
            |row| Ok(row_to_stored_image(row)),
        )?;

        Ok(image)
    }

    /// Get images by perceptual hash
    pub fn get_images_by_perceptual_hash(&self, hash: u64) -> PersistenceResult<Vec<StoredImage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, size, last_modified, format, created, cryptographic_hash, perceptual_hash
             FROM images WHERE perceptual_hash = ?1"
        )?;

        let images = stmt
            .query_map(params![hash], |row| Ok(row_to_stored_image(row)))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// Get an image by perceptual hash (first match)
    pub fn get_image_by_perceptual_hash(&self, hash: u64) -> PersistenceResult<StoredImage> {
        let mut images = self.get_images_by_perceptual_hash(hash)?;

        if images.is_empty() {
            return Err(PersistenceError::NotFound(format!(
                "No image found with perceptual hash: {}",
                hash
            )));
        }

        Ok(images.remove(0))
    }

    /// Get all images
    pub fn get_all_images(&self) -> PersistenceResult<Vec<StoredImage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, size, last_modified, format, created, cryptographic_hash, perceptual_hash
             FROM images"
        )?;

        let images = stmt
            .query_map(params![], |row| Ok(row_to_stored_image(row)))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// Clear all images from the database
    pub fn clear(&self) -> PersistenceResult<usize> {
        let count = self.conn.execute("DELETE FROM images", params![])?;
        Ok(count)
    }

    /// Save a batch of processed images to the database
    pub fn save_processed_images(
        &mut self,
        images: &[crate::types::ProcessedImage],
    ) -> PersistenceResult<()> {
        let tx = self.conn.transaction()?;

        for image in images {
            let stored_image = StoredImage::new(
                &image.original,
                image.cryptographic_hash.as_bytes().to_vec(),
                image.perceptual_hash.0,
            );

            // Path string
            let path_str = stored_image.path.to_string_lossy().to_string();

            // Format string
            let format_str = match &stored_image.format {
                ImageFormat::Jpeg => "jpeg",
                ImageFormat::Png => "png",
                ImageFormat::Tiff => "tiff",
                ImageFormat::Heic => "heic",
                ImageFormat::Raw => "raw",
                ImageFormat::Other(s) => s,
            };

            // Check if image already exists by path
            let exists = tx
                .query_row(
                    "SELECT 1 FROM images WHERE path = ?1",
                    params![path_str],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if exists {
                continue;
            }

            // Insert the image
            tx.execute(
                "INSERT INTO images
                (path, size, last_modified, format, created, cryptographic_hash, perceptual_hash)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    path_str,
                    stored_image.size,
                    stored_image.last_modified,
                    format_str,
                    stored_image.created,
                    stored_image.cryptographic_hash,
                    stored_image.perceptual_hash.to_string(),
                ],
            )?;
        }

        // Commit the transaction
        tx.commit()?;

        Ok(())
    }
}

/// Helper function to convert a database row to a StoredImage
fn row_to_stored_image(row: &rusqlite::Row) -> StoredImage {
    let id: i64 = row.get(0).unwrap();
    let path_str: String = row.get(1).unwrap();
    let size: u64 = row.get(2).unwrap();
    let last_modified: i64 = row.get(3).unwrap();
    let format_str: String = row.get(4).unwrap();
    let created: Option<i64> = row.get(5).unwrap();
    let cryptographic_hash: Vec<u8> = row.get(6).unwrap();
    let perceptual_hash_str: String = row.get(7).unwrap();
    let perceptual_hash = perceptual_hash_str.parse::<u64>().unwrap_or(0);

    let format = match format_str.as_str() {
        "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "tiff" => ImageFormat::Tiff,
        "heic" => ImageFormat::Heic,
        "raw" => ImageFormat::Raw,
        other => ImageFormat::Other(other.to_string()),
    };

    StoredImage {
        id: Some(id),
        path: PathBuf::from(path_str),
        size,
        last_modified,
        format,
        created,
        cryptographic_hash,
        perceptual_hash,
    }
}

/// Open a database at the given path
pub fn open_database<P: AsRef<Path>>(path: P) -> PersistenceResult<Database> {
    let conn = Connection::open(path.as_ref())?;
    let db = Database::new(conn);
    db.initialize()?;
    Ok(db)
}

/// Create a database if it doesn't exist
pub fn create_database_if_not_exists<P: AsRef<Path>>(path: P) -> PersistenceResult<Database> {
    // Create parent directory if it doesn't exist
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PersistenceError::Initialization(format!("Failed to create directory: {}", e))
        })?;
    }

    // Open or create the database
    open_database(path)
}

/// Clear all records from a database
pub fn clear_database<P: AsRef<Path>>(path: P) -> PersistenceResult<usize> {
    let db = open_database(path)?;
    db.clear()
}

/// Add an image to the database
pub fn add_image<P: AsRef<Path>>(db_path: P, image: &StoredImage) -> PersistenceResult<i64> {
    let db = open_database(db_path)?;
    db.add_image(image)
}

/// Remove an image from the database by path
pub fn remove_image<P: AsRef<Path>, I: AsRef<Path>>(
    db_path: P,
    image_path: I,
) -> PersistenceResult<bool> {
    let db = open_database(db_path)?;
    db.remove_image(image_path.as_ref())
}

/// Get an image by path
pub fn get_image_by_path<P: AsRef<Path>, I: AsRef<Path>>(
    db_path: P,
    image_path: I,
) -> PersistenceResult<StoredImage> {
    let db = open_database(db_path)?;
    db.get_image_by_path(image_path.as_ref())
}

/// Get an image by cryptographic hash
pub fn get_image_by_crypto_hash<P: AsRef<Path>>(
    db_path: P,
    hash: &[u8],
) -> PersistenceResult<StoredImage> {
    let db = open_database(db_path)?;
    db.get_image_by_crypto_hash(hash)
}

/// Get an image by perceptual hash
pub fn get_image_by_perceptual_hash<P: AsRef<Path>>(
    db_path: P,
    hash: u64,
) -> PersistenceResult<StoredImage> {
    let db = open_database(db_path)?;
    db.get_image_by_perceptual_hash(hash)
}

/// Save a batch of processed images to the database
pub fn save_processed_images<P: AsRef<Path>>(
    db_path: P,
    images: &[crate::types::ProcessedImage],
) -> PersistenceResult<()> {
    let mut db = create_database_if_not_exists(db_path)?;
    db.save_processed_images(images)
}

/// Initialize the database schema
pub fn initialize_database(
    conn: &PooledConnection<SqliteConnectionManager>,
) -> PersistenceResult<()> {
    // First check if the tables already exist
    let table_count: i32 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='images'",
            params![],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if table_count > 0 {
        // Schema exists, check version
        let version: i32 = conn.query_row("PRAGMA user_version", params![], |row| row.get(0))?;

        if version < DB_VERSION {
            // Update schema if needed
            migrate_schema(conn, version)?;
        }

        return Ok(());
    }

    // Create tables
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS images (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            size INTEGER NOT NULL,
            last_modified INTEGER NOT NULL,
            format TEXT NOT NULL,
            created INTEGER,
            cryptographic_hash BLOB NOT NULL,
            perceptual_hash TEXT NOT NULL
        );

        -- Create indexes for efficient lookups
        CREATE UNIQUE INDEX IF NOT EXISTS idx_images_path ON images(path);
        CREATE INDEX IF NOT EXISTS idx_images_crypto_hash ON images(cryptographic_hash);
        CREATE INDEX IF NOT EXISTS idx_images_perceptual_hash ON images(perceptual_hash);

        PRAGMA user_version = 1;
        COMMIT;",
    )?;

    set_pragmas(conn)?;
    Ok(())
}

/// Migrate schema from an older version
fn migrate_schema(
    conn: &PooledConnection<SqliteConnectionManager>,
    _current_version: i32,
) -> PersistenceResult<()> {
    // Implement migrations as needed
    // For now, we don't have any migrations since this is v1

    // Update version
    conn.execute("PRAGMA user_version = ?1", params![DB_VERSION])?;

    Ok(())
}

/// Set SQLite performance pragmas
fn set_pragmas(conn: &PooledConnection<SqliteConnectionManager>) -> PersistenceResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;
         PRAGMA mmap_size = 30000000000;
         PRAGMA auto_vacuum = INCREMENTAL;
         PRAGMA integrity_check;
         PRAGMA locking_mode = NORMAL;
         PRAGMA busy_timeout = 30000;",
    )?;

    Ok(())
}

/// Get an image by path using a pooled connection
pub fn get_image_by_path_with_conn(
    conn: &PooledConnection<SqliteConnectionManager>,
    path: &Path,
) -> PersistenceResult<StoredImage> {
    let path_str = path.to_string_lossy().to_string();

    let image = conn.query_row(
        "SELECT id, path, size, last_modified, format, created, cryptographic_hash, perceptual_hash
         FROM images WHERE path = ?1",
        params![path_str],
        |row| Ok(row_to_stored_image(row)),
    )?;

    Ok(image)
}

/// Save a single processed image using a pooled connection
pub fn save_processed_image_with_conn(
    conn: &PooledConnection<SqliteConnectionManager>,
    image: &ProcessedImage,
) -> PersistenceResult<()> {
    let stored_image = StoredImage::new(
        &image.original,
        image.cryptographic_hash.as_bytes().to_vec(),
        image.perceptual_hash.0,
    );

    // Path string
    let path_str = stored_image.path.to_string_lossy().to_string();

    // Format string
    let format_str = match &stored_image.format {
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Png => "png",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Heic => "heic",
        ImageFormat::Raw => "raw",
        ImageFormat::Other(s) => s,
    };

    // Check if image already exists by path
    let exists = conn
        .query_row(
            "SELECT 1 FROM images WHERE path = ?1",
            params![path_str],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if exists {
        return Ok(());
    }

    // Insert the image
    conn.execute(
        "INSERT INTO images
        (path, size, last_modified, format, created, cryptographic_hash, perceptual_hash)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            path_str,
            stored_image.size,
            stored_image.last_modified,
            format_str,
            stored_image.created,
            stored_image.cryptographic_hash,
            stored_image.perceptual_hash.to_string(),
        ],
    )?;

    Ok(())
}
