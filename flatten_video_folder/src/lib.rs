use anyhow::{bail, Result};
use clap::*;
use infer::MatcherType;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

// From most to least
const COVER_NAME_PRIORITY: &[&str] = &["folder", "backdrop", "cover"];

#[derive(Debug, Parser)]
/// Move the movie inside a directory out, its name will be renamed to the directory name.
/// Currently does not work recursively
pub struct Args {
    /// The base location to start with, default to the current one
    #[arg(default_value = ".")]
    location: String,
}

impl Args {
    pub fn exec(&self) -> Result<()> {
        let dir = fs::read_dir(&self.location)?
            .filter_map(|v| v.ok())
            .filter(|v| match v.metadata() {
                Ok(metadata) => metadata.is_dir(),
                Err(_) => false,
            });

        for entry in dir {
            if let Err(why) = self.process_dir(entry.path()) {
                log::error!("Error why processing a directory\n{:#?}", why);
            }
        }

        Ok(())
    }

    fn process_dir(&self, path: PathBuf) -> Result<()> {
        let Some(parent) = path.parent() else {
            bail!("Cannot get parent of the directory {:#?}", path);
        };

        let Some(dir_name) = path.file_name() else {
            bail!("Cannot get file name of the directory");
        };

        log::info!("Directory {:#?}", dir_name);

        let entries = fs::read_dir(&path)?
            .filter_map(|v| v.ok())
            .filter(|v| match v.metadata() {
                Ok(metadata) => metadata.is_file(),
                Err(_) => false,
            });

        let mut videos = Vec::new();
        let mut cover_image: Option<PathBuf> = None;
        let mut cover_image_score: Option<u8> = None;

        for entry in entries {
            let path = entry.path();
            let Ok(Some(kind)) = infer::get_from_path(&path) else {
                continue;
            };

            match kind.matcher_type() {
                MatcherType::Video => videos.push(entry.path()),
                MatcherType::Image => {
                    log::info!("{:#?}", path);
                    let Some(file_name) = path.file_name() else {
                        continue;
                    };

                    let score = if file_name == dir_name {
                        0
                    } else {
                        match COVER_NAME_PRIORITY.iter().position(|v| v == &file_name) {
                            Some(pos) => pos as u8 + 1,
                            None => u8::MAX,
                        }
                    };

                    if cover_image_score.filter(|&v| v < score).is_none() {
                        cover_image = Some(path);
                        cover_image_score = Some(score);
                    }
                }

                _ => continue,
            }
        }

        // Moving out
        videos.sort();

        if videos.len() == 1 {
            let video = videos.pop().unwrap();
            if let Some(filename) = video.file_name() {
                let mut new_path = parent.join(filename);
                set_file_stem(&mut new_path, dir_name);
                rename_guard(video, new_path);
            }
        } else {
            for (video, i) in videos.into_iter().zip(1..) {
                let Some(filename) = video.file_name() else {
                    continue;
                };

                let mut new_path = parent.join(filename);
                set_file_stem(
                    &mut new_path,
                    format!("{}_{}", dir_name.to_string_lossy(), i),
                );
                rename_guard(video, new_path);
            }
        }

        if let Some(cover) = cover_image {
            if let Some(filename) = cover.file_name() {
                let mut new_path = parent.join(filename);
                set_file_stem(&mut new_path, dir_name);
                rename_guard(cover, new_path);
            }
        }

        Ok(())
    }
}

fn set_file_stem(path: &mut PathBuf, name: impl AsRef<OsStr>) {
    let ext = match path.extension() {
        Some(ext) => ext.to_os_string(),
        None => return,
    };

    path.set_file_name(format!(
        "{}.{}",
        name.as_ref().to_string_lossy(),
        ext.to_string_lossy()
    ));
}

fn rename_guard(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    let from = from.as_ref();
    let to = to.as_ref();

    log::info!("Moving {:?} to {:?}", from, to);
    if let Err(why) = fs::rename(from, to) {
        log::error!("Cannot move {:#?} to {:#?}\n {:#?}", from, to, why);
    }
}
