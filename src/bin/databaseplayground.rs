use rotfiles::database::*;
use rotfiles::errors::*;
#[macro_use]
extern crate error_chain;
use std::time::SystemTime;

fn run() -> Result<()> {
    println!("Hello additional binary!");
    let cfg = rotfiles::config::Config::from_file("/home/brych/.config/rotfiles/config.json")?;

    let mut db = Database::connect(&cfg)?;
    let entry = make_test_entry(&cfg, "testfile");
    db.add_entry(entry);
    std::thread::sleep(std::time::Duration::from_secs(1));
    db.add_entry(make_test_entry(&cfg, "config/innertest"));
    db.commit()?;
    Ok(())
}

fn make_test_entry(cfg: &rotfiles::config::Config, subpath: &str) -> Entry {
    Entry {
        template_path: cfg.dot_path.join(subpath),
        config_path: None,
        destination: cfg.home_path.join(String::from(".") + subpath),
        last_updated: SystemTime::now(),
    }
}
quick_main!(run);
