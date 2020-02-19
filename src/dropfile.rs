use std::fs::{File};
use std::path::{Path, PathBuf};
use std::ops::{Deref, DerefMut};

pub struct DropFile {
    path: PathBuf,
    file: File
}

impl DropFile {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<DropFile> {
        dbg!(path.as_ref());
        if path.as_ref().exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("{:?} already exists", path.as_ref())))
        }
        let file = File::create(path.as_ref())?;
        println!("Got there");
        Ok(DropFile {
            path: PathBuf::from(path.as_ref()),
            file
        })
    }
}

impl Deref for DropFile {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl DerefMut for DropFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file
    }
}

impl Drop for DropFile {
    fn drop(&mut self) {
        // drop(self.file);
        std::fs::remove_file(&self.path).expect("Dropfile could not remove its file");
    }
}
