use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

struct NextDir {
    path: PathBuf,
    depth: usize,
}

pub struct WalkDir {
    iter: fs::ReadDir,
    folders: Vec<NextDir>,
    depth: usize,
    max_depth: Option<usize>,
}

impl WalkDir {
    pub fn new(path: impl AsRef<Path>, max_depth: impl Into<Option<usize>>) -> Result<Self> {
        Ok(Self {
            iter: fs::read_dir(path)?,
            folders: Vec::new(),
            depth: 0,
            max_depth: max_depth.into(),
        })
    }

    fn next_in_queue(&mut self) -> Option<()> {
        if self.folders.is_empty() {
            return None;
        }

        while let Some(next) = self.folders.pop() {
            match fs::read_dir(next.path) {
                Ok(iter) => {
                    self.iter = iter;
                    self.depth = next.depth;
                    return Some(());
                }

                Err(why) => {
                    log::error!("Cannot read a directory entry\n{:?}", why);
                }
            }
        }

        None
    }
}

impl Iterator for WalkDir {
    type Item = fs::DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(next) = self.iter.next() else {
                self.next_in_queue()?;
                continue;
            };

            match next {
                Ok(item) => {
                    let Ok(metadata) = item.metadata() else {
                        continue;
                    };

                    if metadata.is_dir() {
                        if !matches!(self.max_depth, Some(max) if max <= self.depth) {
                            self.folders.push(NextDir {
                                path: item.path(),
                                depth: self.depth + 1,
                            });
                        }
                    }

                    return Some(item);
                }
                Err(why) => {
                    log::error!("Cannot read a directory entry\n{:?}", why);
                }
            }
        }
    }
}
