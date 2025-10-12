use anyhow::*;
use clap::*;
use std::fs;
use std::io::Read as _;
use std::io::Seek as _;
use std::io::Write as _;
use std::path::Path;

#[derive(Debug, Parser)]
pub struct Args {
    /// Location to starts with
    #[arg(default_value_t = String::from("."))]
    location: String,
    #[arg(long, short, default_value_t = false)]
    /// replace the output.jpg file if exist
    replace: bool,
    #[arg(long, short, default_value_t = true)]
    /// Delete original image
    delete: bool,
    #[arg(long, short, default_value_t = 5)]
    /// Max directory depth
    depth: usize,
    #[arg(long, short, default_value_t = false)]
    /// should follow symlink or not
    follow_symlink: bool,
}

impl Args {
    pub fn exec(&self) -> Result<()> {
        let location = fs::canonicalize(&self.location)?;
        let walker = walkdir::WalkDir::new(location)
            .follow_links(self.follow_symlink)
            .max_depth(self.depth)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|v| v.file_type().is_file());

        for entry in walker {
            if let Err(why) = process(entry.path(), self) {
                log::error!("Cannot process {}\n{:#?}", entry.path().display(), why);
                continue;
            }
        }

        Ok(())
    }
}

fn process(path: &Path, opt: &Args) -> Result<()> {
    let mut file = fs::File::open(path)?;
    if !is_webp(&mut file)? {
        return Ok(());
    }

    println!(""); // skip 1 line
    log::info!("Processing {}", path.to_string_lossy());
    let image = image::load(std::io::BufReader::new(file), image::ImageFormat::WebP)?;
    let name = path.file_stem().context("Failed to get filename")?;
    let output_path = path
        .parent()
        .context("No Parent")?
        .join(format!("{}.jpg", name.to_string_lossy()));

    if std::fs::exists(&output_path)? {
        log::warn!("Path exist");
        if !opt.replace {
            log::warn!("Skipping");
            return Ok(());
        }
    }

    if opt.delete {
        log::info!("Removing");
        fs::remove_file(path)?;
    }

    let mut output = std::io::BufWriter::new(fs::File::create(output_path)?);

    image.write_to(&mut output, image::ImageFormat::Jpeg)?;
    output.flush()?;
    log::info!("Done");

    Ok(())
}

fn is_webp(file: &mut fs::File) -> Result<bool> {
    let mut buf: [u8; 12] = Default::default();
    file.read_exact(&mut buf)?;
    file.rewind()?;
    return Ok(infer::image::is_webp(&buf));
}
