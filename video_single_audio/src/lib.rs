use clap::*;
use human_bytes::human_bytes;
use serde::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Deserialize)]
struct Properties {
    #[serde(default)]
    language: String,
    #[serde(default)]
    language_ietf: String,
}

#[derive(Deserialize)]
struct TrackInfo {
    // #[serde(default)]
    // codec: String,
    #[serde(default)]
    id: u64,
    #[serde(default)]
    r#type: String,
    properties: Properties,
}

#[derive(Deserialize)]
struct MkvInfo {
    // attackments : Vec<_>
    tracks: Vec<TrackInfo>,
}

impl MkvInfo {
    fn from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let cmd = Command::new("mkvmerge")
            .arg("-J")
            .arg(path.as_ref())
            .output()?;

        let s = std::str::from_utf8(&cmd.stdout)?;
        let result = serde_json::from_str(s)?;
        Ok(result)
    }
}

#[derive(Debug, Parser)]
/// Retain only a single audio, other audio tracks on a video is practically useless
/// in a lot of situation and it take up a lot of storage space.
pub struct Args {
    #[arg(default_value = ".")]
    location: String,

    #[arg(long, short, default_value_t = String::from("./output"))]
    /// Where to store the result
    output_dir: String,

    #[arg(long, short, default_value_t = false)]
    /// Replace the original files
    replace: bool,

    #[arg(long, short, default_value_t = 1)]
    /// Recursive done
    depth: usize,

    #[arg(long, short, default_value_t = String::from("jpn"))]
    audio_name: String,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        let iter = WalkDir::new(&self.location)
            .sort_by_file_name()
            .max_depth(self.depth)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|v| v.path().extension().filter(|x| x == &"mkv").is_some());

        let mut saved = 0;

        for entry in iter {
            let mut info = MkvInfo::from_path(entry.path())?;
            info.tracks.retain(|v| v.r#type == "audio");

            if info.tracks.len() < 2 {
                continue;
            }

            let maybe_track_id = info
                .tracks
                .iter()
                .find(|v| {
                    v.properties.language_ietf == self.audio_name
                        || v.properties.language == self.audio_name
                })
                .map(|v| v.id);

            if let Some(track_id) = maybe_track_id {
                log::info!("Processing {:#?}", entry.path());

                match self.retain_audio(entry.path(), track_id) {
                    Ok((original, retained)) => {
                        log::info!(
                            "{} => {}\n",
                            human_bytes(original as f64),
                            human_bytes(retained as f64)
                        );
                        saved += original - retained;
                    }
                    Err(why) => log::error!("Failed to process a video\n{:#?}", why),
                }
            }
        }

        log::info!("Saved total {}", human_bytes(saved as f64));

        Ok(())
    }

    fn retain_audio(&self, path: impl AsRef<Path>, track_id: u64) -> anyhow::Result<(u64, u64)> {
        let path = path.as_ref();
        let filename = path.file_name().unwrap();

        let output = PathBuf::from(&self.output_dir);
        let output_path = output.join(filename);

        Command::new("mkvmerge")
            .arg("-o")
            .arg(&output_path)
            .arg("-a")
            .arg(track_id.to_string())
            .arg(path)
            .output()?;

        let original = path.metadata()?.len();
        let retained = output_path.metadata()?.len();

        if self.replace {
            log::info!("Replacing");
            fs::remove_file(path)?;
            fs::rename(output_path, path)?;
        }

        Ok((original, retained))
    }
}
