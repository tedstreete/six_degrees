#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate tokio;

mod fetch;
mod opt;

use std::env;
use sysinfo::{System, SystemExt};
//use tokio::sync::mpsc;

lazy_static! {
    static ref SYSTEM: System = {
        let mut sys = System::new_all();
        sys.refresh_all();
        sys
    };
}
fn main() {
    env::set_var("RUST_LOG", "six_degrees=trace");
    env_logger::init();

    info!("Getting {} pages deep", opt::OPT.get_depth());
    info!("Caching to {}", opt::OPT.get_cache().to_string_lossy());
    info!("Tasks: {:?}", get_task_count());

    let page = fetch::get_page_from("Rail transport").unwrap();
}

// The number of tasks is determined from (system_memory{<MB>} ÷ 60) rounded down to next power of 2
fn get_task_count() -> u32 {
    let total_memory = SYSTEM.total_memory();
    let mut tasks = (total_memory / 1024) / 60;

    // Round-down to next power of two
    let mut power: u64 = 1;
    while power < tasks {
        power *= 2;
    }

    (power / 2).try_into().unwrap()
}
