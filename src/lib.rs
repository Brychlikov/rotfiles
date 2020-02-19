extern crate handlebars;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
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

mod dropfile;
mod config;



const DOTPATH: &str = "/home/brych/dotfiles";
const HOMEPATH: &str = "/home/brych";
// const WORKPATH: &str = "~/.local/share/rotfiles";
//


pub mod errors {
    error_chain! {
        errors {
            JsonConfigError(fname: String) {
                description("Could not load JSON configuration file")
                display("Could not load JSON configuration file for {}", fname)
            }

            NotADotfile {
                description("File targeted is not a dotfile (not located in $HOME and beggining with '.')")
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

lazy_static! {
    static ref WORKPATH: &'static str = {
        match ensure_workpath_exists() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Could not access nor create directory ~/.local/share/rotfiles\n{}", e);
                std::process::exit(1);
            }
        }       
    };

    
}

#[cfg(test)]
lazy_static! {
    static ref BACKUPDIR: tempfile::TempDir = tempfile::TempDir::new().unwrap();
    static ref BACKUPPATH: &'static str = {
        let p = BACKUPDIR.path();
        p.to_str().unwrap()
    };
}

#[cfg(not(test))]
lazy_static! {
    static ref BACKUPPATH: &'static str = *WORKPATH;
}

fn ensure_workpath_exists() -> std::io::Result<&'static str> {
    let s = "/home/brych/.local/share/rotfiles";
    let path = Path::new(s);
    if !path.exists() {
        std::fs::create_dir(path)?;
    }
    Ok(&s)
}

fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let mut res = String::new();
    let mut file = File::open(&path).chain_err(|| format!("Could not open file: {}", path.as_ref().display()))?;
    file.read_to_string(&mut res).chain_err(|| format!("Could not read file: {}", path.as_ref().display()))?;
    Ok(res)
}

pub fn process_file<P, U>(template_path: P, result_path: U) -> Result<()>
    where P: AsRef<Path>, U: AsRef<Path> {
    let mut handlebars = Handlebars::new();

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
        backup_file(&result_path)?;
    }

    let mut file = File::create(result_path)?;
    debug!("Writing template");
    write!(file, "{}", handlebars.render("file", &data).chain_err(|| "Could not render template")?)?;
    
    Ok(())
}

fn backup_file<P>(path: P) -> Result<PathBuf> 
where P: AsRef<Path> {
    let p = path.as_ref();
    let (mut file_name, ext) = match (p.file_stem(), p.extension()) {
        (Some(x), Some(y)) => (x.to_owned(), y.to_owned()),
        (None, Some(y)) => (OsString::new(), y.to_owned()),
        (Some(x), None) => (x.to_owned(), OsString::new()),
        (None, None) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "No filename on file to backup").into())
    };

    let mut result_path = PathBuf::from(*BACKUPPATH);

    file_name.push(format!("{}", Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()));

    result_path.push(file_name);
    result_path.set_extension(ext);

    debug!("Attempting backup of {:?} to {:?}", p, &result_path);
    std::fs::copy(&path, &result_path)
        .chain_err(|| format!("Could not backup {} to {}", path.as_ref().display(), result_path.display()))?;
    debug!("Backup of {:?} to {:?} complete", p, &result_path);

    Ok(result_path)
}

pub fn add_file<P: AsRef<Path>> (path: P) -> Result<()> {
    debug!("Adding file {:?}", path.as_ref());
    let result_path = dotfile_to_filename(&path)
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

pub fn files_to_process() -> impl Iterator<Item=PathBuf> {
    let glob_path = DOTPATH.to_owned() + "/**/*";
    glob::glob(&glob_path).expect("Incorrect path")
        .filter_map(std::result::Result::ok)  // filter out non-readable files
        .filter(|p| p.is_file() && !(p.extension().is_some() && p.extension().unwrap() == "json")) // filter out json files
}

fn filename_to_dotfile<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    dbg!(path.as_ref().display());
    let p = path.as_ref().canonicalize().chain_err(|| "Path cannot be canonicalized")?;
    dbg!(&p);
    let postfix_string = p.strip_prefix(DOTPATH).chain_err(|| ErrorKind::NotADotfile)?.to_owned().into_os_string();
    let dotted = {
        let mut s = OsString::from(".");
         s.push(postfix_string);
         s
    };
    let result_path = Path::new(HOMEPATH).join(dotted);
    Ok(result_path)
}

