use std::sync::mpsc::Receiver;

use sysinfo::{System, SystemExt};
use tokio::{sync::mpsc, task::JoinHandle};

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
    pub digest: [u8; 16],
    pub title: String,
    pub outbound: Vec<String>,
    pub inbound: Vec<String>,
}

pub struct Worker {
    worker_id: usize,
    tx_command: TxHandles,
    rx_command: RxHandle,
}

type TxHandle = mpsc::Sender<WorkerCommand>;
type RxHandle = mpsc::Receiver<WorkerCommand>;
type TxHandles = Vec<TxHandle>;
type RxHandles = Vec<RxHandle>;

/* *****************************************************************************************************************
 *
 * Start the api task
 *
 * Holds
 *   Vec<mpsc::tx<<workerCommand>> the transmit to all the workers. The worker_id is the index into the Vec
 *   mpsc::tx<FetchCommand> the transmit to fetch
 *   rx<WorkerCommand> for requests from other workers and API - tokio.await on this
 *
 * In all cases, any request to a worker (or fetch) includes a moved cloned Tx handle that is used for the response
 *
 * the worker_id is the lsb set of bits in the MS5 digest necessary to identify all the tasks (the id is guaranteed to
 * be a power of 2)
 *
 * To determine the target worker:-
 *     do a boolean AND between the worker_id and the (number-of-tasks - 1)
 *     convert that into a u32
 *     that is the index into the Vector of Workers
 *
 * if worker needs to go to fetch, it doesn't wait for the fetch to complete. Instead, it spawns a new task to
 * fetch the page from store/wikipedia and immediatley returns an "incomplete", indicating that there
 * are additional links that were not returned. The UI can map that incomplete into a message to the consumer that
 * they should wait a few minutes, then re-try their request
 *
 *******************************************************************************************************************/

pub async fn new(tasks: usize) -> (Vec<JoinHandle<()>>, Vec<mpsc::Sender<WorkerCommand>>) {
    trace!("worker::new");
    let mut tx_handles: TxHandles = Vec::with_capacity(tasks);
    let mut rx_handles: RxHandles = Vec::with_capacity(tasks);
    let mut join_handles: Vec<JoinHandle<()>> = Vec::with_capacity(tasks);

    for _ in 0..tasks {
        let (tx_handle, rx_handle) = mpsc::channel(tasks);
        tx_handles.push(tx_handle);
        rx_handles.push(rx_handle);
    }

    for (worker_id, rx_command) in rx_handles.drain(..).enumerate() {
        let tx_command = tx_handles.clone();
        let worker = Worker {
            worker_id,
            tx_command,
            rx_command,
        };
        join_handles.push(tokio::spawn(
            async move { worker_service(worker, tasks).await },
        ));
    }

    (join_handles, tx_handles)
}

async fn worker_service(mut worker: Worker, tasks: usize) {
    trace!("worker::worker_service: Spawned worker_service");
    let (response_tx, response_rx): (mpsc::Sender<WorkerResponse>, mpsc::Receiver<WorkerResponse>) =
        mpsc::channel(tasks);
    loop {
        use WorkerCommand::*;

        let worker_command = worker.rx_command.recv().await.unwrap();
        trace!("worker {}:: Got command", worker.worker_id);
        match worker_command {
            Request {
                title,
                response_tx_handle,
            } => process_request(title, response_tx_handle),
            End => break,
        }
    }
    trace!("Ending worker {}...", worker.worker_id);
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

fn process_request(title: String, response_tx_handle: mpsc::Sender<WorkerResponse>) {
    trace!("worker:process_request for {}", &title);
    let digest = crate::fetch::FetchEntry::get_digest(&title);

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
        let (mut join_handles, tx_handles) = new(2).await;
        assert_eq!(join_handles.len(), 2);
        tx_handles[0].send(WorkerCommand::End).await.unwrap();
        tx_handles[1].send(WorkerCommand::End).await.unwrap();

        loop {
            match join_handles.pop() {
                Some(jh) => tokio::try_join!(jh).unwrap(),
                None => break,
            };
        }
    }
}
