#![allow(unused_imports)]
extern crate handlebars;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate structopt;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;
use handlebars::Handlebars;
use std::path::{Path, PathBuf};
use std::error::Error;

use std::io::prelude::*;
use std::fs::File;
use serde_json::Value as Json;
use structopt::StructOpt;

use rotfiles::errors::*;


#[derive(StructOpt)]
enum Rotfiles {
    Add {
        fname: PathBuf
    },
    Update
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
        _ => ()
    }
}

fn run() -> rotfiles::errors::Result<()> {
    pretty_env_logger::init();
    debug!("Program start");

    let cfg = rotfiles::config::Config::from_file("/home/brych/.config/rotfiles/config.json")
        .chain_err(|| "Could not load config.json")?;
    let app = rotfiles::App::from_config(cfg)
        .chain_err(|| "Could not instantiate App")?;

    let rfl = Rotfiles::from_args();
    match rfl {
        Rotfiles::Add{fname} => {
            println!("Adding file: {}", fname.to_string_lossy());
            app.add_file(&fname)
                .chain_err(|| format!("Could not add file {}", fname.display()))?;
        }
        Rotfiles::Update => {
            println!("Updating configuration");
            app.process_all_files()
                .chain_err(|| "Error while updating configuration")?;
        }
    }

    Ok(())
}

