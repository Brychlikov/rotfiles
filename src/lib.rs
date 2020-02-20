extern crate handlebars;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate log;
extern crate path_abs;
#[macro_use]
extern crate error_chain;
extern crate glob;
extern crate chrono;
use handlebars::Handlebars;
use std::path::{Path, PathBuf};

use std::io::prelude::*;
use chrono::prelude::*;
use std::fs::File;
use serde_json::Value as Json;
use std::ffi::OsString;

use path_abs::PathAbs;

mod dropfile;
pub mod config;


pub mod errors {
    error_chain! {
        errors {
            JsonConfigError(fname: String) {
                description("Could not load JSON configuration file")
                display("Could not load JSON configuration file for {}", fname)
            }

            NotADotfile(fname: String, home: String) {
                description("File targeted is not a dotfile (not located in $HOME and beggining with '.')"),
                display("File {} is not a dotfile in {} (not located in $HOME and beggining with '.')", fname, home),
            }

            FileNewerThanTemplate(fname: String) {
                description("File has been changed from under its template"),
                display("File {} is newer than its template", fname),
            }

        }
        foreign_links {
            Io(std::io::Error);
            SerdeJson(serde_json::Error);
            HandlebarsTemplate(handlebars::TemplateFileError);
            HandlebarsRender(handlebars::RenderError);
        }
    }
}

use errors::*;

pub struct App {
    cfg: config::Config,

    #[cfg(test)]
    // ensure directory is dropped and cleaned after exit
    _tempdir: Option<tempfile::TempDir>,
}

impl App {

    #[cfg(test)]
    fn new_test<'a>() -> Result<Self> {
        let pseudo_home_dir = tempfile::TempDir::new()
            .chain_err(|| "Can't create temp pseudo home dir")?;
        let home_path = pseudo_home_dir.path().to_owned();
        let dot_path = home_path.join("dotfiles");
        let backup_path = home_path.join(".local/share/rotfiles/backup");

