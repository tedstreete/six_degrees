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

//use crate::opt;
use reqwest::{blocking, header::HeaderValue};
use std::{fmt, io};

lazy_static! {
    static ref ATTRIBUTES_FOR_PAGE: Vec<(&'static str, &'static str)> = {
        let mut v = Vec::with_capacity(3);
        v.push(("action", "parse"));
        v.push(("format", "json"));
        v.push(("prop", "links"));
        v.push(("maxlag", "5"));
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
}

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

// ***********************************************************************************************

#[derive(Deserialize, Serialize, Debug)]
pub struct FetchEntry {
    digest: [u8; 16],
    title: String,
    outbound: Vec<String>,
}

impl FetchEntry {
    pub fn get_digest(title: &str) -> [u8; 16] {
        md5::compute(title).into()
    }
}

#[derive(Debug)]
pub enum FetchError {
    IO(std::io::Error),
    Reqwest(reqwest::Error),
    Http(reqwest::StatusCode),
    Lag(String),
    PageNotFound(String),
    Parse(serde_json::Error),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let err_msg = match self {
            FetchError::IO(io_error) => io_error.to_string(),
            FetchError::Reqwest(io_error) => io_error.to_string(),
            FetchError::Http(status_code) => status_code.as_str().to_string(),
            FetchError::Lag(message) => message.to_string(),
            FetchError::PageNotFound(message) => message.to_string(),
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

impl From<serde_json::Error> for FetchError {
    fn from(error: serde_json::Error) -> Self {
        FetchError::Parse(error)
    }
}

/* *****************************************************************************************************************
 *
 * Parse page
 *
 * *****************************************************************************************************************/

pub fn parse(payload: &str) -> Result<FetchEntry, FetchError> {
    match parse_for_links_from(payload) {
        Ok(fetchEntry) => Ok(fetchEntry),
        Err(_) => parse_for_error_from(payload),
    }
}

fn parse_for_links_from(payload: &str) -> Result<FetchEntry, FetchError> {
    let page: Page = serde_json::from_str(&payload)?;

    let outbound: Vec<String> = page
        .parse
        .links
        .into_iter()
        .filter(|link| link.ns == 0)
        .map(|link| link.title)
        .collect();

    let digest = FetchEntry::get_digest(&page.parse.title);
    Ok(FetchEntry {
        digest,
        title: page.parse.title,
        outbound,
    })
}

fn parse_for_error_from(payload: &str) -> Result<FetchEntry, FetchError> {
    Err(FetchError::Lag(String::from("Placeholder")))
}

/* *****************************************************************************************************************
 *
 * Tests
 *
 * *****************************************************************************************************************/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn parse_fail() {
        let _ = parse(FAIL_PAGE).unwrap();
    }

    #[test]
    fn parse_success() {
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
}
