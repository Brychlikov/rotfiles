use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::prelude::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    dot_path: PathBuf,
    backup_path: PathBuf,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Config> {
        let file = File::open(&path)?;
        let res: Config = serde_json::from_reader(file)?;
        Ok(res)
    }
}
