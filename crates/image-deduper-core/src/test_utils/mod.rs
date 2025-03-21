use std::path::PathBuf;

#[cfg(test)]
pub mod test_support;

#[cfg(test)]
pub fn get_test_data_path(file_type: &str, filename: &str) -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("tests")
        .join("data")
        .join(file_type)
        .join(filename)
}
