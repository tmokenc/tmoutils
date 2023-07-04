use clap::*;
use std::fs;

#[derive(Debug, Parser)]
/// Put all files in the `location` into folder with its name, multiple files that has the same
/// name (but different extension) will go into the same folder
pub struct Args {
    /// The base location to start with, default to the current one
    #[arg(default_value = ".")]
    location: String,
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
                continue
            };

            let Some(filestem) = file.file_stem() else {
                log::warn!("Cannot create folder with empty name for file {:?}", file);
                continue
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