fn dotfile_to_filename<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let p = path.as_ref().canonicalize().chain_err(|| "Path cannot be canonicalized")?;
    let postfix_string = p.strip_prefix(HOMEPATH).chain_err(|| ErrorKind::NotADotfile)?.to_owned().into_os_string().into_string().map_err(|_| "Path contains unvalid unicode")?;
    let undotted = match postfix_string.chars().nth(0) {
        Some('.') => &postfix_string[1..],
        _ => bail!(ErrorKind::NotADotfile)
    };
    let result_path = Path::new(DOTPATH).join(undotted);
    Ok(result_path)
    
}

pub fn process_all_files() -> Result<()> {
    for fname in files_to_process() {
        debug!("File processing loop entry on {:?}", fname);
        let result_fname = filename_to_dotfile(&fname)?;
        let res = process_file(&fname, &result_fname);
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

#[cfg(test)]
mod tests {
    use super::*;
    extern crate tempfile;

    use dropfile::DropFile;

    #[test]
    fn test_backup_func() -> Result<()> {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let orig_content = "This is a test of backup functionality. Some unicode: ąąąćććććęęę";
        write!(file, "{}", orig_content).unwrap();
        let location = backup_file(file.path());

        let mut new_file = File::open(location.unwrap()).unwrap();
        let mut new_content = String::new();
        new_file.read_to_string(&mut new_content).unwrap();

        assert_eq!(orig_content, new_content);
        Ok(())
    }

    #[test]
    fn test_template_empty_data() -> Result<()> {
        let mut template_file = tempfile::NamedTempFile::new()?;
        let orig_content = "content more original than half of youtube";
        write!(template_file, "{}", orig_content)?;

        let mut json_name = template_file.path().as_os_str().to_owned();
        println!("hello there");
        json_name.push(".json");
        let mut json_file = DropFile::open(json_name).unwrap();
        println!("hello there");
        write!(json_file, "{{}}")?;

        let mut result_file = tempfile::NamedTempFile::new().unwrap();
        process_file(template_file.path(), result_file.path()).unwrap();
        let mut res2 = result_file.reopen().unwrap();

        let mut content = String::new();
        res2.read_to_string(&mut content).unwrap();
        assert_eq!(orig_content, content);

        Ok(())
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

        let mut result_file = tempfile::NamedTempFile::new().unwrap();
        process_file(template_file.path(), result_file.path()).unwrap();
        let mut res2 = result_file.reopen().unwrap();

        let mut content = String::new();
        res2.read_to_string(&mut content).unwrap();
        assert_eq!(target_content, content);

        Ok(())
    }

    #[test]
    fn test_filename_to_dotfile() -> Result<()> {
        assert_eq!(filename_to_dotfile("/home/brych/dotfiles/zshrc")?, PathBuf::from("/home/brych/.zshrc"));
        assert_eq!(filename_to_dotfile("/home/brych/dotfiles/config/nvim/init.vim")?, PathBuf::from("/home/brych/.config/nvim/init.vim"));
        Ok(())
    }

    #[test]
    fn test_dotfile_to_filename() {
        assert_eq!(PathBuf::from("/home/brych/dotfiles/zshrc"), dotfile_to_filename("/home/brych/.zshrc").unwrap());
        assert_eq!(PathBuf::from("/home/brych/dotfiles/config/nvim/init.vim"), dotfile_to_filename("/home/brych/.config/nvim/init.vim").unwrap());
    }

    #[test]
    fn test_dotfile_identity() {
        for case in ["/home/brych/dotfiles/zshrc", "/home/brych/dotfiles/config/nvim/init.vim"].iter() {
            assert_eq!(PathBuf::from(case), dotfile_to_filename(filename_to_dotfile(case).unwrap()).unwrap());
        }
    }
}
