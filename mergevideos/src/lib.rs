use clap::*;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

const EXT: &[&str] = &["mp4", "avi", "mkv", "wmv"];

#[derive(Debug, Parser)]
/// Get video files in a folder and output them in a format that can be merged by ffmpeg
pub struct Args {
    #[arg(default_value = ".")]
    location: String,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        fs::read_dir(&self.location)?
            .filter_map(|v| v.ok())
            .filter_map(get_dup)
            .for_each(|(name, data)| write_ffmpeg_merge(&self.location, name, data).unwrap());

        Ok(())
    }
}

fn get_dup(dir: fs::DirEntry) -> Option<(PathBuf, Vec<PathBuf>)> {
    let path = dir.path();
    let mut data = Vec::new();

    log::info!("Processing {:?}", path);
    for entry in fs::read_dir(&path).ok()?.filter_map(|v| v.ok()) {
        let entry_path = entry.path();
        if entry_path
            .extension()
            .and_then(|v| v.to_str())
            .filter(|v| EXT.contains(v))
            .is_some()
        {
            log::info!("Video: {:?}", entry_path);
            data.push(entry_path);
        }
    }

    if data.len() < 2 {
        return None;
    }

    Some((path, data))
}

fn write_ffmpeg_merge(
    location: impl Into<PathBuf>,
    path: PathBuf,
    mut files: Vec<PathBuf>,
) -> anyhow::Result<()> {
    files.sort_unstable();

    let filename = path.file_name().and_then(|v| v.to_str()).unwrap();
    let list_name = location.into().join(format!("{filename}.txt"));
    let mut list = fs::File::create(&list_name)?;

    log::info!("Output: {:?}", list_name);

    for (i, file) in files.iter().enumerate() {
        writeln!(&mut list, "file '{}'", file.to_str().unwrap())?;
        println!("{i}: {:?}: ", file);
    }

    let mut stdin = io::stdin().lines();

    print!("Merge? (Y/else): ");
    io::stdout().flush()?;

    if let Some(Ok(line)) = stdin.next() {
        if matches!(line.to_ascii_lowercase().trim(), "y" | "yes") {
            if !exec_ffmpeg(list_name, path.join(format!("{:?}.mp4", filename))) {
                return Ok(());
            }

            print!("Delete splitted video files? (Y/else): ");
            io::stdout().flush()?;

            if let Some(Ok(line)) = stdin.next() {
                if matches!(line.to_ascii_lowercase().trim(), "y" | "yes") {
                    delete_videos(files);
                }
            }
        }
    }

    print!("Delete splitted videos? (Y/else): ");
    io::stdout().flush()?;

    Ok(())
}

fn exec_ffmpeg(list: PathBuf, output: PathBuf) -> bool {
    Command::new("ffmpeg")
        .args(["-safe", "0", "-f", "concat", "-i"])
        .arg(list)
        .args(["-c", "copy"])
        .arg(output)
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success()
}

fn delete_videos(files: Vec<PathBuf>) {
    for file in files {
        fs::remove_file(file).unwrap();
    }
}
