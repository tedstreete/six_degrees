#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate tokio;

mod fetch;
mod opt;
mod worker;

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
    info!("Slabs: {:?}", get_slab_count());

    /*  let (fetch_service, tx_to_fetch) = fetch::new(*TASKS).await;

    // set-up workers
    let (worker_services, worker_tx_handles) = worker::new(*TASKS).await;

    // initialize API

    // During testing, let things stabilize for 5 seconds
    let duration = tokio::time::Duration::new(5, 0);
    tokio::time::sleep(duration).await;

    // Stop long-running tasks
    tx_to_fetch.send(fetch::FetchCommand::End).await.unwrap();
    for handle in worker_tx_handles {
        handle.send(worker::WorkerCommand::End).await.unwrap();
    }

    // Join long running tasks
    for worker_service in worker_services {
        tokio::try_join!(worker_service).unwrap();
    }

    match tokio::try_join!(fetch_service) {
        Ok(_) => Ok(()),
        Err(err) => {
            error!("Task exit failed with {}.", err);
            Err(err.into())
        }
    }
    */
    Ok(())
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

fn get_slab_count() -> usize {
    let total_memory = SYSTEM.total_memory();
    let tasks = get_task_count();
    let working_memory = 1024 * 1024 * 1024; // Allow 1GB for execution and working memory
    let tx_handle_count = 8 * tasks; // 8 bytes per handle
                                     // TODO Valildate average message size
    let message_size = 1024 * tasks; // Average message size of 1k
    let tokio_task_cache = 64 * tasks;
    let reserved_memory: u64 =
        (working_memory + tx_handle_count + message_size + tokio_task_cache) as u64;
    let memory_for_slabs = total_memory - reserved_memory;

    let slabs = memory_for_slabs / (1024 * 1024); // Each slab is 1MB
    5
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
