use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::{HashMap, HashSet};

use anyhow::Result;
use log::{debug, warn};
use regex::Regex;

pub(crate) struct Trie {
    next: HashMap<OsString, Trie>,
    ignore_exact: HashSet<String>,
    ignore_by_mask: HashMap<String, Regex>,
    fs_path: PathBuf,
}

impl Trie {
    fn new(fs_path: &Path) -> Result<Self> {
        let mut this = Self {
            next: HashMap::new(),
            ignore_exact: HashSet::new(),
            ignore_by_mask: HashMap::new(),
            fs_path: fs_path.to_owned(),
        };

        let mut dockerignore_file = fs_path.read_dir()?
            .into_iter()
            .filter_map(|f| f.ok())
            .filter_map(|f| {
                let path = f.path();
                if let Some(name) = path.file_name() {
                    if name == ".dockerignore" {
                        return Some(path);
                    }
                }
                None
            });

        if let Some(path) = dockerignore_file.next() {
            debug!("Found .dockerignore at {path:?}, loading rules");

            let file = File::open(path)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                this.add_line_to_ignore(line?);
            }
        }

        Ok(this)
    }

    fn add_line_to_ignore(&mut self, line: String) {
        if line.contains(&['*', '?']) {
            let reg_line = line
                .replace(".", "\\.")
                .replace("*", ".*");
            
            if let Result::Ok(regex) = Regex::new(&reg_line) {
                self.ignore_by_mask.entry(line).or_insert(regex);
            }
            else {
                warn!("Failed to process ignore pattern: '{line}'");
            }
        }
        else {
            self.ignore_exact.insert(line);
        }
    }

    pub fn init<'b>(from_cwd: &Path, exclude: impl Iterator<Item = &'b String>) -> Result<Self> {
        let mut this = Self::new(from_cwd)?;
        for item in exclude {
            this.add_line_to_ignore(item.clone());
        }
        Ok(this)
    }

    pub fn fold_path(&mut self, path: &Path) -> Result<&mut Trie>
    {
        debug!("Collect .dockerignore's from {:?} along path {path:?}", self.fs_path);

        let mut current = self;
        for item in path.iter() {
            current = current.append_dir(item)?;
        }

        Ok(current)
    }

    pub fn append_dir(&mut self, dir: &OsStr) -> Result<&mut Trie> {
        let dir = dir.to_owned();
        let next = self.next.entry(dir.clone())
            .or_insert(Trie::new(&self.fs_path.join(dir))?);
        next.ignore_exact.extend(self.ignore_exact.iter().cloned());
        
        for (fs_pattern, regex_pattern) in self.ignore_by_mask.iter() {
            next.ignore_by_mask.entry(fs_pattern.to_owned()).or_insert(regex_pattern.clone());
        }
        Ok(next)
    }

    pub fn should_ignore(&self, name: &OsStr) -> bool {
        if let Some(name) = name.to_str() {
            if self.ignore_exact.contains(name) {
                return true;
            }

            for reg_pattern in self.ignore_by_mask.values() {
                if reg_pattern.is_match(name) {
                    return true;
                }
            }

            false
        }
        else {
            true // if name can't be converted to string - ignore this file
        }
    }
}