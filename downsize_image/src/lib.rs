use clap::Parser;
use image::{self, imageops, ImageFormat};
use rayon::prelude::*;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use zip::{read::ZipArchive, write::FullFileOptions};

#[derive(Parser)]
#[command(name = "zip_image_optimizer")]
#[command(about = "Optimizes images in ZIP files by resizing them to JPG if it reduces size")]
pub struct Args {
    /// Path to the folder containing ZIP files
    path: PathBuf,

    /// Minimum size for the shorter side of the image (pixels)
    #[arg(long, default_value_t = 1280)]
    min_size: u32,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        let args = Args::parse();
        let entries = fs::read_dir(&args.path)?;

        let out_path = args.path.join("out");
        fs::create_dir_all(&out_path).ok();

        entries.par_bridge().try_for_each(|entry| {
            let entry = entry?;
            let path = entry.path();
            log::info!("Processing file: {}\n", path.display());
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("zip") {
                if let Err(why) = process_zip(&path, args.min_size) {
                    eprintln!("Failed to process {}: {}", path.display(), why);
                }
            }
            anyhow::Ok(())
        })?;

        Ok(())
    }
}

fn process_zip(zip_path: &Path, min_size: u32) -> io::Result<()> {
    let file = File::open(zip_path)?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)?;

    let new_zip_path = zip_path
        .parent()
        .unwrap()
        .join("out")
        .join(zip_path.file_name().unwrap());

    let new_file = File::create(&new_zip_path)?;
    let writer_buf = BufWriter::new(new_file);
    let mut writer = zip::ZipWriter::new(writer_buf);

    for i in 0..archive.len() {
        let mut zip_file = archive.by_index(i)?;
        if zip_file.is_dir() {
            continue;
        }

        let name = zip_file.name().to_string();
        let compression = zip_file.compression();
        let options = FullFileOptions::default()
            .compression_method(compression)
            .unix_permissions(zip_file.unix_mode().unwrap_or(0o644))
            .last_modified_time(zip_file.last_modified().unwrap_or_default());

        if is_likely_image(&name) {
            let mut original_data = Vec::new();
            zip_file.read_to_end(&mut original_data)?;

            if let Ok(original_img) = image::load_from_memory(&original_data) {
                let width = original_img.width();
                let height = original_img.height();
                if width == 0 || height == 0 {
                    writer.start_file(&name, options)?;
                    writer.write_all(&original_data)?;
                    writer.flush()?;
                    continue;
                }

                let min_side = width.min(height);
                let scale_factor = min_size as f64 / min_side as f64;
                let new_width = (width as f64 * scale_factor).round() as u32;
                let new_height = (height as f64 * scale_factor).round() as u32;

                let resized_img =
                    original_img.resize(new_width, new_height, imageops::FilterType::Lanczos3);

                let mut resized_data = Vec::new();
                let mut cursor = Cursor::new(&mut resized_data);
                if resized_img.write_to(&mut cursor, ImageFormat::Jpeg).is_ok() {
                    if resized_data.len() < original_data.len() {
                        let name = Path::new(&name)
                            .with_extension("jpg")
                            .to_string_lossy()
                            .to_string();

                        writer.start_file(&name, options)?;
                        writer.write_all(&resized_data)?;
                        writer.flush()?;
                        continue;
                    }
                }
            }

            writer.start_file(&name, options)?;
            writer.write_all(&original_data)?;
        } else {
            writer.raw_copy_file(zip_file)?;
        }

        writer.flush()?;
    }

    let writer = writer.finish()?;
    writer.into_inner()?.flush()?;
    // Ensure the writer is dropped to flush and close the file
    // drop(writer);

    // Compare sizes
    let original_size = fs::metadata(zip_path)?.len();
    let new_size = fs::metadata(&new_zip_path)?.len();

    if new_size >= original_size {
        fs::remove_file(&new_zip_path)?;
    }

    Ok(())
}

fn is_likely_image(name: &str) -> bool {
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());
    matches!(
        ext.as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("gif") | Some("bmp") | Some("webp")
    )
}
