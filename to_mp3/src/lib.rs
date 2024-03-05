use claxon::{FlacReader, FlacReaderOptions};
use std::env;
use std::fs;
use std::io::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<()> {
    let p = PathBuf::from(env::args().nth(1).expect("Folder Path")).canonicalize()?;
    let to = env::args()
        .nth(2)
        .map(|v| v.into())
        .unwrap_or(p.join("output/"));

    fs::create_dir_all(&to)?;

    fs::read_dir(p)?
        .filter_map(|v| v.ok()?.path().into())
        .filter(|p| is_flac(p))
        .filter(|p| get_bits_per_sample(p).unwrap_or(0) > 16)
        .inspect(|v| println!("Processing {:?}", v))
        .try_for_each(|p| downscale(&p, to.as_ref()))
}

fn is_flac(p: &Path) -> bool {
    p.extension()
        .filter(|e| e.to_ascii_lowercase() == "flac")
        .is_some()
}

fn get_bits_per_sample(p: &Path) -> claxon::Result<u32> {
    const OPT: FlacReaderOptions = FlacReaderOptions {
        metadata_only: true,
        read_vorbis_comment: false,
    };

    FlacReader::open_ext(p, OPT).map(|flac| flac.streaminfo().bits_per_sample)
}

fn downscale(p: &Path, to: &Path) -> Result<()> {
    let save_to = if to.is_dir() {
        to.join(p.file_name().unwrap())
    } else {
        to.to_owned()
    };

    Command::new("ffmpeg")
        .arg("-i")
        .arg(p.as_os_str())
        .arg("-af")
        .arg("aresample=out_sample_fmt=s16:out_sample_rate=48000")
        .arg(save_to.as_os_str())
        .output()?;

    Ok(())
}

use clap::Parser;
use std::fs;

#[derive(Debug, Parser)]
/// Put all files in the `location` into folder with its name, multiple files that has the same
/// name (but different extension) will go into the same folder
pub struct Args {
    /// The base location to start with, default to the current one
    #[arg(default_value = ".")]
    location: String,

    #[arg(default_value = "./output")]
    output_dir: String,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        let files = fs::read_dir(&self.location)?
            .filter_map(Result::ok)
            .map(|v| v.path())
            .filter(|v| v.is_file())
            .inspect(|v| log::trace!("File: {:?}", v));

        for file in files {
            let Some(filename) = file.file_name() else {
                log::warn!("Cannot create folder with empty name for file {:?}", file);
                continue;
            };

            let Some(filestem) = file.file_stem() else {
                log::warn!("Cannot create folder with empty name for file {:?}", file);
                continue;
            };

            let folder = file.parent().unwrap().join(&filestem);

            fs::create_dir(&folder).ok();

            let new_path = folder.join(filename);

            if let Err(why) = fs::rename(&file, new_path) {
                log::error!("Cannot move file {:?} into {:?}\n{:#?}", file, folder, why);
            }
        }

        Ok(())
    }
}
