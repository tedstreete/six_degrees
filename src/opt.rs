/*************************************************************************************************
 *
 * Command line options
 *
 *************************************************************************************************/

use clap;
use std::{
    cmp::{max, min},
    path::PathBuf,
};

#[derive(clap::StructOpt, Debug)]
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

    // Override processor core count
    #[structopt(short = 'o', long, help = "Processor core count")]
    cores: Option<u64>,

    // Depth of wikipedia hierarchy to return
    #[structopt(
        short,
        long,
        help = "Depth of hierarchy",
        long_help = "The depth of the hierarchy below the requested page that will be returned. 1 = the requested page only; 2 = the requested page, plus all those directly referenced by that page, etc. The maximum depth is 6",
        default_value = "2"
    )]
    depth: u32,

    // Domain name for wikipedia API URL
    // URLs are defined at https://www.mediawiki.org/wiki/API:Main_page
    #[structopt(
        short = 'n',
        long = "domain_name",
        help = "Domain name for wikipedia API URL",
        default_value = "https://en.wikipedia.org/"
    )]
    domain_name: String,

    // System memory
    // WARNING: USE WITH CARE. Normal operation will avoid the use of swap space
    // This option is intended for development use, to prevent allocation of all memory,
    // relegating the debugger to usiong swap
    #[structopt(
        short,
        long,
        help = "The amount of system memory in KB. Use with care to avoid use of swap space"
    )]
    memory: Option<u64>,

    // Port on which the API should be presented
    #[structopt(
        short,
        long,
        help = "Port on which the API is presented",
        default_value = "6360"
    )]
    port: u32,

    // Number of workers
    #[structopt(
        short,
        long,
        help = "Number of worker tasks that will be spawned",
        long_help = "If no value is provided here, the number of workers is equal to the number of cores in the system, * 2 rounded down to the nearest power of 2"
    )]
    workers: Option<usize>,
}

lazy_static! {
    pub static ref OPT: Opt = clap::Parser::parse();
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
        max(1, min(self.depth, 6))
    }
    pub fn get_port(&self) -> u32 {
        self.port
    }
    pub fn get_domain_name(&self) -> &str {
        &self.domain_name
    }
    pub fn get_memory(&self) -> &Option<u64> {
        &self.memory
    }
    pub fn get_cores(&self) -> &Option<u64> {
        &self.cores
    }
    pub fn get_worker_count(&self) -> Option<usize> {
        self.workers
    }
}
