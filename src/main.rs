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

lazy_static! {
    static ref SYSTEM: System = {
        let mut sys = System::new_all();
        sys.refresh_all();
        sys
    };
    static ref TASKS: usize = get_task_count();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", "six_degrees=trace");
    env_logger::init();

    info!("Getting {} pages deep", opt::OPT.get_depth());
    info!("Caching to {}", opt::OPT.get_cache().to_string_lossy());
    info!("Tasks: {:?}", get_task_count());

    let (fetch_service, tx_to_fetch) = fetch::new(*TASKS).await;
    // initialize API
    // set-up workers

    tx_to_fetch.send(fetch::FetchCommand::End).await.unwrap();

    match tokio::try_join!(fetch_service) {
        Ok(_) => Ok(()),
        Err(err) => {
            error!("Task exit failed with {}.", err);
            Err(err.into())
        }
    }
}

fn get_task_count() -> usize {
    // The number of tasks is determined from (system_memory{<MB>} ÷ 60) rounded down to next power of 2
    let total_memory = SYSTEM.total_memory();
    let raw_tasks = (total_memory / 1024) / 60;

    // Round-down to next power of two
    let mut tasks: u64 = 1;
    while tasks < raw_tasks {
        tasks *= 2;
    }

    (tasks / 2).try_into().unwrap()
}

/* *****************************************************************************************************************
 *
 * Tests
 *
 * *****************************************************************************************************************/

#[cfg(test)]
mod tests {
    use super::*;
}
