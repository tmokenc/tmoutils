use clap::*;
use std::ffi::OsStr;
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const IMG_EXT: &'static [&'static str] = &["jpg", "png", "tiff"];
const THUMBNAIL_NAME: &'static [&'static str] = &["folder", "cover", "thumbnail", "thumb"];

#[derive(Parser, Debug)]
/// Thumbnailning video folders for `nemo` file explorer on linux
pub struct Args {
    #[arg(default_value = ".")]
    location: String,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        fs::read_dir(&self.location)?
            .filter_map(Result::ok)
            .filter(|v| v.metadata().map(|p| p.is_dir()).unwrap_or(false))
            .for_each(|v| match change_thumbnail(&v.path()) {
                Ok(p) => log::info!(
                    "Success thumbnailing for {:?}\nThumbnail: {:?}",
                    v.path(),
                    p
                ),
                Err(why) => log::error!("Cannot change thumbnail for {:?}\n{:#?}", v.path(), why),
            });

        Ok(())
    }
}

fn gio_set_thumbnail(dir: &Path, thumbnail: &Path) -> Result<()> {
    let mut command = Command::new("gio");

    command
        .arg("set")
        .arg(dir)
        .arg("metadata::custom-icon")
        .arg(thumbnail.file_name().unwrap());

    command.output()?;

    Ok(())
}

fn is_getchu_name(s: &OsStr) -> bool {
    if let Some(s) = s.to_str() {
        if !s.ends_with("top") {
            return false;
        }

        return s.trim_end_matches("top").parse::<u32>().is_ok();
    }

    false
}

fn is_known_by_name(s: &OsStr) -> bool {
    let s = s.to_ascii_lowercase();
    THUMBNAIL_NAME.iter().any(|v| v == &s)
}

fn change_thumbnail(path: &Path) -> Result<PathBuf> {
    let dir_name = path.file_name().unwrap();
    let entries = fs::read_dir(&path)?
        .filter_map(|v| v.ok())
        .map(|v| v.path());

    for entry in entries {
        if entry.is_dir() {
            continue;
        }

        let extension = match entry.extension() {
            Some(n) => n.to_ascii_lowercase(),
            None => continue,
        };

        if !IMG_EXT.iter().any(|&v| extension == v) {
            continue;
        }

        let name = match entry.file_stem() {
            Some(n) => n,
            None => continue,
        };

        if name == dir_name || is_known_by_name(name) || is_getchu_name(&name) {
            if gio_set_thumbnail(path, &entry).is_ok() {
                return Ok(entry);
            }
        }
    }

    Err(Error::new(
        ErrorKind::NotFound,
        "Cannot find any suitable thumbnail",
    ))
}
