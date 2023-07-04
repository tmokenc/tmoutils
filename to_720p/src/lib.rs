use anyhow::{Context as _, Error, Result};
use clap::Parser;
use std::collections::LinkedList;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

const SIZE_OFFSET: u64 = 300 * 1024 * 1024; // 300MB offset
const SIZE_PER_SECOND: u64 = 300000; // cal base on a video with 898MB and 53m
const SUPPORTED_EXT: &[&str] = &["mp4", "mkv", "avi", "ts", "wmv"];

#[derive(Parser, Debug)]
#[clap(name = "convert video to 720p mp4")]
pub struct Args {
    #[clap(default_value = ".")]
    path: String,

    #[clap(short, long, default_value = "..")]
    output_dir: String,

    #[clap(short, long)]
    /// limit number of files to be processed
    limit: Option<usize>,

    #[clap(short, long, default_value_t = 1)]
    /// folder depths to scan for video files
    depth: u16,

    #[clap(short, long)]
    /// replace the original file
    replace: bool,

    #[clap(short, long, default_value = "h264_nvenc")]
    /// Chose the encoder for video
    video: String,

    #[clap(short, long, default_value = "copy")]
    /// Chose the encoder for audio
    audio: String,

    #[clap(long)]
    /// Ignore files or folders
    ignore: Vec<String>,

    #[clap(long)]
    /// force all the video to be an mp4 one
    force_mp4: bool,

    #[clap(long)]
    /// force all the video to be 720p
    force_720: bool,

    #[clap(short, long)]
    /// shutdown after all the process done
    shutdown: bool,
}

impl Args {
    pub fn exec(&self) -> Result<()> {
        let path = PathBuf::from(&self.path);
        let output_dir = PathBuf::from(&self.output_dir);

        if path.is_file() {
            let video = Video::from_path_buf(path)?;

            if video.is_over_sized() {
                downscale(&video, &output_dir, &self.video, &self.audio, self.replace)?;
            } else {
                panic!("The video looks fine");
            }

            if self.shutdown {
                Command::new("shutdown").spawn()?.wait()?;
            }

            return Ok(());
        }

        log::info!("Loading videos");

        let mut iter = Videos::new(&path, &self.ignore, self.depth)?
            .inspect(|v| {
                log::info!(
                    "{} {}x{} ({}p) {} minutes {} MB - {:?}",
                    v.ext,
                    v.metadata.width,
                    v.metadata.height,
                    v.resolution(),
                    v.metadata.duration.as_secs() / 60,
                    v.size / 1024 / 1024,
                    v.path
                )
            })
            .filter(|v| {
                let check_720 = self.force_720 && v.resolution() > 720;
                let check_mp4 = self.force_mp4 && v.ext != "mp4";

                if check_720 || check_mp4 {
                    return true;
                }

                v.is_over_sized()
            });

        let mut list = iter
            .by_ref()
            .take(self.limit.unwrap_or(usize::MAX))
            .collect::<LinkedList<_>>();
        let mut total = list.len();

        let mut min_size_per_sec = list
            .iter()
            .map(|v| v.size_per_second())
            .min()
            .context("The list is empty")?;

        // take the remaining video in the iterator
        for video in iter {
            total += 1;
            let size_per_sec = video.size_per_second();

            if size_per_sec <= min_size_per_sec {
                continue;
            }

            let mut new_min = size_per_sec;
            let mut swapped = false;
            for item in list.iter_mut() {
                let size = item.size_per_second();
                if swapped || size != min_size_per_sec {
                    new_min = std::cmp::min(size, new_min);
                    continue;
                }

                *item = video.to_owned();
                swapped = true;
            }

            min_size_per_sec = new_min;
        }

        log::info!("Found {} videos need to process", total);
        log::info!("Taking first {}", list.len());
        log::info!("Sorting");

        log::info!("Processing");

        let start = Instant::now();
        for (video, i) in list.iter().zip(1..) {
            log::info!(
                "({i}/{}) {}x{} ({}p) {} minutes {}MB - {:?}",
                list.len(),
                video.metadata.width,
                video.metadata.height,
                video.resolution(),
                video.metadata.duration.as_secs() / 60,
                video.size / 1024 / 1024,
                video.path
            );

            let video_start = Instant::now();
            downscale(&video, &output_dir, &self.video, &self.audio, self.replace)?;
            log::info!(
                "Done! This video took {}",
                humantime::format_duration(video_start.elapsed())
            );
        }

        log::info!(
            "Done! All of them took {}",
            humantime::format_duration(start.elapsed())
        );

        if self.shutdown {
            Command::new("shutdown").spawn()?.wait()?;
        }

        Ok(())
    }
}

