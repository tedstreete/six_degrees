#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

mod fetch;
mod opt;

use std::{
    env,
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc,
    thread, time,
};

fn main() {
    env::set_var("RUST_LOG", "six_degrees=trace");
    env_logger::init();

    info!("Getting {} pages deep", opt::OPT.get_depth());
    info!("Caching to {}", opt::OPT.get_cache().to_string_lossy());
}
