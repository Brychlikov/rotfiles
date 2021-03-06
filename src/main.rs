extern crate handlebars;
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;
use std::path::PathBuf;

use structopt::StructOpt;

use rotfiles::errors::*;

#[derive(StructOpt)]
enum Rotfiles {
    Add { fname: PathBuf },
    Update,
    Edit { fname: PathBuf },
    Remove { fname: PathBuf },
}

fn main() {
    match run() {
        Err(ref e) => {
            eprintln!("Error: {}", e);
            for e in e.iter().skip(1) {
                eprintln!("caused by: {}", e);
            }
            std::process::exit(1);
        }
        _ => (),
    }
}

fn run() -> rotfiles::errors::Result<()> {
    pretty_env_logger::init();
    debug!("Program start");

    let cfg = rotfiles::config::Config::from_file("/home/brych/.config/rotfiles/config.json")
        .chain_err(|| "Could not load config.json")?;
    let mut app = rotfiles::App::from_config(cfg).chain_err(|| "Could not instantiate App")?;

    let rfl = Rotfiles::from_args();
    match rfl {
        Rotfiles::Add { fname } => {
            println!("Adding file: {}", fname.to_string_lossy());
            app.add_file(&fname, false)
                .chain_err(|| format!("Could not add file {}", fname.display()))?;
        }
        Rotfiles::Update => {
            println!("Updating configuration");
            app.process_all_files()
                .chain_err(|| "Error while updating configuration")?;
        }
        Rotfiles::Edit { fname } => {
            println!("Editing file: {}", fname.display());
            app.edit_file(&fname)?;
        }
        Rotfiles::Remove { fname } => {
            app.remove_file(&fname)?;
        }
    }

    Ok(())
}