#[rustfmt::skip]
fn downscale(video: &Video, output_dir: &Path, cv: &str, ca: &str, replace: bool) -> Result<()> {
    let file_name = format!("{}.mp4", video.path.file_stem().unwrap().to_str().unwrap());
    let output = output_dir.join(&file_name);
    let mut command = Command::new("ffmpeg");

    let mut filters = vec![
        "-c:v", cv,
        "-c:a", ca,
        "-loglevel", "warning", "-hide_banner", "-stats"
    ];

    if let Some(filter) = video.vf_filter() {
        filters.extend(["-vf", filter]);
    }

    command
        .arg("-i")
        .arg(&video.path)
        .args(filters)
        .arg(&output)
        .stdin(Stdio::null());

    log::info!("Executing command\n{:?}", &command);

    let status = command.spawn()?.wait()?;

    if status.success() {
        let old_size: i64;
        let new_size: i64;


        #[cfg(target_os = "linux")]
        {
            old_size = video.path.metadata()?.size() as i64;
            new_size = output.metadata()?.size() as i64;
        }

        #[cfg(target_os = "windows")]
        {
            old_size = video.path.metadata()?.file_size() as i64;
            new_size = output.metadata()?.file_size() as i64;
        }

        log::info!(
            "Done {:?}\nOutput {:?}\nNew size {}MB (reduced {}MB)",
            video.path,
            output,
            new_size / 1024 / 1024,
            (old_size - new_size) / 1024 / 1024,
        );

        if replace && old_size > new_size {
            let new_file_path = video.path.canonicalize()?.parent().unwrap().join(file_name);
            move_file(&output, &new_file_path)?;
        }

        return Ok(());
    }

    Err(Error::msg("Something went wrong"))
}

#[cfg(target_os = "linux")]
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    let dev_a = fs::metadata(from)?.dev();
    let dev_b = fs::metadata(to)?.dev();

    fs::remove_file(to).ok();

    if dev_a == dev_b {
        fs::rename(from, to)?;
    } else {
        fs::copy(from, to)?;
        fs::remove_file(from)?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::remove_file(to).ok();
    fs::rename(from, to)
}

#[derive(Clone)]
struct Video {
    metadata: VideoMetadata,
    ext: &'static str,
    size: u64,
    path: PathBuf,
}

impl Video {
    fn from_path_buf(p: PathBuf) -> Result<Self> {
        let ext = p.extension().context("No extension")?.to_ascii_lowercase();
        let ext = *SUPPORTED_EXT
            .into_iter()
            .find(|&&v| v == ext)
            .context("File extension is not supported yet")?;

        let size = fs::metadata(&p)?.len();

        let metadata_opt = match ext {
            "mp4" => VideoMetadata::mp4(&p).ok(),
            "mkv" => VideoMetadata::mkv(&p).ok(),
            _ => None,
        };

        let metadata = metadata_opt
            .or_else(|| VideoMetadata::ffmpeg(&p).ok())
            .context("Cannot read video metadata")?;

        Ok(Self {
            path: p,
            size,
            ext,
            metadata,
        })
    }

    fn resolution(&self) -> u32 {
        std::cmp::min(self.metadata.width, self.metadata.height)
    }

    fn vf_filter(&self) -> Option<&'static str> {
        if self.resolution() <= 720 {
            return None;
        }

