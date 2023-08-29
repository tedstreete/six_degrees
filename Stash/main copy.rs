#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate tokio;

mod fetch;
mod opt;

use core::time;
use std::{env, thread, time::Duration};
use sysinfo::{System, SystemExt};
use tokio::{process::Command, sync::mpsc, task::JoinHandle};

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

    let (fetch, tx_to_fetch) = init_fetch().await;

    Ok(())
}

async fn init_fetch() -> (JoinHandle<()>, mpsc::Sender<fetch::FetchCommand>) {
    let (tx_to_fetch, mut rx_by_fetch): (
        mpsc::Sender<fetch::FetchCommand>,
        mpsc::Receiver<fetch::FetchCommand>,
    ) = mpsc::channel(*TASKS);

    trace!("about to spawn thread");

    let fetch = tokio::spawn(async move { fetch::new(rx_by_fetch).await });

    //    let fetch = thread::spawn(|| fetch::new());
    //  let fetch = thread::spawn(|| println!("Waiting for command"));

    trace!("spawned thread");

    let (tx_by_fetch, mut rx_from_fetch): (
        mpsc::Sender<Result<fetch::FetchEntry, fetch::FetchError>>,
        mpsc::Receiver<Result<fetch::FetchEntry, fetch::FetchError>>,
    ) = mpsc::channel(1);

    trace!("sending message then sleeping 1 sec");

    tokio::time::sleep(Duration::from_millis(1000)).await;

    tx_to_fetch
        .send(fetch::FetchCommand::Get {
            title: String::from("test message to fetch"),
            tx: tx_by_fetch.clone(),
        })
        .await
        .unwrap();
    //   let page = fetch::get_page_from("Rail transport").unwrap();

    trace!("Sent message. Sleeping 1 sec before reading");
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let response = rx_from_fetch.recv().await.unwrap();
    match response {
        Ok(response) => info!("Response: {}", response.title),
        Err(_) => todo!(),
        //        fetch::FetchResponse::Get { title } => info!("Response from fetch: {}", title),
        //        fetch::FetchResponse::Set { key, val } => todo!(),
    }

    trace!("Got response 1");

    (fetch, tx_to_fetch)
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
