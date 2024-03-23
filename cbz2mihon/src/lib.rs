use clap::*;
use serde::*;
use std::fmt::Write as _;
use std::fs;
use std::io::prelude::*;

const METADATA_FILE: &str = "info.yaml";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CbzMetadata {
    title: String,
    artist: Vec<String>,
    circle: Vec<String>,
    parody: Vec<String>,
    magazine: Vec<String>,
    tags: Vec<String>,
    released: Option<u64>,
    pages: u64,
    thumbnail: u64,
    url: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MihonMetadata {
    title: String,
    author: String,
    artist: String,
    description: String,
    genre: Vec<String>,
    status: &'static str, // 2 for complete
}

impl MihonMetadata {
    fn from_cbz_metadata(cbz: &CbzMetadata) -> Self {
        let mut description = format!("Parody: {}\nPage: {}\n", cbz.parody.join(", "), cbz.pages);

        if !cbz.magazine.is_empty() {
            writeln!(&mut description, "Magazine: {}", cbz.magazine.join(", ")).unwrap();
        }

        if let Some(released) = cbz.released {
            writeln!(&mut description, "Released: {}", released).unwrap();
        }

        if let Some(ref url) = cbz.url {
            writeln!(&mut description, "Url: {url}").unwrap();
        }

        if let Some(ref url) = cbz.source {
            writeln!(&mut description, "Source: {url}").unwrap();
        }

        let mut genre = cbz.tags.to_owned();
        genre.extend_from_slice(&cbz.artist);
        genre.extend_from_slice(&cbz.circle);

        Self {
            title: cbz.title.to_owned(),
            author: cbz.circle.join(", "),
            artist: cbz.artist.join(", "),
            status: "2",
            description,
            genre,
        }
    }
}

#[derive(Debug, Parser)]
/// Make all .cbz files that contains an info.yml into mihon local library format
pub struct Args {
    /// The base location to start with, default to the current one
    #[arg(default_value = ".")]
    location: String,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        let cbz_files = fs::read_dir(&self.location)?
            .filter_map(Result::ok)
            .filter(|v| v.path().extension().filter(|&v| v == "cbz").is_some());

        for cbz in cbz_files {
            log::info!("{:?}", cbz.path());

            let path = cbz.path().canonicalize()?;
            let file = fs::File::open(&path)?;
            let mut zip = zip::ZipArchive::new(file)?;

            let Ok(metadata) = zip.by_name(METADATA_FILE) else {
                log::error!("No {METADATA_FILE} found in {:?}", cbz.path());
                continue;
            };

            let yaml: CbzMetadata = serde_yaml::from_reader(metadata)?;
            // let new_path = PathBuf::from(&self.location).join(&yaml.title);
            let filename = path.file_stem().unwrap();
            let new_path = path.parent().unwrap().join(filename);
            let new_cbz = new_path.join(&path.file_name().unwrap());

            fs::create_dir(&new_path).ok();
            fs::rename(&path, new_cbz)?;

            let info_file = fs::File::create(new_path.join("details.json"))?;
            let mihon_metadata = MihonMetadata::from_cbz_metadata(&yaml);
            serde_json::to_writer(info_file, &mihon_metadata)?;

            // thumbnail
            let mut files = zip
                .file_names()
                .filter(|v| *v != METADATA_FILE)
                .map(String::from)
                .collect::<Vec<_>>();
            files.sort();

            let thumbnail_name = &files[yaml.thumbnail as usize - 1];
            let mut thumbnail = zip.by_name(&thumbnail_name)?;
            let mut thumbnail_file = fs::File::create(new_path.join("cover.jpg"))?;

            let mut buf = Vec::new();
            thumbnail.read_to_end(&mut buf)?;

            if thumbnail_name.ends_with(".jpg") {
                thumbnail_file.write_all(&buf)?;
            } else {
                let image = image::load_from_memory(&buf)?;
                image.write_to(&mut thumbnail_file, image::ImageFormat::Jpeg)?;
            }
        }

        Ok(())
    }
}