        let res = Self {
            cfg: config::Config::new(home_path, dot_path, backup_path),
            _tempdir: Some(pseudo_home_dir),
        };
        res.ensure_workpath_exists()
            .chain_err(|| "Could not create workpaths")?;
        Ok(res)
    }

    pub fn from_config(cfg: config::Config) -> Result<Self> {
        let res = Self {
            cfg,
            #[cfg(test)]
            _tempdir: None
        };
        res.ensure_workpath_exists()
            .chain_err(|| "Could not create workpaths")?;
        Ok(res)
    }

    fn ensure_workpath_exists(&self) -> std::io::Result<()> {
        let path = &self.cfg.backup_path;
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        let path2 = &self.cfg.dot_path;
        if !path2.exists() {
            std::fs::create_dir_all(path2)?;
        }
        Ok(())
    }

    fn ensure_template_newer_than_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let template_path = path.as_ref();
        let dotfile_path = self.filename_to_dotfile(template_path)
            .chain_err(|| "Can't translate filename to dotfile")?;

        let template_metadata = template_path.metadata()
            .chain_err(|| "Can't access file's metadata")?;
        let dotfile_metadata = dotfile_path.metadata()
            .chain_err(|| "Can't access file's metadata")?;

        let template_mtime = template_metadata.modified()
            .chain_err(|| "Can't access modification time")?;
        let dotfile_mtime = dotfile_metadata.modified()
            .chain_err(|| "Can't access modification time")?;
        
        if dotfile_mtime > template_mtime {
            bail!(ErrorKind::FileNewerThanTemplate(template_path.to_string_lossy().into()));
        }
        Ok(())
    }

    pub fn process_file<P, U>(&self, template_path: P, result_path: U) -> Result<()>
        where P: AsRef<Path>, U: AsRef<Path> {
        let mut handlebars = Handlebars::new();

        self.ensure_template_newer_than_file(&template_path)
            .chain_err(|| "Error comparing modification times")?;

        let json_path: PathBuf = {
            let mut ostring = template_path.as_ref().to_path_buf().into_os_string();
            ostring.push(".json");
            ostring.into()
        };

        debug!("Opening json file on path {:?}", json_path);
        let json_string = read_file(json_path)?;
        let data: Json = serde_json::from_str(&json_string).chain_err(|| ErrorKind::JsonConfigError("Could not read json config".to_string()))?;

        handlebars.register_template_file("file", &template_path)
            .chain_err(|| format!("Could not parse template file: {}", template_path.as_ref().display()))?;

        if result_path.as_ref().exists() {
            self.backup_file(&result_path)?;
        }

        let mut file = File::create(result_path)?;
        debug!("Writing template");
        write!(file, "{}", handlebars.render("file", &data).chain_err(|| "Could not render template")?)?;
        
        Ok(())
    }

    fn backup_file<P>(&self, path: P) -> Result<PathBuf> 
    where P: AsRef<Path> {
        let p = path.as_ref();
        let (mut file_name, ext) = match (p.file_stem(), p.extension()) {
            (Some(x), Some(y)) => (x.to_owned(), y.to_owned()),
            (None, Some(y)) => (OsString::new(), y.to_owned()),
            (Some(x), None) => (x.to_owned(), OsString::new()),
            (None, None) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "No filename on file to backup").into())
        };

        let mut result_path = self.cfg.backup_path.clone();

        file_name.push(format!("{}", Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()));

        result_path.push(file_name);
        result_path.set_extension(ext);

        debug!("Attempting backup of {:?} to {:?}", p, &result_path);
        std::fs::copy(&path, &result_path)
            .chain_err(|| format!("Could not backup {} to {}", path.as_ref().display(), result_path.display()))?;
        debug!("Backup of {:?} to {:?} complete", p, &result_path);

        Ok(result_path)
    }



    pub fn files_to_process(&self) -> impl Iterator<Item=PathBuf> {
        let glob_path = self.cfg.dot_path.to_string_lossy() + "/**/*";
        glob::glob(&glob_path).expect("Incorrect path")
            .filter_map(std::result::Result::ok)  // filter out non-readable files
            // filter out json files
            .filter(|p| p.is_file() && !(p.extension().is_some() && p.extension().unwrap() == "json"))
    }

    fn filename_to_dotfile<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        // dbg!(path.as_ref().display());
        let p = PathAbs::new(path.as_ref())
            .chain_err(|| "Path cannot be canonicalized")?
            .as_path().to_owned();
            
        // dbg!(&p);

        let postfix_string = p.strip_prefix(&self.cfg.dot_path)
            .chain_err(|| ErrorKind::NotADotfile(
                    p.to_string_lossy().into(),
                    self.cfg.home_path.to_string_lossy().into()
                    ))?
            .to_owned().
            into_os_string();
        let dotted = {
            let mut s = OsString::from(".");
             s.push(postfix_string);
             s
        };
        let result_path = self.cfg.home_path.join(dotted);
        Ok(result_path)
    }

    fn dotfile_to_filename<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let p = PathAbs::new(path.as_ref())
            .chain_err(|| "Path cannot be canonicalized")?
            .as_path().to_owned();
        let postfix_string = p.strip_prefix(&self.cfg.home_path)
            .chain_err(|| ErrorKind::NotADotfile
                (
                    p.to_string_lossy().into(),
                    self.cfg.home_path.to_string_lossy().into()
                )
            )?
            .to_owned()
            .into_os_string()
            .into_string()
            .map_err(|_| "Path contains unvalid unicode")?;
        let undotted = match postfix_string.chars().nth(0) {
            Some('.') => &postfix_string[1..],
            _ => bail!(ErrorKind::NotADotfile
                (
                    p.to_string_lossy().into(),
                    self.cfg.home_path.to_string_lossy().into()
                )
            )
        };
        let result_path = self.cfg.dot_path.join(undotted);
        Ok(result_path)
        
    }

    pub fn process_all_files(&self) -> Result<()> {
        for fname in self.files_to_process() {
            debug!("File processing loop entry on {:?}", fname);
            let result_fname = self.filename_to_dotfile(&fname)?;
            let res = self.process_file(&fname, &result_fname);
            match res {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Could not process file: {}->{}\n{}", fname.display(), result_fname.display(), e);
                    for e in e.iter().skip(1) {
                        eprintln!("caused by: {}", e);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn add_file<P: AsRef<Path>> (&self, path: P) -> Result<()> {
        debug!("Adding file {:?}", path.as_ref());
        let result_path = self.dotfile_to_filename(&path)
            .chain_err(|| format!("Could not convert {} to filename to store", path.as_ref().display()))?;
        ensure_parent_exists(&result_path).chain_err(|| format!("Cant create parents of {}", path.as_ref().display()))?;
        debug!("Attempting copy from {:?} to {:?}", path.as_ref(), &result_path);
        std::fs::copy(&path, &result_path)
            .chain_err(|| format!("Could not copy {} to {}", path.as_ref().display(), result_path.display()))?;
        sanitize_file(&result_path).chain_err(|| format!("Error sanitizing {}", result_path.display()))?;

        let json_fname = {
            let mut p = result_path.as_os_str().to_owned();
            p.push(".json");
            p
        };

        let json_value = json!({get_hostname(): true});
        let mut json_file = File::create(json_fname).chain_err(|| "Could not create json file")?;
        write!(json_file, "{}", json_value.to_string()).chain_err(|| "Could not write to json file")?;

        Ok(())
    }

}


fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let mut res = String::new();
    let mut file = File::open(&path).chain_err(|| format!("Could not open file: {}", path.as_ref().display()))?;
    file.read_to_string(&mut res).chain_err(|| format!("Could not read file: {}", path.as_ref().display()))?;
    Ok(res)
}


fn ensure_parent_exists<P: AsRef<Path>>(path: P) -> Result<()> {
    let p = path.as_ref().parent().ok_or("Supplied path has no parent")?;
    std::fs::create_dir_all(p).chain_err(|| "Could not create parent directories")?;
    Ok(())
}


fn sanitize_file<P: AsRef<Path>>(path: P) -> Result<()> {
    let contents = read_file(&path)?.replace("{{", "\\{{").replace("}}", "\\}}");
    let mut file = File::create(&path)
        .chain_err(|| format!("Could not overwrite file {} for sanitization", path.as_ref().display()))?;
    write!(file, "{}", contents)?;
    Ok(())
}


fn get_hostname() -> String {
    let mut result = String::new();
    let mut f = File::open("/etc/hostname").unwrap();
    f.read_to_string(&mut result).unwrap();
    String::from(result.trim_end())
}


#[cfg(test)]
mod tests {
    use super::*;
    extern crate tempfile;

    use dropfile::DropFile;

    fn pretty_err_catcher<F: FnOnce() -> Result<()>>(func: F) {
        match func() {
            Ok(_) => (),
            Err(ref e) => {
                println!("Error encountered: {}", e);
                for e in e.iter().skip(1) {
                    println!("caused by: {}", e);
                }
                panic!("Execution failed");
            }
        }
    }

    #[test]
    fn test_backup() {
        pretty_err_catcher( || {
            let app = App::new_test()?;
            let mut file = tempfile::NamedTempFile::new()
                .chain_err(|| "Can't create temp file")?;
            let orig_content = "This is a test of backup functionality. Some unicode: ąąąćććććęęę";
            write!(file, "{}", orig_content)
                .chain_err(|| "Can't write to temp file")?;
            let location = app.backup_file(file.path()).
                chain_err(|| "Failed to back file up")?;

            let new_content = read_file(location).
                chain_err(|| "Couldnt read backed up file")?;
            

            assert_eq!(orig_content, new_content);
            Ok(())
        });   
    }

    #[test]
    fn test_template_empty_data() {
        pretty_err_catcher(|| {
            let app = App::new_test()?;
            let mut template_file = tempfile::NamedTempFile::new()?;
            let orig_content = "content more original than half of youtube";
            write!(template_file, "{}", orig_content)?;

            let mut json_name = template_file.path().as_os_str().to_owned();
            println!("hello there");
            json_name.push(".json");
            let mut json_file = DropFile::open(json_name).unwrap();
            println!("hello there");
            write!(json_file, "{{}}")?;

            let result_file = tempfile::NamedTempFile::new().unwrap();
            app.process_file(template_file.path(), result_file.path()).unwrap();
            let mut res2 = result_file.reopen().unwrap();

            let mut content = String::new();
            res2.read_to_string(&mut content).unwrap();
            assert_eq!(orig_content, content);
            Ok(())
        });
    }

    #[test]
    fn test_sanitize() {
        let s = "String to be left alone";
        let s2 = "String with daaangerous {{s and }}s";
        let s2_target = r#"String with daaangerous \{{s and \}}s"#;

        let mut orig_file1 = tempfile::NamedTempFile::new().unwrap();
        write!(orig_file1, "{}", s).unwrap();
        sanitize_file(orig_file1.path()).unwrap();
        let mut changed_file = File::open(orig_file1.path()).unwrap();
        let mut result1 = String::new();
        changed_file.read_to_string(&mut result1).unwrap();

        assert_eq!(result1, s);

        let mut orig_file2 = tempfile::NamedTempFile::new().unwrap();
        write!(orig_file2, "{}", s2).unwrap();

        sanitize_file(orig_file2.path()).unwrap();
        changed_file = File::open(orig_file2.path()).unwrap();

        let mut result2 = String::new();
        changed_file.read_to_string(&mut result2).unwrap();

        assert_eq!(result2, s2_target);
    }

    #[test]
    fn test_template() -> Result<()> {
        let app = App::new_test()?;
        let mut template_file = tempfile::NamedTempFile::new()?;
        let target_content = "content more original than half of youtube";
        let orig_content = "content more original than half of {{site}}";
        write!(template_file, "{}", orig_content)?;

        let mut json_name = template_file.path().as_os_str().to_owned();
        println!("hello there");
        json_name.push(".json");
        let mut json_file = DropFile::open(json_name).unwrap();
        println!("hello there");
        write!(json_file, r#"{{"site": "youtube"}}"#)?;

        let result_file = tempfile::NamedTempFile::new().unwrap();
        app.process_file(template_file.path(), result_file.path()).unwrap();
        let mut res2 = result_file.reopen().unwrap();

        let mut content = String::new();
        res2.read_to_string(&mut content).unwrap();
        assert_eq!(target_content, content);

        Ok(())
    }

    #[test]
    fn test_filename_to_dotfile() {
        pretty_err_catcher(|| {
            let app = App::new_test()?;
            let home = &app.cfg.home_path;
            let cases = vec![
                (home.join("dotfiles/zshrc"), home.join(".zshrc")),
                (home.join("dotfiles/config/nvim/init.vim"), home.join(".config/nvim/init.vim"))
            ];

            for (case, res) in cases {
                assert_eq!(
                    res,
                    app.filename_to_dotfile(case).chain_err(|| "Couldn't translate filename to dotfile")?
                );
            }
            Ok(())
        });
    }

    #[test]
    fn test_dotfile_to_filename() {
        pretty_err_catcher(|| {
            let app = App::new_test()
                .chain_err(|| "Could instantiate app")?;
            let home = &app.cfg.home_path;

            let cases = vec![
                (home.join(".zshrc"), home.join("dotfiles/zshrc")),
                (home.join(".config/nvim/init.vim"), home.join("dotfiles/config/nvim/init.vim"))
            ];

            for (case, res) in cases {
                assert_eq!(
                    res,
                    app.dotfile_to_filename(case).chain_err(|| "Couldnt translete dotfile to filename")?
                );
            }

            Ok(())
        });
    }

    #[test]
    fn test_dotfile_identity() {
        pretty_err_catcher( || {
            let app = App::new_test().unwrap();
            let home = &app.cfg.home_path;
            for case in [home.join("dotfiles/zshrc"), home.join("dotfiles/config/nvim/init.vim")].iter() {
                assert_eq!(
                    *case, 
                    app.dotfile_to_filename(
                        app.filename_to_dotfile(case)
                            .chain_err(|| "Unable to translate filename to dotfile")?)
                        .chain_err(|| "Unable to translate dotfile to filename")?
                );
            }
            Ok(())
        });
    }
}
