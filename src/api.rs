use std::{
    cmp::{max, min},
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    process,
};
use url::form_urlencoded::parse;

use tokio::{
    sync::mpsc::{self, Sender},
    task::JoinHandle,
};

//use hyper::service::{make_service_fn, service_fn};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use regex::Regex;

//use crate::fetch::FetchCommand;
use crate::fetch;
use crate::opt::OPT;

static DEAFULT_API_PORT: u16 = 6457;
static DEFAULT_MANAGEMENT_PORT: u16 = 6458;

lazy_static! {
    static ref DEFAULT_API_SOCKET: SocketAddr =
        std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEAFULT_API_PORT));
}

// static DEFAULT_SOCKET: SocketAddr =
//     std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, API_PORT));

// ***********************************************************************************************

#[derive(Debug)]
enum StartFrom {
    title(String),
    url(String),
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ApiCommand {
    End,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ApiRequest {
    pub title: String,
}

struct Worker {
    worker_id: u32,
    tx_to_worker: mpsc::Sender<ApiRequest>,
}
/* *****************************************************************************************************************
 *
 * Start the api task
 *
 * Kick off a new task (assembler) that
 *    - queries the first worker
 *    - queries the descendent workers until foundation.depth is reached
 * This keeps the workers clean. They pull messages and
 *    - provide an Entry if one exists in their slabl
 *    - returns <Fetch> if no Entry exists
 *    - add/update and entry from the fetch process
 *
 * If a worker returns <Fetch> to assembler then this means the requested Entry is not available. The assembler task
 * must call fetch to have the Entry created, either from the local cache, or from Wikipedia. Fetch will call the
 * target worker directly with the Entry, using an <update> request.
 *
 * In the event that one or more workers return <Fetch>, the assembler task will wait 20 seconds then retry all the
 * entries for which it received a <Fetch> response. Any responses that still return <Fetch> will be returned as a
 * Fetch warning to the client
 *
 * API has one endpoint
 *    /title: look for a page with the title in wikipedia. Title must be appropriately encoded to avoid white space or
 *            other illegal characters
 *
 *******************************************************************************************************************/

// pub async fn new(tx_to_fetch: Sender<FetchCommand>) -> JoinHandle<()> {
pub async fn new(tx_to_fetch: Sender<fetch::FetchCommand>) {
    trace!("api::new");
    //   let api_service = tokio::spawn(async move { start_api_service() }).await;
    start_api_service();
    trace!("api: REST server started");
    // let api_service = tokio::spawn(async move { api_service(tx_to_fetch).await });

    //    api_service
}

fn start_api_service() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    trace!("api::start_api_service");
    let addr = get_api_address();
    info!("Addr: {:?}", addr);

    // let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(api_service)) });
    // let server = Server::bind(&addr).serve(service);
    // info!("Listening on http://{}", addr);
    // server.await?;
    // info!("API Shutting down");

    Ok(())
}

pub async fn api_service(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    if req.method() == &Method::GET {
        println!("method::GET");
        let path = req.uri().path();
        println!("Path: {}", path);

        let components: Vec<&str> = path.split('/').collect();
        // Three components only
        // 1. The characters before the leading '/'. This will be empty
        // 2. The string 'connections'
        if components.len() != 2 || components[1].to_ascii_lowercase() != "connections" {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            let message = format!("Nothing found at {}", &path);
            *not_found.body_mut() = Body::from(message);
            return Ok(not_found);
        }

        // Extract query options from uri
        // From: https://users.rust-lang.org/t/using-hyper-how-to-get-url-query-string-params/23768/2

        let params: HashMap<String, String> = req
            .uri()
            .query()
            .map(|v| {
                url::form_urlencoded::parse(v.as_bytes())
                    .into_owned()
                    .collect()
            })
            .unwrap_or_else(HashMap::new);

        let depth = max(
            min(
                params
                    .get("depth")
                    .unwrap_or(&"2".to_string())
                    .parse()
                    .unwrap_or(2),
                6,
            ),
            1,
        );

        let root;
        if params.contains_key("title") {
            root = Some(StartFrom::title(params.get("title").unwrap().to_string()))
        } else if params.contains_key("url") {
            root = Some(StartFrom::url(params.get("url").unwrap().to_string()))
        } else {
            root = None
        }

        let body = format!("Depth: {}\nRoot:   {:?}", depth, &root);
        let body = Body::from(body);
        return Ok(Response::new(body));
    }
    let mut not_found = Response::default();
    *not_found.status_mut() = StatusCode::NOT_FOUND;
    return Ok(not_found);
}
// listen for message on tx_to_api
// spawn a new task "assembler" to process the request
//    identify target worker
//    send request to target worker
//    get response from target worker
//    if response is <Fetch>
//       add  <Fetch> response to  retry vector
//       send message to fetch, to have the entry pulled from cache or wikipedia
//    if <Fetch> vector has any entries
//       wait 20 seconds
//       attempt to get an entry for each element in the vector
//    Assemble a response
//    send response on API
//    ignore any API errors (e.g. timeout)
//    exit task
// loop to listen for ...

//   trace!("API ending...");

fn get_api_address() -> SocketAddr {
    //
    // let address = get_address(&add);

    let socket = match OPT.get_api() {
        Some(api_target) => get_address(&api_target),
        None => *DEFAULT_API_SOCKET,
    };

    println!("Socket Address: {:?}", socket);
    std::process::exit(1);

    socket
}

fn get_address(addr: &str) -> SocketAddr {
    match try_v4_address(addr) {
        Some(socket) => socket,
        // None => match try_v6_address {some return v6addr none;return DEFAULT_SOCKET }
        None => *DEFAULT_API_SOCKET,
    }
}

fn try_v4_address(address_from_command_line: &str) -> Option<SocketAddr> {
    let v4_match =
        Regex::new(r"((\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3}))?(:(\d{1,5}))?").unwrap();

    if !v4_match.is_match(address_from_command_line) {
        return None;
    }

    let mut address_builder: Vec<u8> = Vec::with_capacity(4);
    let mut address;
    let caps = v4_match.captures(address_from_command_line).unwrap();
    if caps.get(1).is_some() {
        for x in 2..6 {
            if caps.get(x).is_some() {
                println!("Group: {} contains {:?}", x, caps.get(x).unwrap().as_str());
                let octet: u16 = caps.get(x).unwrap().as_str().parse::<u16>().unwrap();
                if (octet > 255) {
                    panic!(
                        "IPv4 address should use octets in the range 0-255. Found {} in address.",
                        octet
                    );
                }
                address_builder.push(octet.try_into().unwrap());
            }
        }
        address = Ipv4Addr::new(
            address_builder[0],
            address_builder[1],
            address_builder[2],
            address_builder[3],
        );
    }

    if caps.get(7).is_some() {
        println!("Group: {} contains {:?}", 7, caps.get(7).unwrap().as_str());
    }
    Some(*DEFAULT_API_SOCKET)
}

/* *****************************************************************************************************************
 *
 * Tests
 *
 * *****************************************************************************************************************/

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn test_api_v4_success() {
        let address =
            std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEAFULT_API_PORT));
        assert_eq!(get_address("192.168.1.2:3303"), address);
    }

    #[test]
    fn test_api_v4_address_only_success() {
        let address =
            std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEAFULT_API_PORT));
        assert_eq!(get_address("192.168.1.2"), address);
    }

    #[test]
    fn test_api_v4_port_only_success() {
        let address =
            std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEAFULT_API_PORT));
        assert_eq!(get_address(":3303"), address);
    }

    #[test]
    #[should_panic]
    fn test_api_v4_address_octet_too_large_fail() {
        let address =
            std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEAFULT_API_PORT));
        assert_eq!(get_address("266.168.1.2:3303"), address);
    }

    #[test]
    #[should_panic]
    fn test_api_v4_port_too_large_fail() {
        let address =
            std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEAFULT_API_PORT));
        assert_eq!(get_address("192.168.1.2:67034"), address);
    }

    /*  Tests

    1. valid v4 with port
    2. valid v4 without port
    3. valid v4 port only
    4. octet greater than 255
    5. port greater than 65536
    1. valid v6 with port
    valid v6 with shorthand notation and with port
    2. valid v6 without port
    3.  v6 port only should prov ide v6 localhost ie "[]:4010" should use port 4010 on v6Localhost
    4. octet greater than 255
    5. port greater than 65536
    */
}
