#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate tokio;

mod entry;
mod fetch;
mod foundation;
mod opt;
mod worker;

use std::env;

use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", "six_degrees=trace");
    env_logger::init();

    info!("Getting {} pages deep", opt::OPT.get_depth());
    info!("Caching to {}", opt::OPT.get_cache().to_string_lossy());

    let foundation = foundation::Foundation::new();
    info!("Foundation: {:?}", foundation);

    let (workers, tx_to_workers) = worker::new(&foundation).await;
    let (fetch_service, tx_to_fetch) = fetch::new(&foundation).await;

    // *******
    // Temporary test code starts here

    let (response_tx, response_rx): (
        mpsc::Sender<worker::WorkerResponse>,
        mpsc::Receiver<worker::WorkerResponse>,
    ) = mpsc::channel(1024);
    let request = worker::WorkerCommand::Request {
        title: "Railways".to_string(),
        tx_resp: response_tx.clone(),
    };
    let _ = tx_to_workers[0].send(request).await;

    // During testing, let things stabilize for 5 seconds
    let duration = tokio::time::Duration::new(5, 0);
    tokio::time::sleep(duration).await;

    // Stop long-running tasks
    tx_to_fetch.send(fetch::FetchCommand::End).await.unwrap();
    for tx in tx_to_workers {
        tx.send(worker::WorkerCommand::End).await.unwrap();
    }

    worker::shut_down(workers).await?;

    tokio::try_join!(fetch_service)?;

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
