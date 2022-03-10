#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate tokio;

mod fetch;
mod foundation;
mod opt;
mod worker;

use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", "six_degrees=trace");
    env_logger::init();

    info!("Getting {} pages deep", opt::OPT.get_depth());
    info!("Caching to {}", opt::OPT.get_cache().to_string_lossy());
    let foundation = foundation::Foundation::new();

    info!("Foundation: {:?}", foundation);

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

/* *****************************************************************************************************************
 *
 * Tests
 *
 * *****************************************************************************************************************/

#[cfg(test)]
mod tests {
    use super::*;
}
