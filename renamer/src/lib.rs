mod walkdir;

use clap::*;
use std::cmp::Reverse;
use std::fs;
use std::io::{self, Write as _};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum FileType {
    Folder,
    File,
    #[default]
    All,
}

#[derive(Debug, Parser)]
/// Put all files in the `location` into folder with its name, multiple files that has the same
/// name (but different extension) will go into the same folder
pub struct Args {
    /// Regex pattern to match
    pattern: String,

    #[arg(long, short, default_value_t = String::new())]
    /// replace the matches pattern by
    replace: String,

    #[arg(long, short, value_enum, default_value_t = FileType::File)]
    r#type: FileType,
    /// The base location to start with, default to the current one
    #[arg(long, short, default_value = ".")]
    location: String,

    #[arg(long, short, default_value_t = false)]
    /// Delete matches file. This will overwrite the `replace` flag
    delete: bool,

    #[arg(long)]
    depth: Option<usize>,

    #[arg(long, default_value_t = false)]
    /// Follow symlink or not
    follow: bool,
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        let regex = regex::Regex::new(&self.pattern)?;
        let mut todolist = Vec::new();

        for item in WalkDir::new(&self.location, self.depth)? {
            let Ok(metadata) = item.metadata() else {
                continue;
            };

            if !self.follow && metadata.is_symlink() {
                continue;
            }

            if matches!(
                (self.r#type, metadata.is_dir()),
                (FileType::File, true) | (FileType::Folder, false)
            ) {
                continue;
            }

            if matches!(
                (self.r#type, metadata.is_file()),
                (FileType::File, false) | (FileType::Folder, true)
            ) {
                continue;
            }

            let path = item.path();

            let Some(filename) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };

            if !regex.is_match(filename) {
                continue;
            }

            let new_name = regex.replace_all(filename, &self.replace);
            let new_path = path.parent().unwrap().join(&*new_name);

            todolist.push((path, new_path));
        }

        if todolist.is_empty() {
            log::info!("Nothing matches");
            return Ok(());
        }

        todolist.sort_by_key(|(current, _new)| Reverse(current.components().count()));

        for (i, (current, new)) in todolist.iter().enumerate() {
            if self.delete {
                log::info!("To Delete: #{i} {:#?}", &current);
            } else {
                log::info!("To Rename: #{i} {:#?}\n=> {:#?}\n", &current, &new);
            }
        }

        print!("Process? (Y/else): ");
        io::stdout().flush()?;
        let mut stdin = io::stdin().lines();
        if let Some(Ok(line)) = stdin.next() {
            if matches!(line.trim(), "y" | "yes") {
                for (current, new) in todolist {
                    if self.delete {
                        if current.is_dir() {
                            if let Err(why) = fs::remove_dir_all(current) {
                                log::error!("Cannot delete directory\n{:#?}", why);
                            }
                        } else {
                            if let Err(why) = fs::remove_file(current) {
                                log::error!("Cannot delete file\n{:#?}", why);
                            }
                        }
                    } else {
                        if let Err(why) = fs::rename(current, new) {
                            log::error!("Cannot renaming file\n{:#?}", why);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
