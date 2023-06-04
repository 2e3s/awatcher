use image::Rgba;
use std::{env, error::Error, fs::File, io::BufReader};

fn main() -> Result<(), Box<dyn Error>> {
    if env::var_os("CARGO_FEATURE_BUNDLE").is_some() {
        let argb32 = convert("src/bundle/logo.png")?;
        std::fs::write("src/bundle/logo.argb32", argb32)?;
    }

    Ok(())
}

fn convert(filename: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let buf = BufReader::new(File::open(filename)?);
    let img = image::load(buf, image::ImageFormat::Png)?;
    let mut img = img.to_rgba8();
    for Rgba(pixel) in img.pixels_mut() {
        *pixel = u32::from_be_bytes(*pixel).rotate_right(8).to_be_bytes();
    }

    Ok(img.into_raw())
}
