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

use crate::opt;
use reqwest::{blocking, header::HeaderValue, Url};
use std::{fmt, fs, io};

lazy_static! {
    static ref ATTRIBUTES_FOR_PAGE: Vec<(&'static str, &'static str)> = {
        let mut v = Vec::with_capacity(3);
        v.push(("action", "parse"));
        v.push(("format", "json"));
        v.push(("prop", "links"));
        v.push(("maxlag", "5"));
        v
    };
    static ref client: blocking::Client = {
        let user_agent = HeaderValue::from_str("SixDegrees/0.1 sixdegrees@streete.net")
            .expect(&"Internal error parsing USER_AGENT value in wikipedia::init()");
        reqwest::blocking::Client::builder()
            .gzip(true)
            .user_agent(user_agent)
            .build()
            .expect("Internal error creating fetch::client")
    };
}

#[derive(Deserialize, Serialize, Debug)]
struct Entry {
    digest: [u8; 16],
    title: String,
    outbound: Vec<[u8; 16]>,
    inbound: Vec<[u8; 16]>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WikiLink {
    pub ns: i32,
    pub exists: Option<String>,
    #[serde(rename = "*")]
    pub link: String,
}

#[derive(Debug)]
pub enum FetchError {
    IO(std::io::Error),
    Reqwest(reqwest::Error),
    Http(reqwest::StatusCode),
    Lag(String),
    PageNotFound(String),
    Parse(String),
}

type FetchResult = Result<Entry, FetchError>;

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let err_msg = match self {
            FetchError::IO(io_error) => io_error.to_string(),
            FetchError::Reqwest(io_error) => io_error.to_string(),
            FetchError::Http(status_code) => status_code.as_str().to_string(),
            FetchError::Lag(message) => message.to_string(),
            FetchError::PageNotFound(message) => message.to_string(),
            FetchError::Parse(message) => message.to_string(),
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

/* *****************************************************************************************************************
 *
 * Get a page from Wikipedia or local cache
 *
 * NOTE: Not tested by unit tests
 *
 *******************************************************************************************************************/

pub fn get_page_from(page: &str) -> Result<String, FetchError> {
    let mut path_to_page = opt::OPT.get_cache();
    path_to_page.push(page);
    let fetch = {
        if path_to_page.exists() {
            fs::read_to_string(path_to_page)?
        } else {
            fetch_page(page)?
        }
    };

    Ok(fetch)
}

fn fetch_page(title: &str) -> Result<String, FetchError> {
    let url = build_url(title);
    let response = reqwest::blocking::get(url.as_str())?;
    let status = response.status();
    let links = match status {
        reqwest::status_code::success => deserialize_jason_from(&response.text()?),
        _ => info!("Request returned status code: {}", status.to_string),
    };

    if status.is_success() {
        Ok(response.text()?)
    } else {
        Err(FetchError::Http(status))
    }
}

fn build_url(title: &str) -> url::Url {
    // TODO: encode title into query encoding if necessary (the Url crate may map this correctly - need to check for accented characters etc.)
    let api = Url::parse_with_params(
        opt::OPT.get_root_url(),
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
//fn parse (&str) -> Result <Entry,FetchError> {
//Ok(Entry{});
//}

/*
            if status.is_success() {
                fs::write(&path_to_page, &body).unwrap();
            }
            let status = reqwest::StatusCode::from_u16(299);
            let body = String::from(
                r#"{
    "parse": {
        "title": "Six degrees of separation",
        "pageid": 40117483,
        "links": []
    }
}"#,
            );
            (Some(status), body)
        }
    };

    let links = match status {
        None => deserialize_jason_from(&body),
        Some(code) => {
            info!(
                "Deserializing {} with status {}",
                &page,
                code.unwrap().as_u16()
            );
            deserialize_jason_from(&body)
            /*
                        Some(code) => {
                            info!("Error in wikipedia response. Status {}", &code.as_str());
                            Err(anyhow::Error::new(ResponseError::new(
                                "Error in wikipedia response",
                            )))
                        }
            */
        }
    };
    links */

/* *****************************************************************************************************************
 *
 * Tests
 *
 *******************************************************************************************************************/

#[test]
fn network_error() {}
fn maxlag_success() {}
fn maxlag_error() {}
fn cache_error() {}
fn parse_error() {}
fn success() {}

const MISSINGTITLE_PAGE: &str = r###"{
  "error": {
    "code": "missingtitle",
    "info": "The page you specified doesn't exist.",
    "*": "See https://en.wikipedia.org/w/api.php for API usage. Subscribe to the mediawiki-api-announce mailing list at &lt;https://lists.wikimedia.org/mailman/listinfo/mediawiki-api-announce&gt; for notice of API deprecations and breaking changes."
  },
  "servedby": "mw1286"
}
"###;

const MAXLAG_PAGE: &str = r###"{
  "error": {
    "code": "maxlag",
    "info": "Waiting for 10.64.16.12: 5.0743770599365 seconds lagged.",
    "host": "10.64.16.12",
    "lag": 5.074377059936523,
    "type": "db",
    "*": "See https://en.wikipedia.org/w/api.php for API usage. Subscribe to the mediawiki-api-announce mailing list at &lt;https://lists.wikimedia.org/mailman/listinfo/mediawiki-api-announce&gt; for notice of API deprecations and breaking changes."
  },
  "servedby": "mw1378"
}
"###;

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
				"ns": 14,
				"exists": "",
				"*": "Category:NPOV disputes from May 2018"
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
				"ns": 4,
				"exists": "",
				"*": "Wikipedia:Neutral point of view"
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
