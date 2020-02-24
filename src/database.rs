extern crate serde_json;
extern crate serde;

use std::path::PathBuf;

use serde::{Serialize, Deserialize};
use std::io::prelude::*;
use crate::errors::*;
use std::time::SystemTime;
use std::collections::{HashMap};

use std::fs::File;

// For now the "Database" is gonna just be a json file

#[derive(Serialize, Deserialize)]
pub struct Entry {
    pub template_path: PathBuf,
    pub config_path: Option<PathBuf>,
    pub destination: PathBuf,
    pub last_updated: SystemTime,
}

pub struct Database {
    fname: PathBuf,
    data: HashMap<PathBuf, Entry>,
}

impl Database {

    pub fn connect(cfg: &crate::config::Config) -> Result<Self> {
        let fname = cfg.backup_path.join("database.json");
        Database::ensure_exists(cfg)?;
        let file = File::open(&fname)
            .chain_err(|| "Couldn't open database file")?;
        let data = serde_json::from_reader(file)
            .chain_err(|| "Error decoding")?;

        Ok(Self {
            fname,
            data,
        })
    }

    fn ensure_exists(cfg: &crate::config::Config) -> Result<()> {
        let fname = cfg.backup_path.join("database.json");
        if !fname.exists() {
            crate::ensure_parent_exists(&fname)?;
            let mut file = File::create(fname)
                .chain_err(|| "Could not open database.json")?;
            write!(file, "{{}}")
                .chain_err(|| "Error writing empty database file")
        }
        else {
            Ok(())
        }
    }

    pub fn commit(&self) -> Result<()> {
        let file = File::create(&self.fname)
            .chain_err(|| "Couldn't overwrite database file")?;
        serde_json::to_writer_pretty(file, &self.data)
            .chain_err(|| "Error writing data")?;

        Ok(())
    }

    pub fn touch(&mut self, path: &PathBuf) -> Result<()> {
        let res = match self.data.get_mut(path) {
            Some(ref mut e) => {
                e.last_updated = SystemTime::now();
                Ok(())
            }
            None => Err("File to be touched is not in database".into())
        };
        self.log_contents();
        res
    }

    pub fn log_contents(&self) {
        let mut res = String::new();
        for (k, v) in self.data.iter() {
            res += &format!("{} -> {:?}\n", k.display(), v.last_updated);
        }
        trace!("Database contents:\n{}", res);
    }

    pub fn add_entry(&mut self, e: Entry) -> Option<Entry> {
        let res = self.data.insert(e.destination.clone(), e);
        self.log_contents();
        res
    }

    pub fn rm_key(&mut self, path: &PathBuf) {
        self.data.remove(path);
        self.log_contents();
    }

    pub fn rm_entry(&mut self, e: Entry) {
        self.data.remove(&e.destination);
        self.log_contents();
    }

    pub fn last_updated(&self, path: &PathBuf) -> Option<SystemTime> {
        self.data.get(path).map(|e| e.last_updated)
    }

    pub fn last_updated_entry(&self, e: Entry) -> Option<SystemTime> {
        self.data.get(&e.destination).map(|x| x.last_updated)
    }
}

// During tests we use temporary database, so its not necessary to preserve it
// Morover, due to dropping order, it can be run after removing temporary folder and panic
#[cfg(not(test))]
impl Drop for Database {
    fn drop(&mut self) {
        debug!("Commiting to database on drop");
        self.commit().unwrap();
    }
}

#[cfg(test)]
mod tests {

}
