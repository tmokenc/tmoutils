use clap::*;
use rayon::prelude::*;
use serde::*;
use std::fmt::Write as _;
use std::fs;
use std::io::prelude::*;

const ARCHIVE_TYPE: &[&str] = &["zip", "cbz"];
const IMAGE_TYPE: &[&str] = &["jpg", "jpeg", "png", "gif"];
const METADATA_FILE: &[&str] = &["info.yaml"];

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
    thumbnail: usize,
    url: Option<String>,
    source: Option<String>,
}

struct Page {
    is_cover: bool,
    width: u32,
    height: u32,
}

impl CbzMetadata {
    fn to_comicinfo(&self, pages: &[Page]) -> String {
        let mut res = String::from(
            "<?xml version=\"1.0\"?>
<ComicInfo xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" 
xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">
<Manga>YesAndRightToLeft</Manga>
<AgeRating>R18+</AgeRating>",
        );

        write!(&mut res, "<Title>{}</Title>", self.title).unwrap();
        write!(&mut res, "<PageCount>{}</PageCount>", self.pages).unwrap();
        let mut url = Vec::new();
        url.extend(self.source.to_owned());
        url.extend(self.url.to_owned());

        if !url.is_empty() {
            write!(&mut res, "<Web>{}</Web>", url.join(", ")).unwrap();
        }

        if !self.artist.is_empty() {
            write!(&mut res, "<Writer>{}</Writer>", self.artist.join(", ")).unwrap();
        }

        if !self.circle.is_empty() {
            write!(
                &mut res,
                "<CoverArtist>{}</CoverArtist>",
                self.circle.join(", ")
            )
            .unwrap();
        }

        if !self.magazine.is_empty() {
            write!(
                &mut res,
                "<Publisher>{}</Publisher>",
                self.magazine.join(", ")
            )
            .unwrap();
        }

        res.push_str("<Genre>");
        res.push_str(&self.tags.join(","));
        if !self.tags.is_empty() && !self.parody.is_empty() {
            res.push_str(",");
        }
        res.push_str(&self.parody.join(","));
        res.push_str("</Genre>");

        for (i, page) in pages.iter().enumerate() {
            write!(
                &mut res,
                r#"<Page Image="{i}" ImageWidth="{}" ImageHeight="{}""#,
                page.width, page.height
            )
            .unwrap();

            if page.is_cover {
                res.push_str(r#" Type="FrontCover""#);
            }

            res.push_str("/>");
        }

        res.push_str("</ComicInfo>\n");
        res
    }
}

#[derive(Debug, Parser)]
/// Downscale manga to put into phone/tablet and add ComicInfo metadata if possible.
/// Currently support for cbz and zip files manga
pub struct Args {
    #[arg(default_value = ".")]
    location: String,

    #[arg(long, short)]
    /// Where to store the result
    output_dir: String,

    #[arg(long, short, default_value_t = 1600)]
    min_res: u32,

    #[arg(long, default_value_t = false)]
    /// Overwrite the output if it already exists
    overwrite: bool,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        fs::read_dir(&self.location)?
            .filter_map(Result::ok)
            .filter(|v| {
                v.path()
                    .extension()
                    .filter(|v| ARCHIVE_TYPE.iter().any(|x| x == v))
                    .is_some()
            })
			.collect::<Vec<_>>()
			.into_par_iter()
			.for_each(|v| {
                    let path = v.path();
                    log::info!("{:?}", path);
                    if let Err(why) = process_archive(&path, self) {
                        log::error!("{:?}: Cannot process the archive\n{:#?}", path, why);
                    }
                });

        Ok(())
    }
}

fn process_archive(archive: &std::path::Path, opt: &Args) -> anyhow::Result<()> {
    let path = archive.canonicalize()?;
    let file = fs::File::open(&path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut metadata: Option<CbzMetadata> = None;
    let mut inner_files = zip.file_names().map(String::from).collect::<Vec<_>>();

    inner_files.sort();

    let mut pages: Vec<Page> = Vec::new();
    let Some(filestem) = path.file_stem() else {
        log::error!("{:?}: no filename", path);
        return Ok(());
    };

    let filename = format!("{}.cbz", filestem.to_str().unwrap());
    let mut new_path = std::path::PathBuf::from(&opt.output_dir);
    new_path.push(filename);

    if !opt.overwrite && new_path.exists() {
        return Ok(());
    }

    let new_file = fs::File::create(new_path)?;
    let mut result = zip::ZipWriter::new(new_file);

    for (i, file) in inner_files.into_iter().enumerate() {
        if METADATA_FILE.contains(&&*file) {
            let file = zip.by_name(&file)?;
            let mut thumbnail = 1;
            match serde_yaml::from_reader(file) {
                Ok(res) => {
                    let res: CbzMetadata = res;
                    thumbnail = res.thumbnail;
                    metadata.replace(res);
                }
                Err(why) => log::error!("{:?}: cannot parse metadata\n{:#?}", path, why),
            }

            if thumbnail != 1 {
                if pages.len() >= thumbnail {
                    pages[thumbnail - 1].is_cover = true;
                }
            }

            continue;
        }

        if !IMAGE_TYPE.iter().any(|v| file.ends_with(v)) {
            log::warn!("{:?}: unknown file type {file}", archive);
            continue;
        }

        let mut image_data = zip.by_name(&file)?;

        let mut buf = Vec::new();
        image_data.read_to_end(&mut buf)?;
        let mut image = image::load_from_memory(&buf)?;

        drop(image_data);
        image_data = zip.by_name(&file)?;

        if file.ends_with(".gif") {
            pages.push(Page {
                width: image.width(),
                height: image.height(),
                is_cover: metadata.as_ref().filter(|v| v.thumbnail - 1 == i).is_some(),
            });

            result.raw_copy_file(image_data)?;
            continue;
        }

        if (file.ends_with(".jpg") || file.ends_with(".jpeg"))
            && std::cmp::min(image.width(), image.height()) <= opt.min_res
        {
            result.raw_copy_file(image_data)?;
        } else {
            result.start_file(file.replace(".png", ".jpg"), Default::default())?;
            let buf = Vec::new();
            let mut writer = std::io::BufWriter::new(std::io::Cursor::new(buf));

            let mut width = image.width();
            let mut height = image.height();

            if width > height {
                height = opt.min_res;
            } else {
                width = opt.min_res;
            }

            image = image.resize(width, height, image::imageops::FilterType::CatmullRom);
            let rgb_image = image.to_rgb8();
            rgb_image.write_to(&mut writer, image::ImageFormat::Jpeg)?;
            writer.flush()?;

            result.write_all(writer.get_ref().get_ref())?;
            result.flush()?;
        }

        pages.push(Page {
            width: image.width(),
            height: image.height(),
            is_cover: metadata.as_ref().filter(|v| v.thumbnail - 1 == i).is_some(),
        });
    }

    if let Some(ref metadata) = metadata {
        result.start_file("ComicInfo.xml", Default::default())?;
        result.write_all(metadata.to_comicinfo(&pages).as_bytes())?;
    } else {
        log::warn!("No metadata found in {:?}", archive);
    }

    Ok(())
}
