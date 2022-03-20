//! Determine foundational attributes based on available system memory

use md5::Digest;
use std::{cmp, env, panic};
use sysinfo::{ComponentExt, System, SystemExt};

use crate::opt::OPT;

lazy_static! {
    static ref SYSTEM: System = {
        let mut sys = System::new_all();
        sys.refresh_all();
        sys
    };
}

#[derive(Debug)]
pub struct Foundation {
    worker_count: u32,
    bits_for_workers: u32,
    slabs_per_worker: u32,
    bits_per_slab: u32,
    spare_count: u32,
    spare_slabs: Vec<u64>,
}

impl Foundation {
    pub fn new() -> Foundation {
        get_foundation_for(system_memory(), system_cores())
    }

    pub fn get_worker_count(&self) -> u32 {
        self.worker_count
    }

    pub fn get_slabs_per_worker(&self) -> u32 {
        self.slabs_per_worker
    }

    /// Returns the number of unallocated spare slabs
    pub fn get_spare_count(&self) -> u32 {
        self.spare_count
    }

    /// Allocate a spare slab from the pool of spare slabs
    ///
    /// TODO Stub: Write this and tests for this

    pub fn get_spare_slab(&self) -> Option<bool> {
        // Don't forget to reduce the spare_slabs count when allocating
        None
    }
}

/*
* Allocate slabs as [u8, 1MB]
* use bincode to serialize entries from structs into slabs and deserialize into structs

*/
fn get_foundation_for(system_memory: u64, cores: usize) -> Foundation {
    if system_memory < 2097152 {
        error!("Minimum memory is 2GB");
        std::process::exit(1);
    }

    // Use cores * 2 to account for hyperthreading that may be enabled on some processor architectures
    // Over-allocating tasks on a non-hyperthreaded processor will not have a meaningful impact
    // worker count cannot exceed 65K workers (16 bits)
    let raw_workers: u32 = (cores * 2) as u32;
    let worker_count = round_down_to_power_of_2(raw_workers);

    // Deprecated calculation for worker_count. Replaced with core based approach
    // The number of tasks is determined from (system_memory{<MB>} ÷ 60) rounded down to next power of 2
    //    let raw_workers = (system_memory / 1024) / 60;
    //   let worker_count = round_down_to_power_of_2(raw_workers);

    let working_memory = 1024 * 1024; // Allow 1GB for execution and working memory
    let tx_handle_count = cmp::max(8 * worker_count / 1024, 1024); // 8 bytes per handle, with minimum of 1MB
                                                                   // TODO Valildate average message size
    let message_size = worker_count * 1024; // Average message size of 1k
    let tokio_task_cache = 64 * worker_count / 1024;
    let reserved_memory: u64 =
        (working_memory + tx_handle_count + message_size + tokio_task_cache) as u64;
    let memory_for_slabs = system_memory - reserved_memory;
    let slabs = (memory_for_slabs / (1024)) as u32; // Each slab is 1MB
    let slabs_per_worker = round_down_to_power_of_2(slabs / worker_count as u32);
    let spare_count = (slabs - (worker_count * slabs_per_worker)) * 2; // spare slabs are 500KB
    let spare_slabs = Vec::new();

    // Only the lower 32 bits in the digest are significant in identifying slabs and workers.
    //    16 bits max for worker_id (65k workers for a single instance)
    //    16 bits max for slab id (65k slabs per worker)
    // Panic if the number of workers and slabs exceed 2^64
    let bounds: u64 = (worker_count as u32 * slabs_per_worker).into();
    if bounds > u32::MAX.into() {
        error!(
            "Too many slabs or workers. Use the --memory option to reduce the memory when starting"
        );
        // This is an extremely unlikely occurrance, so we'll just panic rather than propagating an error that is
        // very unlikely to occur
        panic!(
            "Too many slabs or workers. Use the --memory option to reduce the memory when starting"
        )
    }

    Foundation {
        worker_count,
        slabs_per_worker,
        spare_count,
        spare_slabs,
        bits_for_workers: 0,
        bits_per_slab: 0,
    }
}

fn round_down_to_power_of_2(value: u32) -> u32 {
    // Round-down to next power of two
    let mut power: u32 = 1;
    while power <= value {
        power *= 2;
    }

    power / 2
}

fn system_memory() -> u64 {
    match OPT.get_memory() {
        Some(memory) => *memory,
        None => SYSTEM.total_memory(),
    }
}

fn system_cores() -> usize {
    match OPT.get_cores() {
        Some(cores) => *cores as usize,
        None => SYSTEM.physical_core_count().unwrap(),
    }
}

/* *****************************************************************************************************************
 *
 * Tests
 *
 * *****************************************************************************************************************/

// Module is public, as get_test_foundation is called from test functions in worker.rs
#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_foundation() {
        let foundation = get_test_foundation();
        assert_eq!(foundation.get_worker_count(), 16);
        assert_eq!(foundation.get_slabs_per_worker(), 256);
        assert_eq!(foundation.get_spare_count(), 6502);
        assert_eq!(foundation.spare_slabs.len(), 0);
    }

    /* *****************************************************************************************************************
     *
     * Helper functions - Used only by test routines, but need to be public so that they are accessible from other
     * modules
     * *****************************************************************************************************************/

    /// Create a default Foundation struct with a small memory footprint that will not exhaust available memory,
    /// leaving sufficient memory for developer tools to run alongside the tests

    pub fn get_test_foundation() -> Foundation {
        get_foundation_for(8589934, 8)
    }
}
