use std::{fmt, sync::mpsc::Receiver};

use sysinfo::{System, SystemExt};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::entry;
use crate::foundation;
use crate::opt::OPT;

// ***********************************************************************************************

#[derive(Debug)]
pub enum WorkerCommand {
    End,
    Request {
        title: String,
        response_tx_handle: mpsc::Sender<WorkerResponse>,
    },
}

#[derive(Debug)]
pub enum WorkerResponse {
    Links, // inbound and outbound links from page in slab
    Fetch, // page is not in slab. Fetching from local cache or wikipedia.com
}

#[derive(Debug)]
pub struct Links {
    pub digest: entry::Digest,
    pub title: String,
    pub outbound: Vec<String>,
    pub inbound: Vec<String>,
}

pub struct Worker {
    worker_id: usize,
    tx_commands: TxCommands,
    rx_command: RxCommand,
}

type Workers = Vec<Worker>;
type TxCommand = mpsc::Sender<WorkerCommand>;
type RxCommand = mpsc::Receiver<WorkerCommand>;
type TxCommands = Vec<TxCommand>;
type RxCommands = Vec<RxCommand>;

/* *****************************************************************************************************************
 *
 * If worker needs to go to fetch, it doesn't wait for the fetch to complete. Instead, it
 *    spawns a new task to fetch the page from store/wikipedia. When this fetch completes, the worker
 *       adds the information to the appropriate slab
 *    immediately returns an "incomplete", indicating that there are additional links that were not returned.
 *       The UI can map that incomplete into a message to the consumer that they should wait a few minutes,
 *       then re-try their request
 *
 *******************************************************************************************************************/

/// Create worker tasks

pub async fn new(foundation: &foundation::Foundation) -> (Vec<JoinHandle<()>>, TxCommands) {
    trace!("worker::new");

    let worker_count: usize = match OPT.get_worker_count() {
        Some(count) => count,
        None => foundation.get_worker_count().try_into().unwrap(),
    };
    let worker_count = foundation.get_worker_count().try_into().unwrap();

    let mut tx_commands: TxCommands = Vec::with_capacity(worker_count);
    let mut rx_commands: RxCommands = Vec::with_capacity(worker_count);
    let mut join_handles: Vec<JoinHandle<()>> = Vec::with_capacity(worker_count);

    // Create the communications mesh. Each worker will hold a Vec with a tx channel to every other
    // worker, and a single tx channel on which it will receive messages from the api service and
    // every other worker service.
    for _ in 0..worker_count {
        let (tx_command, rx_command) = mpsc::channel(worker_count);
        tx_commands.push(tx_command);
        rx_commands.push(rx_command);
    }

    for (worker_id, rx_command) in rx_commands.drain(..).enumerate() {
        join_handles.push(Worker::new_worker(
            worker_id,
            tx_commands.clone(),
            rx_command,
        ));
    }
    (join_handles, tx_commands)
}

pub async fn shut_down(
    join_handles: Vec<JoinHandle<()>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Join long running tasks
    for join_handle in join_handles {
        tokio::try_join!(join_handle)?;
    }
    Ok(())
}

impl Worker {
    fn new_worker(
        worker_id: usize,
        //foundation: &foundation::Foundation,
        tx_commands: TxCommands,
        rx_command: RxCommand,
    ) -> JoinHandle<()> {
        let worker = Worker {
            worker_id,
            tx_commands,
            rx_command,
        };
        tokio::spawn(async move { Worker::worker_service(worker).await })
    }

    async fn worker_service(mut worker: Worker) {
        trace!("worker::worker_service: Spawned worker_service");
        let (response_tx, response_rx): (
            mpsc::Sender<WorkerResponse>,
            mpsc::Receiver<WorkerResponse>,
        ) = mpsc::channel(worker.tx_commands.len());
        loop {
            use WorkerCommand::*;

            let worker_command = worker.rx_command.recv().await.unwrap();
            debug!(
                "worker {}:: Rx command -> {}",
                worker.worker_id, &worker_command
            );
            match worker_command {
                Request {
                    title,
                    response_tx_handle,
                } => Worker::process_request(title, response_tx_handle),
                End => break,
            }
        }
        debug!("Worker {} exiting...", worker.worker_id);
    }

    fn process_request(title: String, response_tx_handle: mpsc::Sender<WorkerResponse>) {
        trace!("worker:process_request for {}", &title);
        let digest = crate::entry::get_digest(&title);

        // get digest for title
        // can title be handled locally?
        //    yes: handle here on this task
        //    no:  panic - it should not have been sent here
        // return if depth == opt::depth
        // increment depth
        // look for the page in slabs
        // page entry exists?
        //    yes: Parse struct Entry: for each inbound and outbound title
        //            send a message to the target worker for links related to the title
        //            on response
        //               if response == entry => add the title to the struct Entry
        //               if response == not found add "not found" to struct Entry
        //            simplify struct entry => eliminate paths when a shorter path already exists
        //            return struct Entry on the response_tx_handle
        //    no:  Send "not found" on response_tx_handle
        //         Send async request to fetch for the page
        //         Add page to slab when fetch responds
    }
}

/*

    let (tx_to_api, rx_by_api): (mpsc::Sender<ApiCommand>, mpsc::Receiver<ApiCommand>) =
        mpsc::channel(tasks);

    let workers: Vec<Worker> = Vec::with_capacity(tasks);

    let api_service = tokio::spawn(async move { api_service(rx_by_api).await });

    (api_service, tx_to_api)
}


pub async fn api_service(mut rx: mpsc::Receiver<ApiCommand>) {
    //pub async fn new() {
    trace!("fetch::new: Spawned fetch");
    loop {
        // listen for message on tx_to_api
        // spawn a new task to process the request
        //    identify target worker
        //    send ApiRequest to target worker
        //    wait for response from target worker
        //    send response on API
        //    ignore any API errors (e.g. timeout)
        //    exit task
        // loop to listen for ...

        use FetchCommand::*;

        let fetch_command = rx.recv().await.unwrap();
        trace!("fetch:: Got command");
        match fetch_command {
            Get { title, tx } => tx.send(get_page_from(&title).await).await.unwrap(),
            End => break,
        }
    }
    trace!("Ending...");
}
*/

impl fmt::Display for WorkerCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            WorkerCommand::End => "End".to_string(),
            WorkerCommand::Request {
                title,
                response_tx_handle: _,
            } => format!("Request:: Title: {}", title),
        };
        write!(f, "{}", msg)
    }
}

/* *****************************************************************************************************************
 *
 * Tests
 *
 * *****************************************************************************************************************/

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_worker() {
        let (mut join_handles, mut tx_handles) =
            new(&foundation::tests::get_test_foundation()).await;

        assert_eq!(join_handles.len(), 16);
        for tx_handle in tx_handles.drain(..) {
            tx_handle.send(WorkerCommand::End).await.unwrap();
        }

        for join_handle in join_handles.drain(..) {
            tokio::try_join!(join_handle).unwrap();
        }
    }
}
