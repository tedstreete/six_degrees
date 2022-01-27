/*************************************************************************************************
 *
 * Command line options
 *
 *************************************************************************************************/

use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "six_degrees")]
pub struct Opt {
    // Directory to hold cache files
    #[structopt(
        short,
        long,
        parse(from_os_str),
        help = "Directory where six_degrees can cache pages",
        default_value = "$HOME/six_degrees_cache"
    )]
    cache: PathBuf,

    // Directory to hold cache files
    #[structopt(
        short,
        long,
        help = "Depth of hierarchy",
        long_help = "The depth of the hierarchy below the requested page. 1 = the requested page only; 2 = the requested page, plus all those directly referenced by that page, etc. The maximum depth is 6",
        default_value = "2"
    )]
    depth: u32,

    // Port on which the API should be presented
    #[structopt(
        short,
        long,
        help = "Port on which the API is presented",
        default_value = "6360"
    )]
    port: u32,

    // Numbr of tasks
    #[structopt(
        short,
        long,
        help = "Number of tasks that will be spawned",
        long_help = "If no value is provided here, the number of tasks will be calculated from the formula (<amount of memory in MB> / 60) rounded down to the nearest power of 2",
        default_value = "0"
    )]
    tasks: u32,
}

lazy_static! {
    pub static ref OPT: Opt = Opt::from_args();
}

impl Opt {
    pub fn get_cache(&self) -> PathBuf {
        if self.cache.starts_with("$HOME") {
            let mut cache = PathBuf::new();
            cache.push(home::home_dir().unwrap());
            cache.push(self.cache.file_name().unwrap().clone());
            cache
        } else {
            self.cache.clone()
        }
    }
    pub fn get_depth(&self) -> u32 {
        self.depth
    }
    pub fn get_port(&self) -> u32 {
        self.port
    }
}
