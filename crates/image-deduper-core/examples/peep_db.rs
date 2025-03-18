use rocksdb::{IteratorMode, Options, DB};
use std::env;
use std::error::Error;
use std::fmt::Write as FmtWrite;
use std::path::Path;

// Define your PHash enum to match your application
#[derive(Debug)]
enum PHash {
    Standard(u64),
    Enhanced([u64; 16]),
}

fn main() -> Result<(), Box<dyn Error>> {
    // Hardcoded database path - CHANGE THIS TO YOUR ACTUAL PATH
    let db_path =
        "/Users/richardlyon/dev/mine/rust/image-deduper/crates/image-deduper-core/image_hash_db";

    // Default display options
    let limit = 20;
    let show_phashes = true;
    let show_chashes = true;

    println!("Opening RocksDB at: {}", db_path);

    // Open the database in read-only mode
    let options = Options::default();
    let db = DB::open_for_read_only(&options, db_path, false)?;

    // Open the database in read-only mode
    let options = Options::default();
    let db = DB::open_for_read_only(&options, db_path, false)?;

    println!("Inspecting RocksDB at: {}", db_path);

    // Count and display perceptual hashes
    if show_phashes {
        let p_count = count_prefix_entries(&db, b"p:")?;
        println!("Perceptual hashes: {} entries", p_count);
    }

    // Count and display cryptographic hashes
    if show_chashes {
        let c_count = count_prefix_entries(&db, b"c:")?;
        println!("Cryptographic hashes: {} entries", c_count);
    }

    // Count entries with no recognized prefix
    let other_count = count_other_entries(&db, &[b"p:", b"c:"])?;
    if other_count > 0 {
        println!("Other entries (no recognized prefix): {}", other_count);
    }

    Ok(())
}

// Count entries with a specific prefix
fn count_prefix_entries(db: &DB, prefix: &[u8]) -> Result<usize, Box<dyn Error>> {
    let count = db
        .iterator(IteratorMode::Start)
        .filter_map(|r| r.ok())
        .filter(|(key, _)| key.starts_with(prefix))
        .count();
    Ok(count)
}

// Count entries without any of the specified prefixes
fn count_other_entries(db: &DB, prefixes: &[&[u8]]) -> Result<usize, Box<dyn Error>> {
    let count = db
        .iterator(IteratorMode::Start)
        .filter_map(|r| r.ok())
        .filter(|(key, _)| !prefixes.iter().any(|prefix| key.starts_with(prefix)))
        .count();
    Ok(count)
}

// List entries with a specific prefix
fn list_hash_entries(
    db: &DB,
    prefix: &[u8],
    limit: usize,
    is_phash: bool,
) -> Result<(), Box<dyn Error>> {
    let mut count = 0;
    for (key, value) in db
        .iterator(IteratorMode::Start)
        .filter_map(|r| r.ok())
        .filter(|(key, _)| key.starts_with(prefix))
        .take(limit)
    {
        // Print the key (without prefix) as hex
        let key_without_prefix = &key[prefix.len()..];
        let hex_key = format_as_hex(key_without_prefix);

        // Try to display the path
        let path = String::from_utf8_lossy(&value);

        // Display the hash value interpretation if it's a perceptual hash
        let hash_interpretation = if is_phash {
            // Try to interpret as PHash
            interpret_phash(key_without_prefix)
        } else {
            // For blake3, just show summary
            if key_without_prefix.len() == 32 {
                " [blake3 hash]".to_string()
            } else {
                "".to_string()
            }
        };

        println!(
            "{}: {} => {}{}",
            count + 1,
            hex_key,
            path,
            hash_interpretation
        );
        count += 1;
    }

    if count == 0 {
        println!(
            "  No entries found with prefix: {}",
            String::from_utf8_lossy(prefix)
        );
    }

    Ok(())
}

// List entries without any of the specified prefixes
fn list_other_entries(db: &DB, prefixes: &[&[u8]], limit: usize) -> Result<(), Box<dyn Error>> {
    let mut count = 0;
    for (key, value) in db
        .iterator(IteratorMode::Start)
        .filter_map(|r| r.ok())
        .filter(|(key, _)| !prefixes.iter().any(|prefix| key.starts_with(prefix)))
        .take(limit)
    {
        // Print the full key as hex
        let hex_key = format_as_hex(&key);

        // Try to display the value as UTF-8, fall back to hex
        let value_display = match String::from_utf8(value.to_vec()) {
            Ok(s)
                if s.chars()
                    .all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace()) =>
            {
                s
            }
            _ => format_as_hex(&value),
        };

        println!("{}: {} => {}", count + 1, hex_key, value_display);
        count += 1;
    }

    if count == 0 {
        println!("  No entries found without recognized prefixes");
    }

    Ok(())
}

// Format bytes as a hex string
fn format_as_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

// Try to interpret bytes as a PHash
fn interpret_phash(bytes: &[u8]) -> String {
    if bytes.len() == 8 {
        // Likely a standard PHash (u64)
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(bytes);
        let value = u64::from_be_bytes(value_bytes);
        format!(" [Standard PHash: {}]", value)
    } else if bytes.len() == 128 {
        // Likely an enhanced PHash ([u64; 16])
        " [Enhanced PHash - 1024 bits]".to_string()
    } else {
        format!(" [Unknown PHash format: {} bytes]", bytes.len())
    }
}
