use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub home_path: PathBuf,
    pub dot_path: PathBuf,
    pub backup_path: PathBuf,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Config> {
        let file = File::open(&path)?;
        let res: Config = serde_json::from_reader(file)?;
        Ok(res)
    }

    pub fn new<P, R, S>(home_path: P, dot_path: R, backup_path: S) -> Self
    where
        P: AsRef<Path>,
        R: AsRef<Path>,
        S: AsRef<Path>,
    {
        Self {
            home_path: home_path.as_ref().to_owned(),
            dot_path: dot_path.as_ref().to_owned(),
            backup_path: backup_path.as_ref().to_owned(),
        }
    }
}
