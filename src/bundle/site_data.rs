use std::{fs, io::Cursor, path::PathBuf};

const SITE_DATA: &[u8] = include_bytes!("./dist.zip");

pub fn unpack_data() -> anyhow::Result<PathBuf> {
    let target_dir = std::env::temp_dir().join("awatcher");

    if target_dir.exists() {
        fs::remove_dir_all(&target_dir)?;
    }
    zip_extract::extract(Cursor::new(SITE_DATA), &target_dir, false)?;

    Ok(target_dir)
}
