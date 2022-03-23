/************************************************************************************************
 *
 * ---------------
 * Wikipedia notes
 * ---------------
 *
 * In keeping with the wikimedia API best practices (https://www.mediawiki.org/wiki/API:Etiquette), this status_code
 * runs the thread single thread, and uses the reqwest blocking client, thereby ensuring that requests to the wiki API
 * can never overlap (at least from a single session).
 *
 * Use GZip compression when making API calls (Accept-Encoding: gzip). Bots eat up a lot of bandwidth,
 *   which is not free.
 *
 * Set a descriptive User Agent header (User-Agent: User name/email/framework/...). Include your username and
 *   wiki or email address.
 *
 * Minimize the number of API calls by asking for multiple items in one request. Use titles=PageA|PageB|PageC
 *   and get all the needed lists and properties at the same time. Only ask for what is actually needed. (This
 *   option is not availble for the 'parse' action).
 *
 * Resources:
 * Query documentation is at:- https://www.mediawiki.org/wiki/API:Query
 * Parse documentation is at:- https://www.mediawiki.org/wiki/API:Parsing_wikitext
 * Some attributes are documented at:- https://www.mediawiki.org/wiki/Manual:Database_layout
 * Sandbox for testing queries is at: https://en.wikipedia.org/wiki/Special:ApiSandbox
 *
 * Test pages
 * https://en.wikipedia.org/w/api.php?action=parse&format=json&page=supermarine&prop=links
 *
 *************************************************************************************************
 *
 * --------------------
 * Errors and responses
 * --------------------
 *
 * Network error:                   Return FetchError::IO(std::io::Error)
 * MaxLag: Wait, then try again:    Return FetchError::Lag(String) after LAG_DEFERRAL attempts
 * PageNotFound:                    Return FetchError::PageNotFound(String)
 * Unable to parse JSON:            Return FetchError::Parse(String)
 *
 *************************************************************************************************
 *
 * Aging Policy
 * ------------
 *
 * Pages that parse successfully: Calculated from page last update time (Min 7 days)
 * Pages that are not found:      7 days
 *
 *************************************************************************************************/

/*************************************************************************************************
 *
 * Loop
 *    Wait for request on mpsc_receive
 *    Parse request: convert title to url if necessary (does request start with "http(s)://")
 *    Loop until request = 5
 *       Request page
 *          Network error -> return FetchError::IO(std::io::Error)
 *          Lag error loop until request == 5
 *             request == 5 -> return FetchError::Lag(String)
 *    Fetch successful
 *    Save page to cache - Save in folder hierarchy based on 16 LSB: 256 dirs, each holding 256 dirs
 *    Parse page
 *       Page not found error - return FetchError::PageNotFound(String)
 *       Parse error -> Return FetchError::Parse(String)
 *    Parse successful
 *       return Success(struct Entry)
 *
 *************************************************************************************************/

use crate::entry;
use crate::foundation;
use reqwest::{blocking, header::HeaderValue, StatusCode, Url};
use tokio::{sync::mpsc, task::JoinHandle};

use std::{
    fmt,
    fs::{self, create_dir_all},
    io,
    path::PathBuf,
};

use crate::opt;

lazy_static! {
    static ref ATTRIBUTES_FOR_PAGE: Vec<(&'static str, &'static str)> = {
        let mut v = Vec::with_capacity(3);
        v.push(("action", "parse"));
        v.push(("format", "json"));
        v.push(("prop", "links"));
        v.push(("maxlag", MAXLAG));
        v
    };
    static ref CLIENT: blocking::Client = {
        let user_agent = HeaderValue::from_str("SixDegrees/0.1 sixdegrees@streete.net")
            .expect(&"Internal error parsing USER_AGENT value in wikipedia::init()");
        reqwest::blocking::Client::builder()
            .gzip(true)
            .user_agent(user_agent)
            .build()
            .expect("Internal error creating fetch::client")
    };
    static ref URL: String = {
        let mut url = opt::OPT.get_domain_name().to_string();
        url.push_str(PATH);
        url
    };
    static ref MAXLAG_VALUE: u64 = MAXLAG.parse().unwrap();
}

static MAXLAG: &'static str = "5";
static PATH: &'static str = "/w/api.php";
static PARSE_ERROR: &'static str = "Unknown wikipedia payload";

// ***********************************************************************************************

// ***********************************************************************************************