        if self.metadata.width > self.metadata.height {
            Some("scale=-1:720")
        } else {
            Some("scale=720:-1")
        }
    }

    fn is_over_sized(&self) -> bool {
        self.size > self.metadata.duration.as_secs() * SIZE_PER_SECOND + SIZE_OFFSET
    }

    fn size_per_second(&self) -> u64 {
        self.size / self.metadata.duration.as_secs()
    }
}

#[derive(Clone, Copy)]
struct VideoMetadata {
    height: u32,
    width: u32,
    duration: Duration,
}

impl VideoMetadata {
    fn ffmpeg(p: &Path) -> Result<Self> {
        let cmd = Command::new("ffprobe")
            .args(["-v", "error"])
            .args(["-select_streams", "v"])
            .arg("show_entries")
            .arg("stream=width,height,duration")
            .args(["-of", "csv=p=0:s=x"])
            .arg(&p)
            .output()?;

        let mut iter = std::str::from_utf8(&cmd.stdout)?.split("x");
        let width = iter.next().context("Get video width")?.parse::<u32>()?;
        let height = iter.next().context("Get video height")?.parse::<u32>()?;
        let duration = iter.next().context("Get video duration")?.parse::<f32>()?;
        let duration = Duration::from_secs_f32(duration);

        Ok(Self {
            height,
            width,
            duration,
        })
    }

    fn mp4(p: &Path) -> Result<Self> {
        let file = fs::File::open(p)?;
        let mp4 = mp4::read_mp4(file)?;
        let (height, width) = mp4
            .tracks()
            .values()
            .map(|v| (v.height(), v.width()))
            .max()
            .context("Cannot get the height")?;
        let duration = mp4.duration();

        Ok(Self {
            height: height as u32,
            width: width as u32,
            duration,
        })
    }

    fn mkv(p: &Path) -> Result<Self> {
        let file = fs::File::open(p)?;
        let mkv = matroska::Matroska::open(file)?;
        let duration = mkv.info.duration.context("Cannot get the duration")?;
        let video = mkv.video_tracks().next().context("No video track")?;

        if let matroska::Settings::Video(ref v) = video.settings {
            let height = v.display_height.unwrap_or(v.pixel_height);
            let width = v.display_width.unwrap_or(v.pixel_width);
            return Ok(Self {
                height: height as u32,
                width: width as u32,
                duration,
            });
        }

        Err(Error::msg("Cannot get the height"))
    }
}

struct SubFolder {
    depth: u16,
    path: PathBuf,
}

struct Videos {
    sub: Vec<SubFolder>,
    ignores: Vec<PathBuf>,
    max_depth: u16,
    current_depth: u16,
    iter: fs::ReadDir,
}

impl Videos {
    fn new(path: &Path, ignores: &[String], max_depth: u16) -> std::io::Result<Self> {
        let ignores = ignores
            .into_iter()
            .filter_map(|v| PathBuf::from(v).canonicalize().ok())
            .collect::<Vec<_>>();

        fs::read_dir(path).map(|iter| Self {
            max_depth,
            current_depth: 0,
            sub: Vec::new(),
            ignores,
            iter,
        })
    }
}

impl Iterator for Videos {
    type Item = Video;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(Err(_)) => continue,
                Some(Ok(entry)) => {
                    let path = entry.path();

                    if let Ok(canon) = path.canonicalize() {
                        if self.ignores.iter().any(|v| v == &canon) {
                            continue;
                        }
                    }

                    if path.is_dir() {
                        if self.max_depth > self.current_depth {
                            self.sub.push(SubFolder {
                                depth: self.current_depth + 1,
                                path,
                            });
                        }

                        continue;
                    }

                    if let Ok(video) = Video::from_path_buf(path) {
                        return Some(video);
                    }
                }

                None => {
                    let sub = self.sub.pop()?;

                    if let Ok(iter) = fs::read_dir(sub.path) {
                        self.iter = iter;
                        self.current_depth = sub.depth;
                    }
                }
            }
        }
    }
}
