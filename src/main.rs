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

mod config;

#[derive(StructOpt)]
enum Rotfiles {
    Add {
        fname: PathBuf
    },
    Update
}


fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    debug!("Program start");
    error!("Program start");


    let rfl = Rotfiles::from_args();
    match rfl {
        Rotfiles::Add{fname} => {
            println!("Adding file: {}", fname.to_str().unwrap());
            if let Err(e) = rotfiles::add_file(fname) {
                eprintln!("Could not add file: {}", e);
            }
        }
        Rotfiles::Update => {
            println!("Updating configuration");
            if let Err(e) = rotfiles::process_all_files() {
                eprintln!("Error while updating configuration: {}", e);
            }
        }
    }

    // let cfg = config::Config::from_file("config.json");
    // println!("{:?}", cfg);

    // for i in rotfiles::files_to_process() {
    //     println!("{:?}", i);
    // }
    // rotfiles::process_all_files()?;

    // println!("Hello, world!");
    // rotfiles::process_file("/home/brych/dotfiles/test", "/home/brych/test")?;
    // println!("Got past first one");
    // rotfiles::add_file("/home/brych/.zshrc")?;
    
    Ok(())
}