// JSON used on Wikipedia response

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub ns: i32,
    pub exists: Option<String>,
    #[serde(rename = "*")]
    pub title: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    pub title: String,
    pub pageid: u32,
    pub links: Vec<Link>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Page {
    parse: Links,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct MaxLagFrame {
    code: String,
    info: String,
    host: String,
    lag: f32,
    #[serde(rename = "type")]
    maxlag_type: String,
    #[serde(rename = "*")]
    notes: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct MaxLagError {
    error: MaxLagFrame,
    servedby: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct MisssingTitleFrame {
    code: String,
    info: String,
    #[serde(rename = "*")]
    notes: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct MissingTitleError {
    error: MisssingTitleFrame,
    servedby: String,
}

// ***********************************************************************************************

#[derive(Debug)]
#[allow(dead_code)]
pub enum FetchCommand {
    End,
    Get {
        title: String,
        tx: mpsc::Sender<FetchResult>,
    },
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FetchEntry {
    pub digest: entry::Digest,
    pub title: String,
    pub outbound: Vec<String>,
}

impl FetchEntry {
    fn log(&self, title: &str) {
        info!("Retrieved page {}", title);
    }
}

#[derive(Debug)]
pub enum FetchError {
    IO(std::io::Error),
    Reqwest(reqwest::Error),
    Http(reqwest::StatusCode),
    Lag(f32),
    MissingTitle,
    Parse(String),
}

impl FetchError {
    fn log(&self, title: &str) {
        match &self {
            FetchError::IO(err) => error!("IO error fetching page: {}", err),
            FetchError::Reqwest(err) => error!(
                r#"Reqwest reported an error fetching page "{}"". Reported error: {}"#,
                title, err
            ),
            FetchError::Http(err) => error!("Http response {:?} fetching page: {}", err, title),
            FetchError::Lag(lag) => error!("Lag error of {} secs fetching page: {}", lag, title),
            FetchError::MissingTitle => error!(r#"Requested title "{}" cannot be found"#, title),
            FetchError::Parse(parse_err) => error!(
                "Unable to parse response from page title {} secs fetching page: {}",
                title, parse_err
            ),
        }
    }
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let err_msg = match self {
            FetchError::IO(io_error) => io_error.to_string(),
            FetchError::Reqwest(io_error) => io_error.to_string(),
            FetchError::Http(status_code) => status_code.as_str().to_string(),
            FetchError::Lag(message) => message.to_string(),
            FetchError::MissingTitle => "Missing title".to_string(),
            FetchError::Parse(parse_error_) => parse_error_.to_string(),
        };
        write!(f, "{}", err_msg)
    }
}

impl From<io::Error> for FetchError {
    fn from(error: io::Error) -> Self {
        FetchError::IO(error)
    }
}

impl From<reqwest::Error> for FetchError {
    fn from(error: reqwest::Error) -> Self {
        FetchError::Reqwest(error)
    }
}

type FetchResult = Result<FetchEntry, FetchError>;

/* *****************************************************************************************************************
 *
 * Start the fetch task
 *
 *******************************************************************************************************************/

pub async fn new(
    foundation: &foundation::Foundation,
) -> (JoinHandle<()>, mpsc::Sender<FetchCommand>) {
    trace!("main::init_fetch");

    let worker_count = foundation.get_worker_count().try_into().unwrap();
    let (tx_to_fetch, rx_by_fetch): (mpsc::Sender<FetchCommand>, mpsc::Receiver<FetchCommand>) =
        mpsc::channel(worker_count);

    let fetch_service = tokio::spawn(async move { fetch_service(rx_by_fetch).await });

    (fetch_service, tx_to_fetch)
}

pub async fn fetch_service(mut rx: mpsc::Receiver<FetchCommand>) {
    //pub async fn new() {
    trace!("fetch::new: Spawned fetch");
    loop {
        use FetchCommand::*;

        let fetch_command = rx.recv().await.unwrap();
        trace!("fetch:: Got command");
        match fetch_command {
            Get { title, tx } => tx.send(get_links_from_title(title).await).await.unwrap(),
            End => break,
        }
    }
    trace!("Ending...");
}

/* *****************************************************************************************************************
 *
 * Get a page from Wikipedia or local cache
 *
 *******************************************************************************************************************/

// UNTESTED
pub async fn get_links_from_title(title: String) -> FetchResult {
    let title = title.trim();
    let fetched_page = get_page_from(title).await?;
    let response = parse(&fetched_page);
    check_maxlag(&URL, response, fetched_page, title).await
}

// UNTESTED
async fn get_page_from(title: &str) -> Result<String, FetchError> {
    let path_to_page = get_cache_directory_from(&title);

    let mut exists = false;
    if let Ok(path) = &path_to_page {
        exists = path.exists();
    }

    if exists {
        info!(r#"Found page "{}" in local cache"#, title);
        Ok(fs::read_to_string(path_to_page.as_ref().unwrap())?)
    } else {
        info!(r#"Pulling page "{}" from Wikipedia"#, title);
        let fetch = fetch_page(&URL, title).await?;
        Ok(fetch)
    }
}

fn parse(payload: &str) -> FetchResult {
    let parsed: Result<Page, serde_json::Error> = serde_json::from_str(&payload);
    if let Ok(parsed) = parsed {
        trace!("fetch::parse: Parsed page: {}", &parsed.parse.title);
        return extract_links_from(parsed);
    }

    let maxlag: Result<MaxLagError, serde_json::Error> = serde_json::from_str(&payload);
    if let Ok(lag) = maxlag {
        let lag_value = lag.error.lag;
        trace!("fetch::parse: Received maxlag of {} sec", lag_value);
        return Err(FetchError::Lag(lag_value));
    }

    let missing_title: Result<MissingTitleError, serde_json::Error> =
        serde_json::from_str(&payload);
    if let Ok(_) = missing_title {
        trace!("fetch::parse: Received Missing Title");
        return Err(FetchError::MissingTitle);
    }

    error!("fetch::parse: Unknown wikipedia payload: {}", payload);
    return Err(FetchError::Parse(String::from(PARSE_ERROR)));
}

async fn check_maxlag(
    url: &str,
    mut response: FetchResult,
    mut page: String,
    title: &str,
) -> FetchResult {
    let mut tries = 4;
    loop {
        match &response {
            Ok(_) => {
                cache_page(&page, get_cache_directory_from(&title));
                break response;
            }
            Err(lag_error) if matches!(lag_error, FetchError::Lag(_)) => {
                if tries <= 0 {
                    break response;
                };
                tries -= 1;
                let duration = tokio::time::Duration::new(*MAXLAG_VALUE, 0);
                tokio::time::sleep(duration).await;
                page = fetch_page(url, title).await?;
                response = parse(&page);
            }
            Err(_) => break response,
        }
    }
}

// ***********************************************************************************************

async fn fetch_page(root_url: &str, title: &str) -> Result<String, FetchError> {
    let url = build_url(root_url, title);
    let response = reqwest::get(url.as_str()).await?;
    let status = response.status();
    let links = match status {
        StatusCode::OK => Ok(response.text().await?),
        _ => {
            info!(
                "fetch::fetch_page: Reqwest returned status code: {}",
                status.to_string()
            );
            Err(FetchError::Http(status))
        }
    };
    links
}

fn extract_links_from(parsed: Page) -> FetchResult {
    let outbound: Vec<String> = parsed
        .parse
        .links
        .into_iter()
        .filter(|link| link.ns == 0)
        .map(|link| link.title)
        .collect();

    let digest = entry::Entry::get_digest(&parsed.parse.title);
    Ok(FetchEntry {
        digest,
        title: parsed.parse.title,
        outbound,
    })
}

fn cache_page(contents: &str, path_to_page: Result<PathBuf, io::Error>) {
    if let Ok(path) = &path_to_page {
        match fs::write(path, &contents) {
            Ok(_) => info!("Saved {:?} to cache", path.as_os_str()),
            Err(_) => info!("Failed to save {:?} to cache", path.as_os_str()),
        }
    }
}

fn get_cache_directory_from(title: &str) -> Result<PathBuf, io::Error> {
    let title_digest = entry::Entry::get_digest(title);
    let mut path_to_page = opt::OPT.get_cache();
    path_to_page.push(format!("{:02x?}", title_digest[2]));
    path_to_page.push(format!("{:02x?}", title_digest[1]));
    path_to_page.push(format!("{:02x?}", title_digest[0]));
    create_dir_all(&path_to_page)?;
    path_to_page.push(title);
    path_to_page.set_extension("json");
    Ok(path_to_page)
}

fn build_url(root_url: &str, title: &str) -> Url {
    let api = Url::parse_with_params(
        root_url,
        &[
            ("action", "parse"),
            ("format", "json"),
            ("page", title),
            ("prop", "links"),
        ],
    )
    .unwrap();

    api
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
    fn test_parse_maxlag() {
        let parsed = parse(MAXLAG_PAGE).err();

        if let FetchError::Lag(lag) = parsed.unwrap() {
            assert_eq!(lag, 0.596);
        } else {
            assert!(false)
        }
    }

    #[test]
    fn test_parse_fail() {
        let parsed = parse(FAIL_PAGE).err();

        if let FetchError::Parse(message) = parsed.unwrap() {
            assert_eq!(message, String::from(PARSE_ERROR));
        } else {
            assert!(false)
        }
    }

    #[test]
    fn test_parse_missing_page() {
        let parsed = parse(MISSING_TITLE_PAGE).err();
        assert!(matches!(parsed.unwrap(), FetchError::MissingTitle));
    }

    #[test]
    fn test_parse_success() {
        let entry = parse(SUCCESS_PAGE).unwrap();
        assert_eq!(entry.title, "Value network");
        assert_eq!(
            entry.digest,
            [165, 46, 141, 56, 102, 47, 14, 148, 186, 90, 70, 92, 181, 12, 96, 46]
        );
        assert_eq!(entry.outbound.len(), 2);
        assert_eq!(entry.outbound[0], "Adolescent cliques");
        assert_eq!(entry.outbound[1], "Assortative mixing");
    }

    #[tokio::test]
    async fn test_fetch_success() {
        // External url "https://en.wikipedia.org/w/api.php?action=parse&format=json&page=Value+network&prop=links"
        // Will use the url "<server>:<port>?action=parse&format=json&page=Value+network&prop=links"

        let server = MockServer::start();
        let ms = server.mock(|when, then| {
            when.path(PATH)
                .query_param("action", "parse")
                .query_param("format", "json")
                .query_param("page", "Value network")
                .query_param("prop", "links");
            then.status(200).body(SUCCESS_PAGE);
        });

        let url = server.url(PATH).to_string();
        let links = fetch_page(&url, "Value network").await;
        ms.assert();
        assert_eq!(links.is_ok(), true);
        assert_eq!(links.unwrap(), SUCCESS_PAGE);
    }

    #[tokio::test]
    async fn test_maxlag() {
        let server = MockServer::start();
        let ms = server.mock(|when, then| {
            when.path(PATH)
                .query_param("action", "parse")
                .query_param("format", "json")
                .query_param("page", "Maxlag Value")
                .query_param("prop", "links");
            then.status(200).body(MAXLAG_PAGE);
        });

        let url = server.url(PATH).to_string();
        //  let links = fetch_page(&url, "Maxlag Value").await;
        let response = parse(&MAXLAG_PAGE);
        let fetch_result =
            check_maxlag(&url, response, String::from(MAXLAG_PAGE), "Maxlag Value").await;
        ms.assert_hits(4);
        assert!(fetch_result.is_err());
        assert!(matches!(fetch_result.unwrap_err(), FetchError::Lag(_)));
    }

    #[test]
    fn test_build_url() {
        let root_url = "https://en.wikipedia.org/";
        let url = build_url(root_url, "Value network");
        assert_eq!(
            url.as_str(),
            "https://en.wikipedia.org/?action=parse&format=json&page=Value+network&prop=links"
        );
    }

    // ***********************************************************************************************

    const SUCCESS_PAGE: &str = r###"{
	"parse": {
		"title": "Value network",
		"pageid": 1614337,
		"links": [
			{
				"ns": 1,
				"exists": "",
				"*": "Talk:Value network"
			},
			{
				"ns": 0,
				"exists": "",
				"*": "Adolescent cliques"
			},
			{
				"ns": 0,
				"exists": "",
				"*": "Assortative mixing"
			},
			{
				"ns": 11,
				"exists": "",
				"*": "Template talk:Social networking"
			},
			{
				"ns": 12,
				"exists": "",
				"*": "Help:Maintenance template removal"
			}
		]
	}
}
"###;

    const FAIL_PAGE: &str = r###"{
        "invalid": {
            "title": "Value network",
            "pageid": 1614337,
            "links": [
                {
                    "ns": 0,
                    "exists": "",
                    "*": "Adolescent cliques"
                }
            ]
        }
    }
"###;

    const MAXLAG_PAGE: &str = r###"{
        "error": {
            "code": "maxlag",
            "info": "Waiting for 10.64.48.58: 0.596932 seconds lagged.",
            "host": "10.64.48.58",
            "lag": 0.596,
            "type": "db",
            "*": "See https://www.mediawiki.org/w/api.php for API usage. Subscribe to the mediawiki-api-announce mailing list at &lt;https://lists.wikimedia.org/postorius/lists/mediawiki-api-announce.lists.wikimedia.org/&gt; for notice of API deprecations and breaking changes."
        },
        "servedby": "mw1359"
    }
"###;

    const MISSING_TITLE_PAGE: &str = r###"{
        "error": {
            "code": "missingtitle",
            "info": "The page you specified doesn't exist.",
            "*": "See https://en.wikipedia.org/w/api.php for API usage. Subscribe to the mediawiki-api-announce mailing list at &lt;https://lists.wikimedia.org/mailman/listinfo/mediawiki-api-announce&gt; for notice of API deprecations and breaking changes."
        },
        "servedby": "mw1316"
    }
"###;
}
