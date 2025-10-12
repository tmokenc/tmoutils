use clap::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
pub struct Args {
    dir_list: Vec<String>,

    #[arg(long, short)]
    move_to: Option<String>,
}

struct Folder {
    path: PathBuf,
    target_path: Option<PathBuf>,
    entry_name_set: HashSet<String>,
}

impl Folder {
    pub fn new(path: impl AsRef<Path>, target_path: Option<&str>) -> anyhow::Result<Self> {
        let path = fs::canonicalize(path)?;

        let entry_name_set = fs::read_dir(&path)?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name != target_path.unwrap_or_default())
            .collect();

        let target_path = target_path.map(|v| path.join(v));

        if let Some(ref target_path) = target_path {
            fs::create_dir_all(&target_path).ok();
        }

        Ok(Self {
            path,
            target_path,
            entry_name_set,
        })
    }

    pub fn move_entry(&self, entry_name: &str) -> anyhow::Result<()> {
        if let Some(ref target_path) = self.target_path {
            let source_path = self.path.join(entry_name);
            let target_path = target_path.join(entry_name);
            fs::rename(&source_path, &target_path)?;
        }

        Ok(())
    }
}

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        let list: Vec<Folder> = self
            .dir_list
            .iter()
            .filter_map(|d| Folder::new(d, self.move_to.as_deref()).ok())
            .collect::<Vec<_>>();

        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                let common: Vec<_> = list[i]
                    .entry_name_set
                    .intersection(&list[j].entry_name_set)
                    .collect();

                if !common.is_empty() {
                    log::info!(
                        "Common entries between '{}' and '{}':",
                        list[i].path.display(),
                        list[j].path.display()
                    );

                    for name in &common {
                        log::info!("  {}", name);
                        list[i].move_entry(name)?;
                        list[j].move_entry(name)?;
                    }

                    println!();
                }
            }
        }

        Ok(())
    }
}
